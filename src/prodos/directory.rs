use std::collections::HashSet;

use crate::error::{A2FuseError, Result};

use super::block::BlockDevice;
use super::path::decode_filename_with_case;
use super::types::{AccessFlags, ProdosTimestamp, StorageType};

pub const PRODOS_ENTRY_LENGTH: usize = 39;
pub const PRODOS_ENTRIES_PER_BLOCK: usize = 13;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub name: String,
    pub storage_type: StorageType,
    pub file_type: u8,
    pub key_pointer: u16,
    pub blocks_used: u16,
    pub eof: u32,
    pub creation: Option<ProdosTimestamp>,
    pub modification: Option<ProdosTimestamp>,
    pub access: AccessFlags,
    pub aux_type: u16,
    pub header_pointer: u16,
}

impl DirectoryEntry {
    pub fn parse(bytes: &[u8]) -> Result<Option<Self>> {
        if bytes.len() < PRODOS_ENTRY_LENGTH {
            return Err(A2FuseError::InvalidDirectoryEntry(format!(
                "expected {PRODOS_ENTRY_LENGTH} bytes, got {}",
                bytes.len()
            )));
        }

        let storage_nibble = bytes[0] >> 4;
        let name_length = usize::from(bytes[0] & 0x0f);
        if storage_nibble == 0 {
            return Ok(None);
        }
        if name_length == 0 || name_length > 15 {
            return Err(A2FuseError::InvalidDirectoryEntry(format!(
                "invalid filename length {name_length}"
            )));
        }
        let storage_type = StorageType::from_nibble(storage_nibble).ok_or_else(|| {
            A2FuseError::InvalidDirectoryEntry(format!("unknown storage type {storage_nibble:#x}"))
        })?;

        Ok(Some(Self {
            name: decode_filename_with_case(&bytes[1..1 + name_length], read_u16(bytes, 0x1c)),
            storage_type,
            file_type: bytes[0x10],
            key_pointer: read_u16(bytes, 0x11),
            blocks_used: read_u16(bytes, 0x13),
            eof: read_u24(bytes, 0x15),
            creation: ProdosTimestamp::decode(read_u16(bytes, 0x18), read_u16(bytes, 0x1a)),
            access: AccessFlags(bytes[0x1e]),
            aux_type: read_u16(bytes, 0x1f),
            modification: ProdosTimestamp::decode(read_u16(bytes, 0x21), read_u16(bytes, 0x23)),
            header_pointer: read_u16(bytes, 0x25),
        }))
    }

    pub fn is_directory(&self) -> bool {
        self.storage_type == StorageType::Subdirectory
    }

    pub fn is_file(&self) -> bool {
        self.storage_type.is_regular_file()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directory {
    pub key_block: u16,
    pub entries: Vec<DirectoryEntry>,
}

impl Directory {
    pub fn read(device: &BlockDevice, key_block: u16) -> Result<Self> {
        let mut entries = Vec::new();
        let mut block_number = key_block;
        let mut visited = HashSet::new();
        let mut first_block = true;

        while block_number != 0 {
            if !visited.insert(block_number) {
                return Err(A2FuseError::InvalidDirectory(format!(
                    "directory block chain contains a cycle at block {block_number}"
                )));
            }

            let block = device.read_block(block_number)?;
            let next_block = u16::from_le_bytes([block[2], block[3]]);

            for slot in 0..PRODOS_ENTRIES_PER_BLOCK {
                if first_block && slot == 0 {
                    continue;
                }
                let start = 4 + slot * PRODOS_ENTRY_LENGTH;
                let end = start + PRODOS_ENTRY_LENGTH;
                if let Some(entry) = DirectoryEntry::parse(&block[start..end])?
                    && !matches!(
                        entry.storage_type,
                        StorageType::VolumeHeader | StorageType::SubdirectoryHeader
                    )
                {
                    entries.push(entry);
                }
            }

            first_block = false;
            block_number = next_block;
        }

        Ok(Self { key_block, entries })
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u24(bytes: &[u8], offset: usize) -> u32 {
    u32::from(bytes[offset])
        | (u32::from(bytes[offset + 1]) << 8)
        | (u32::from(bytes[offset + 2]) << 16)
}

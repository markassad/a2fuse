use crate::error::{A2FuseError, Result};

use super::block::{BLOCK_SIZE, BlockDevice};
use super::directory::DirectoryEntry;
use super::types::StorageType;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileFork {
    pub name: String,
    pub storage_type: StorageType,
    pub key_pointer: u16,
    pub blocks_used: u16,
    pub eof: u32,
}

impl FileFork {
    pub fn from_entry(entry: &DirectoryEntry) -> Self {
        Self {
            name: entry.name.clone(),
            storage_type: entry.storage_type,
            key_pointer: entry.key_pointer,
            blocks_used: entry.blocks_used,
            eof: entry.eof,
        }
    }

    pub fn new(
        name: impl Into<String>,
        storage_type: StorageType,
        key_pointer: u16,
        blocks_used: u16,
        eof: u32,
    ) -> Self {
        Self {
            name: name.into(),
            storage_type,
            key_pointer,
            blocks_used,
            eof,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtendedKeyBlock {
    pub data_fork: FileFork,
    pub resource_fork: FileFork,
    pub finder_info: [u8; 36],
}

pub fn data_block_numbers(
    device: &BlockDevice,
    entry: &DirectoryEntry,
) -> Result<Vec<Option<u16>>> {
    let block_count =
        usize::try_from(entry.eof.div_ceil(BLOCK_SIZE as u32)).expect("a 24-bit EOF fits in usize");

    if block_count > 0 && entry.key_pointer == 0 {
        return Err(A2FuseError::InvalidDirectoryEntry(format!(
            "{} has data but no key block",
            entry.name
        )));
    }

    match entry.storage_type {
        StorageType::Seedling => {
            if block_count > 1 {
                return Err(A2FuseError::InvalidDirectoryEntry(format!(
                    "seedling file {} is too large for one block",
                    entry.name
                )));
            }
            Ok((block_count > 0)
                .then_some(Some(entry.key_pointer))
                .into_iter()
                .collect())
        }
        StorageType::Sapling => {
            if block_count > 256 {
                return Err(A2FuseError::InvalidDirectoryEntry(format!(
                    "sapling file {} is too large for one index block",
                    entry.name
                )));
            }
            let index = device.read_block(entry.key_pointer)?;
            Ok((0..block_count)
                .map(|position| pointer_at(index, position))
                .collect())
        }
        StorageType::Tree => {
            let master = device.read_block(entry.key_pointer)?;
            let mut blocks = Vec::with_capacity(block_count);
            for position in 0..block_count {
                let sapling_position = position / 256;
                let data_position = position % 256;
                let sapling_pointer = pointer_at(master, sapling_position);
                let data_pointer = match sapling_pointer {
                    Some(pointer) => pointer_at(device.read_block(pointer)?, data_position),
                    None => None,
                };
                blocks.push(data_pointer);
            }
            Ok(blocks)
        }
        storage_type => Err(A2FuseError::UnsupportedStorageType {
            storage_type: storage_type as u8,
            name: entry.name.clone(),
        }),
    }
}

pub fn read_fork(device: &BlockDevice, fork: &FileFork) -> Result<Vec<u8>> {
    if !fork.storage_type.is_regular_file() {
        return Err(A2FuseError::UnsupportedStorageType {
            storage_type: fork.storage_type as u8,
            name: fork.name.clone(),
        });
    }

    let block_count =
        usize::try_from(fork.eof.div_ceil(BLOCK_SIZE as u32)).expect("a 24-bit EOF fits in usize");

    if block_count > 0 && fork.key_pointer == 0 {
        return Err(A2FuseError::InvalidDirectoryEntry(format!(
            "{} has data but no key block",
            fork.name
        )));
    }

    let mut data = Vec::with_capacity(fork.eof as usize);
    for block_number in match fork.storage_type {
        StorageType::Seedling => {
            if block_count > 1 {
                return Err(A2FuseError::InvalidDirectoryEntry(format!(
                    "seedling file {} is too large for one block",
                    fork.name
                )));
            }
            (block_count > 0)
                .then_some(Some(fork.key_pointer))
                .into_iter()
                .collect()
        }
        StorageType::Sapling => {
            if block_count > 256 {
                return Err(A2FuseError::InvalidDirectoryEntry(format!(
                    "sapling file {} is too large for one index block",
                    fork.name
                )));
            }
            let index = device.read_block(fork.key_pointer)?;
            (0..block_count)
                .map(|position| pointer_at(index, position))
                .collect()
        }
        StorageType::Tree => {
            let master = device.read_block(fork.key_pointer)?;
            let mut blocks = Vec::with_capacity(block_count);
            for position in 0..block_count {
                let sapling_position = position / 256;
                let data_position = position % 256;
                let sapling_pointer = pointer_at(master, sapling_position);
                let data_pointer = match sapling_pointer {
                    Some(pointer) => pointer_at(device.read_block(pointer)?, data_position),
                    None => None,
                };
                blocks.push(data_pointer);
            }
            blocks
        }
        storage_type => {
            return Err(A2FuseError::UnsupportedStorageType {
                storage_type: storage_type as u8,
                name: fork.name.clone(),
            });
        }
    } {
        match block_number {
            Some(block_number) => data.extend_from_slice(device.read_block(block_number)?),
            None => data.resize(data.len() + BLOCK_SIZE, 0),
        }
    }
    data.truncate(fork.eof as usize);
    Ok(data)
}

pub fn read_file(device: &BlockDevice, entry: &DirectoryEntry) -> Result<Vec<u8>> {
    if entry.storage_type == StorageType::Extended {
        return read_extended_data_fork(device, entry);
    }
    if !entry.is_file() {
        return Err(A2FuseError::NotAFile(entry.name.clone()));
    }
    read_fork(device, &FileFork::from_entry(entry))
}

pub fn read_extended_data_fork(device: &BlockDevice, entry: &DirectoryEntry) -> Result<Vec<u8>> {
    read_fork(device, &extended_key_block(device, entry)?.data_fork)
}

pub fn read_extended_resource_fork(
    device: &BlockDevice,
    entry: &DirectoryEntry,
) -> Result<Vec<u8>> {
    read_fork(device, &extended_key_block(device, entry)?.resource_fork)
}

pub fn extended_key_block(
    device: &BlockDevice,
    entry: &DirectoryEntry,
) -> Result<ExtendedKeyBlock> {
    if entry.storage_type != StorageType::Extended {
        return Err(A2FuseError::UnsupportedStorageType {
            storage_type: entry.storage_type as u8,
            name: entry.name.clone(),
        });
    }
    if entry.key_pointer == 0 {
        return Err(A2FuseError::InvalidDirectoryEntry(format!(
            "{} has no key block",
            entry.name
        )));
    }
    let block = device.read_block(entry.key_pointer)?;
    let data_fork = fork_entry(entry.name.clone(), block, 0)?;
    let resource_fork = fork_entry(format!("._{}", entry.name), block, 256)?;
    let mut finder_info = [0_u8; 36];
    finder_info.copy_from_slice(&block[8..44]);
    Ok(ExtendedKeyBlock {
        data_fork,
        resource_fork,
        finder_info,
    })
}

fn fork_entry(
    name: impl Into<String>,
    block: &[u8; BLOCK_SIZE],
    offset: usize,
) -> Result<FileFork> {
    let storage_type = StorageType::from_nibble(block[offset]).ok_or_else(|| {
        A2FuseError::InvalidDirectoryEntry(format!(
            "unknown extended fork storage type {:#x}",
            block[offset]
        ))
    })?;
    Ok(FileFork::new(
        name,
        storage_type,
        u16::from_le_bytes([block[offset + 1], block[offset + 2]]),
        u16::from_le_bytes([block[offset + 3], block[offset + 4]]),
        u32::from(block[offset + 5])
            | (u32::from(block[offset + 6]) << 8)
            | (u32::from(block[offset + 7]) << 16),
    ))
}

fn pointer_at(index_block: &[u8; BLOCK_SIZE], position: usize) -> Option<u16> {
    if position >= 256 {
        return None;
    }
    let pointer = u16::from(index_block[position]) | (u16::from(index_block[position + 256]) << 8);
    (pointer != 0).then_some(pointer)
}

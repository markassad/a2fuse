use std::path::Path;

use crate::error::{A2FuseError, Result};

use super::block::BlockDevice;
use super::directory::{Directory, DirectoryEntry, PRODOS_ENTRY_LENGTH};
use super::file::{
    FileFork, extended_key_block, read_extended_data_fork, read_extended_resource_fork, read_fork,
};
use super::path::{MetadataMode, decode_filename_with_case, host_filename};
use super::types::{AccessFlags, ProdosTimestamp, StorageType};

pub const VOLUME_DIRECTORY_KEY_BLOCK: u16 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeHeader {
    pub name: String,
    pub creation: Option<ProdosTimestamp>,
    pub access: AccessFlags,
    pub entry_length: u8,
    pub entries_per_block: u8,
    pub file_count: u16,
    pub bitmap_pointer: u16,
    pub total_blocks: u16,
}

impl VolumeHeader {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < PRODOS_ENTRY_LENGTH {
            return Err(A2FuseError::InvalidVolume(
                "volume header is shorter than 39 bytes".to_owned(),
            ));
        }
        let storage_type = bytes[0] >> 4;
        let name_length = usize::from(bytes[0] & 0x0f);
        if storage_type != StorageType::VolumeHeader as u8 {
            return Err(A2FuseError::InvalidVolume(format!(
                "block 2 does not begin with a volume header (storage type {storage_type:#x})"
            )));
        }
        if name_length == 0 || name_length > 15 {
            return Err(A2FuseError::InvalidVolume(format!(
                "invalid volume name length {name_length}"
            )));
        }

        let header = Self {
            name: decode_filename_with_case(&bytes[1..1 + name_length], read_u16(bytes, 0x16)),
            creation: ProdosTimestamp::decode(read_u16(bytes, 0x18), read_u16(bytes, 0x1a)),
            access: AccessFlags(bytes[0x1e]),
            entry_length: bytes[0x1f],
            entries_per_block: bytes[0x20],
            file_count: read_u16(bytes, 0x21),
            bitmap_pointer: read_u16(bytes, 0x23),
            total_blocks: read_u16(bytes, 0x25),
        };

        if usize::from(header.entry_length) != PRODOS_ENTRY_LENGTH {
            return Err(A2FuseError::InvalidVolume(format!(
                "unsupported directory entry length {}",
                header.entry_length
            )));
        }
        if header.entries_per_block != 13 {
            return Err(A2FuseError::InvalidVolume(format!(
                "unsupported entries-per-block value {}",
                header.entries_per_block
            )));
        }
        Ok(header)
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub entry: DirectoryEntry,
    pub data_fork: Option<FileFork>,
    pub resource_fork: Option<FileFork>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn host_name(&self, metadata_mode: MetadataMode) -> String {
        host_filename(&self.entry, metadata_mode)
    }

    pub fn is_directory(&self) -> bool {
        self.entry.is_directory()
    }

    pub fn is_extended_file(&self) -> bool {
        self.resource_fork.is_some()
    }

    pub fn effective_eof(&self) -> u32 {
        self.data_fork
            .as_ref()
            .map_or(self.entry.eof, |fork| fork.eof)
    }

    pub fn effective_blocks_used(&self) -> u16 {
        self.data_fork
            .as_ref()
            .map_or(self.entry.blocks_used, |fork| fork.blocks_used)
    }

    pub fn effective_storage_type(&self) -> StorageType {
        self.data_fork
            .as_ref()
            .map_or(self.entry.storage_type, |fork| fork.storage_type)
    }
}

#[derive(Clone, Debug)]
pub struct Volume {
    device: BlockDevice,
    pub header: VolumeHeader,
    pub root: Vec<Node>,
}

impl Volume {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_device(BlockDevice::open(path)?)
    }

    pub fn from_device(device: BlockDevice) -> Result<Self> {
        if device.block_count() <= usize::from(VOLUME_DIRECTORY_KEY_BLOCK) {
            return Err(A2FuseError::InvalidVolume(
                "image is too small to contain a volume directory".to_owned(),
            ));
        }

        let directory_block = device.read_block(VOLUME_DIRECTORY_KEY_BLOCK)?;
        let header = VolumeHeader::parse(&directory_block[4..4 + PRODOS_ENTRY_LENGTH])?;
        if header.total_blocks != 0 && usize::from(header.total_blocks) > device.block_count() {
            return Err(A2FuseError::InvalidVolume(format!(
                "volume claims {} blocks but image contains {}",
                header.total_blocks,
                device.block_count()
            )));
        }

        let root_directory = Directory::read(&device, VOLUME_DIRECTORY_KEY_BLOCK)?;
        let root = Self::load_nodes(&device, root_directory, &mut Vec::new())?;
        Ok(Self {
            device,
            header,
            root,
        })
    }

    pub fn read_entry(&self, entry: &DirectoryEntry) -> Result<Vec<u8>> {
        if entry.storage_type == StorageType::Extended {
            read_extended_data_fork(&self.device, entry)
        } else {
            read_fork(&self.device, &FileFork::from_entry(entry))
        }
    }

    pub fn read_fork(&self, fork: &FileFork) -> Result<Vec<u8>> {
        read_fork(&self.device, fork)
    }

    pub fn read_resource_fork(&self, entry: &DirectoryEntry) -> Result<Vec<u8>> {
        read_extended_resource_fork(&self.device, entry)
    }

    pub fn find<'a>(&'a self, path: &str, metadata_mode: MetadataMode) -> Result<&'a Node> {
        let mut nodes = &self.root;
        let mut found = None;

        for component in path.split('/').filter(|part| !part.is_empty()) {
            let node = nodes
                .iter()
                .find(|node| {
                    node.host_name(metadata_mode)
                        .eq_ignore_ascii_case(component)
                })
                .ok_or_else(|| A2FuseError::PathNotFound(path.to_owned()))?;
            found = Some(node);
            nodes = &node.children;
        }

        found.ok_or_else(|| A2FuseError::PathNotFound(path.to_owned()))
    }

    fn load_nodes(
        device: &BlockDevice,
        directory: Directory,
        ancestors: &mut Vec<u16>,
    ) -> Result<Vec<Node>> {
        if ancestors.contains(&directory.key_block) {
            return Err(A2FuseError::InvalidDirectory(format!(
                "recursive directory reference to block {}",
                directory.key_block
            )));
        }
        ancestors.push(directory.key_block);

        let mut nodes = Vec::with_capacity(directory.entries.len());
        for entry in directory.entries {
            let (data_fork, resource_fork, children) = if entry.is_directory() {
                if entry.key_pointer == 0 {
                    return Err(A2FuseError::InvalidDirectory(format!(
                        "subdirectory {} has no key block",
                        entry.name
                    )));
                }
                let child_directory = Directory::read(device, entry.key_pointer)?;
                let children = Self::load_nodes(device, child_directory, ancestors)?;
                (None, None, children)
            } else if entry.storage_type == StorageType::Extended {
                let extended = extended_key_block(device, &entry)?;
                (
                    Some(extended.data_fork),
                    Some(extended.resource_fork),
                    Vec::new(),
                )
            } else {
                if entry.eof > 0 && entry.key_pointer == 0 {
                    return Err(A2FuseError::InvalidDirectoryEntry(format!(
                        "{} has no key block",
                        entry.name
                    )));
                }
                (Some(FileFork::from_entry(&entry)), None, Vec::new())
            };
            nodes.push(Node {
                entry,
                data_fork,
                resource_fork,
                children,
            });
        }

        ancestors.pop();
        Ok(nodes)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

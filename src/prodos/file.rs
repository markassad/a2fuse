use crate::error::{A2FuseError, Result};

use super::block::{BLOCK_SIZE, BlockDevice};
use super::directory::DirectoryEntry;
use super::types::StorageType;

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

pub fn read_file(device: &BlockDevice, entry: &DirectoryEntry) -> Result<Vec<u8>> {
    if !entry.is_file() {
        return Err(A2FuseError::NotAFile(entry.name.clone()));
    }

    let mut data = Vec::with_capacity(entry.eof as usize);
    for block_number in data_block_numbers(device, entry)? {
        match block_number {
            Some(block_number) => data.extend_from_slice(device.read_block(block_number)?),
            None => data.resize(data.len() + BLOCK_SIZE, 0),
        }
    }
    data.truncate(entry.eof as usize);
    Ok(data)
}

fn pointer_at(index_block: &[u8; BLOCK_SIZE], position: usize) -> Option<u16> {
    if position >= 256 {
        return None;
    }
    let pointer = u16::from(index_block[position]) | (u16::from(index_block[position + 256]) << 8);
    (pointer != 0).then_some(pointer)
}

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{A2FuseError, Result};

use super::block::{BLOCK_SIZE, BlockDevice};
use super::directory::{PRODOS_ENTRIES_PER_BLOCK, PRODOS_ENTRY_LENGTH};
use super::types::{AccessFlags, StorageType};
use super::volume::{VOLUME_DIRECTORY_KEY_BLOCK, Volume};

const ROOT_DIRECTORY_BLOCKS: std::ops::RangeInclusive<u16> = 2..=5;
const BITMAP_KEY_BLOCK: u16 = 6;
const MAX_PRODOS_EOF: usize = 0x00ff_ffff;

#[derive(Clone, Debug)]
pub struct CreateOptions {
    pub name: String,
    pub blocks: u16,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            name: "UNTITLED".to_owned(),
            blocks: 280,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PutOptions {
    pub name: String,
    pub file_type: u8,
    pub aux_type: u16,
    pub access: AccessFlags,
}

impl PutOptions {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            file_type: 0x06,
            aux_type: 0,
            access: AccessFlags(0xe3),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Image {
    bytes: Vec<u8>,
}

impl Image {
    pub fn create(options: &CreateOptions) -> Result<Self> {
        let encoded_name = encode_name(&options.name)?;
        let bitmap_blocks = bitmap_block_count(options.blocks)?;
        let first_free_block = BITMAP_KEY_BLOCK
            .checked_add(bitmap_blocks)
            .ok_or_else(|| A2FuseError::InvalidVolumeSize("bitmap block overflow".to_owned()))?;
        if options.blocks <= first_free_block {
            return Err(A2FuseError::InvalidVolumeSize(format!(
                "{} blocks is too small; at least {} are required",
                options.blocks,
                first_free_block + 1
            )));
        }

        let mut image = Self {
            bytes: vec![0; usize::from(options.blocks) * BLOCK_SIZE],
        };
        image.initialise_root_directory(&encoded_name, options.blocks);
        image.initialise_bitmap(options.blocks, bitmap_blocks, first_free_block);
        image.validate()?;
        image.validate_writable_layout()?;
        Ok(image)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| A2FuseError::ReadImage {
            path: path.to_path_buf(),
            source,
        })?;
        let image = Self { bytes };
        image.validate()?;
        image.validate_writable_layout()?;
        Ok(image)
    }

    pub fn volume(&self) -> Result<Volume> {
        Volume::from_device(BlockDevice::from_bytes(self.bytes.clone())?)
    }

    pub fn put_file(&mut self, data: &[u8], options: &PutOptions) -> Result<()> {
        if data.len() > MAX_PRODOS_EOF {
            return Err(A2FuseError::FileTooLarge { size: data.len() });
        }
        let encoded_name = encode_name(&options.name)?;
        let volume = self.volume()?;
        if volume
            .root
            .iter()
            .any(|node| node.entry.name.eq_ignore_ascii_case(&options.name))
        {
            return Err(A2FuseError::FileExists(options.name.clone()));
        }

        let slot = self.find_root_slot()?;
        let data_block_count = data.len().div_ceil(BLOCK_SIZE);
        let storage_type = storage_type_for_blocks(data_block_count);
        let index_block_count = match storage_type {
            StorageType::Seedling => 0,
            StorageType::Sapling => 1,
            StorageType::Tree => data_block_count.div_ceil(256) + 1,
            _ => unreachable!("standard files use seedling, sapling, or tree storage"),
        };
        let allocated = self.allocate_blocks(data_block_count + index_block_count)?;

        let (key_pointer, blocks_used) =
            self.write_file_blocks(data, storage_type, data_block_count, &allocated)?;
        let entry = encode_file_entry(
            &encoded_name,
            storage_type,
            options,
            key_pointer,
            blocks_used,
            data.len() as u32,
        );
        self.bytes[slot..slot + PRODOS_ENTRY_LENGTH].copy_from_slice(&entry);

        let count_offset = block_offset(VOLUME_DIRECTORY_KEY_BLOCK) + 4 + 0x21;
        let file_count = read_u16(&self.bytes, count_offset)
            .checked_add(1)
            .ok_or_else(|| A2FuseError::InvalidDirectory("file count overflow".to_owned()))?;
        put_u16(&mut self.bytes, count_offset, file_count);
        self.validate()?;
        Ok(())
    }

    pub fn save_new(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if path.exists() {
            return Err(A2FuseError::ImageExists(path.to_path_buf()));
        }
        self.write_to(path, true)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.write_to(path.as_ref(), false)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn validate(&self) -> Result<()> {
        Volume::from_device(BlockDevice::from_bytes(self.bytes.clone())?).map(|_| ())
    }

    fn validate_writable_layout(&self) -> Result<()> {
        let header_offset = block_offset(VOLUME_DIRECTORY_KEY_BLOCK) + 4;
        let total_blocks = read_u16(&self.bytes, header_offset + 0x25);
        let bitmap_key_block = read_u16(&self.bytes, header_offset + 0x23);
        if total_blocks == 0 {
            return Err(A2FuseError::InvalidVolumeSize(
                "the volume header declares zero blocks".to_owned(),
            ));
        }
        let bitmap_blocks = bitmap_block_count(total_blocks)?;
        let bitmap_end = bitmap_key_block
            .checked_add(bitmap_blocks)
            .ok_or_else(|| A2FuseError::InvalidVolumeSize("bitmap block overflow".to_owned()))?;
        if bitmap_key_block == 0 || bitmap_end > total_blocks {
            return Err(A2FuseError::InvalidVolumeSize(format!(
                "bitmap blocks {bitmap_key_block}..{bitmap_end} are outside the volume"
            )));
        }
        Ok(())
    }

    fn initialise_root_directory(&mut self, name: &EncodedName, blocks: u16) {
        for block in ROOT_DIRECTORY_BLOCKS {
            let previous = if block == *ROOT_DIRECTORY_BLOCKS.start() {
                0
            } else {
                block - 1
            };
            let next = if block == *ROOT_DIRECTORY_BLOCKS.end() {
                0
            } else {
                block + 1
            };
            let offset = block_offset(block);
            put_u16(&mut self.bytes, offset, previous);
            put_u16(&mut self.bytes, offset + 2, next);
        }

        let header_offset = block_offset(VOLUME_DIRECTORY_KEY_BLOCK) + 4;
        let header = &mut self.bytes[header_offset..header_offset + PRODOS_ENTRY_LENGTH];
        header[0] = (StorageType::VolumeHeader as u8) << 4 | name.bytes.len() as u8;
        header[1..1 + name.bytes.len()].copy_from_slice(&name.bytes);
        put_u16(header, 0x16, name.case_bits);
        header[0x1e] = 0xe3;
        header[0x1f] = PRODOS_ENTRY_LENGTH as u8;
        header[0x20] = PRODOS_ENTRIES_PER_BLOCK as u8;
        put_u16(header, 0x23, BITMAP_KEY_BLOCK);
        put_u16(header, 0x25, blocks);
    }

    fn initialise_bitmap(&mut self, blocks: u16, bitmap_blocks: u16, first_free_block: u16) {
        for block in 0..blocks {
            self.set_block_free(block, block >= first_free_block);
        }
        for bitmap_block in BITMAP_KEY_BLOCK..BITMAP_KEY_BLOCK + bitmap_blocks {
            debug_assert!(bitmap_block < first_free_block);
            self.set_block_free(bitmap_block, false);
        }
    }

    fn find_root_slot(&self) -> Result<usize> {
        let mut block = VOLUME_DIRECTORY_KEY_BLOCK;
        let mut first = true;
        while block != 0 {
            let offset = block_offset(block);
            for slot in 0..PRODOS_ENTRIES_PER_BLOCK {
                if first && slot == 0 {
                    continue;
                }
                let entry_offset = offset + 4 + slot * PRODOS_ENTRY_LENGTH;
                if self.bytes[entry_offset] == 0 {
                    return Ok(entry_offset);
                }
            }
            block = read_u16(&self.bytes, offset + 2);
            first = false;
        }
        Err(A2FuseError::DirectoryFull)
    }

    fn allocate_blocks(&mut self, count: usize) -> Result<Vec<u16>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let header_offset = block_offset(VOLUME_DIRECTORY_KEY_BLOCK) + 4;
        let total_blocks = read_u16(&self.bytes, header_offset + 0x25);
        let blocks: Vec<u16> = (0..total_blocks)
            .filter(|block| self.block_is_free(*block))
            .take(count)
            .collect();
        if blocks.len() != count {
            return Err(A2FuseError::DiskFull);
        }
        for block in &blocks {
            self.set_block_free(*block, false);
        }
        Ok(blocks)
    }

    fn write_file_blocks(
        &mut self,
        data: &[u8],
        storage_type: StorageType,
        data_block_count: usize,
        allocated: &[u16],
    ) -> Result<(u16, u16)> {
        if data_block_count == 0 {
            return Ok((0, 0));
        }

        match storage_type {
            StorageType::Seedling => {
                self.write_data_blocks(data, &allocated[..1]);
                Ok((allocated[0], 1))
            }
            StorageType::Sapling => {
                let index_block = allocated[0];
                let data_blocks = &allocated[1..];
                self.write_index_block(index_block, data_blocks);
                self.write_data_blocks(data, data_blocks);
                Ok((index_block, allocated.len() as u16))
            }
            StorageType::Tree => {
                let master_block = allocated[0];
                let sapling_count = data_block_count.div_ceil(256);
                let sapling_blocks = &allocated[1..1 + sapling_count];
                let data_blocks = &allocated[1 + sapling_count..];
                self.write_index_block(master_block, sapling_blocks);
                for (sapling, chunk) in sapling_blocks.iter().zip(data_blocks.chunks(256)) {
                    self.write_index_block(*sapling, chunk);
                }
                self.write_data_blocks(data, data_blocks);
                Ok((master_block, allocated.len() as u16))
            }
            _ => Err(A2FuseError::UnsupportedStorageType {
                storage_type: storage_type as u8,
                name: "new file".to_owned(),
            }),
        }
    }

    fn write_index_block(&mut self, block: u16, pointers: &[u16]) {
        let offset = block_offset(block);
        self.bytes[offset..offset + BLOCK_SIZE].fill(0);
        for (position, pointer) in pointers.iter().enumerate() {
            self.bytes[offset + position] = *pointer as u8;
            self.bytes[offset + 256 + position] = (*pointer >> 8) as u8;
        }
    }

    fn write_data_blocks(&mut self, data: &[u8], blocks: &[u16]) {
        for (block, chunk) in blocks.iter().zip(data.chunks(BLOCK_SIZE)) {
            let offset = block_offset(*block);
            self.bytes[offset..offset + BLOCK_SIZE].fill(0);
            self.bytes[offset..offset + chunk.len()].copy_from_slice(chunk);
        }
    }

    fn block_is_free(&self, block: u16) -> bool {
        let (byte_offset, mask) = self.bitmap_location(block);
        self.bytes[byte_offset] & mask != 0
    }

    fn set_block_free(&mut self, block: u16, free: bool) {
        let (byte_offset, mask) = self.bitmap_location(block);
        if free {
            self.bytes[byte_offset] |= mask;
        } else {
            self.bytes[byte_offset] &= !mask;
        }
    }

    fn bitmap_location(&self, block: u16) -> (usize, u8) {
        let header_offset = block_offset(VOLUME_DIRECTORY_KEY_BLOCK) + 4;
        let bitmap_key_block = read_u16(&self.bytes, header_offset + 0x23);
        let bitmap_byte = usize::from(block) / 8;
        let offset = block_offset(bitmap_key_block) + bitmap_byte;
        let mask = 0x80 >> (block % 8);
        (offset, mask)
    }

    fn write_to(&self, path: &Path, create_new: bool) -> Result<()> {
        let temporary = temporary_path(path);
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|source| A2FuseError::WriteImage {
                    path: temporary.clone(),
                    source,
                })?;
            file.write_all(&self.bytes)
                .and_then(|_| file.sync_all())
                .map_err(|source| A2FuseError::WriteImage {
                    path: temporary.clone(),
                    source,
                })?;
            if create_new && path.exists() {
                return Err(A2FuseError::ImageExists(path.to_path_buf()));
            }
            std::fs::rename(&temporary, path).map_err(|source| A2FuseError::WriteImage {
                path: path.to_path_buf(),
                source,
            })
        })();
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        write_result
    }
}

#[derive(Clone, Debug)]
struct EncodedName {
    bytes: Vec<u8>,
    case_bits: u16,
}

fn encode_name(name: &str) -> Result<EncodedName> {
    if name.is_empty() || name.len() > 15 {
        return Err(A2FuseError::InvalidName {
            name: name.to_owned(),
            reason: "names must contain between 1 and 15 ASCII characters".to_owned(),
        });
    }
    let mut case_bits = 0x8000;
    let mut bytes = Vec::with_capacity(name.len());
    for (index, character) in name.bytes().enumerate() {
        let valid = if index == 0 {
            character.is_ascii_alphabetic()
        } else {
            character.is_ascii_alphanumeric() || character == b'.'
        };
        if !valid {
            return Err(A2FuseError::InvalidName {
                name: name.to_owned(),
                reason: "use a leading letter followed by ASCII letters, digits, or periods"
                    .to_owned(),
            });
        }
        if character.is_ascii_lowercase() {
            case_bits |= 1 << (14 - index);
        }
        bytes.push(character.to_ascii_uppercase());
    }
    Ok(EncodedName { bytes, case_bits })
}

fn encode_file_entry(
    name: &EncodedName,
    storage_type: StorageType,
    options: &PutOptions,
    key_pointer: u16,
    blocks_used: u16,
    eof: u32,
) -> [u8; PRODOS_ENTRY_LENGTH] {
    let mut entry = [0_u8; PRODOS_ENTRY_LENGTH];
    entry[0] = (storage_type as u8) << 4 | name.bytes.len() as u8;
    entry[1..1 + name.bytes.len()].copy_from_slice(&name.bytes);
    entry[0x10] = options.file_type;
    put_u16(&mut entry, 0x11, key_pointer);
    put_u16(&mut entry, 0x13, blocks_used);
    put_u24(&mut entry, 0x15, eof);
    put_u16(&mut entry, 0x1c, name.case_bits);
    entry[0x1e] = options.access.0;
    put_u16(&mut entry, 0x1f, options.aux_type);
    put_u16(&mut entry, 0x25, VOLUME_DIRECTORY_KEY_BLOCK);
    entry
}

fn storage_type_for_blocks(block_count: usize) -> StorageType {
    match block_count {
        0..=1 => StorageType::Seedling,
        2..=256 => StorageType::Sapling,
        _ => StorageType::Tree,
    }
}

fn bitmap_block_count(blocks: u16) -> Result<u16> {
    if blocks == 0 {
        return Err(A2FuseError::InvalidVolumeSize(
            "a volume cannot contain zero blocks".to_owned(),
        ));
    }
    Ok(blocks.div_ceil(4096))
}

fn block_offset(block: u16) -> usize {
    usize::from(block) * BLOCK_SIZE
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u24(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset] = value as u8;
    bytes[offset + 1] = (value >> 8) as u8;
    bytes[offset + 2] = (value >> 16) as u8;
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| "a2fuse".into(), |name| name.to_os_string());
    name.push(format!(".tmp-{}", std::process::id()));
    path.with_file_name(name)
}

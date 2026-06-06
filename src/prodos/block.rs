use std::path::Path;
use std::sync::Arc;

use crate::error::{A2FuseError, Result};

pub const BLOCK_SIZE: usize = 512;

#[derive(Clone, Debug)]
pub struct BlockDevice {
    bytes: Arc<[u8]>,
}

impl BlockDevice {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| A2FuseError::ReadImage {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self> {
        let bytes = bytes.into();
        if bytes.len() % BLOCK_SIZE != 0 {
            return Err(A2FuseError::InvalidImageLength {
                length: bytes.len(),
            });
        }
        Ok(Self {
            bytes: bytes.into(),
        })
    }

    pub fn block_count(&self) -> usize {
        self.bytes.len() / BLOCK_SIZE
    }

    pub fn read_block(&self, block: u16) -> Result<&[u8; BLOCK_SIZE]> {
        let start = usize::from(block) * BLOCK_SIZE;
        let end = start + BLOCK_SIZE;
        let bytes = self
            .bytes
            .get(start..end)
            .ok_or(A2FuseError::BlockOutOfRange {
                block,
                block_count: self.block_count(),
            })?;
        Ok(bytes.try_into().expect("a block slice is always 512 bytes"))
    }
}

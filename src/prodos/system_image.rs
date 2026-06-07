use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{A2FuseError, Result};

use super::block::{BLOCK_SIZE, BlockDevice};
use super::path::MetadataMode;
use super::types::AccessFlags;
use super::volume::Volume;

pub const PRODOS_RELEASE_URL: &str =
    "https://raw.githubusercontent.com/ProDOS-8/ProDOS8-Releases/master/ProDOS_2_4_3.po";
pub const PRODOS_CACHE_FILENAME: &str = "ProDOS_2_4_3.po";
pub const PRODOS_BOOT_BLOCK_BYTES: usize = BLOCK_SIZE * 2;
pub const PRODOS_2_4_3_SHA256: &str =
    "398d333cb2ab92df9f8bb2cf64b946f2567116910eb8359cf4bdee5d4194f0fa";

#[derive(Clone, Debug)]
pub struct BootComponents {
    pub boot_blocks: Vec<u8>,
    pub prodos_system: BootFile,
    pub basic_system: BootFile,
}

#[derive(Clone, Debug)]
pub struct BootFile {
    pub data: Vec<u8>,
    pub file_type: u8,
    pub aux_type: u16,
    pub access: AccessFlags,
}

pub fn ensure_cached_prodos(force: bool, cache_dir: Option<&Path>) -> Result<PathBuf> {
    let cache_dir = cache_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(default_cache_directory);
    std::fs::create_dir_all(&cache_dir).map_err(|source| A2FuseError::CreateCacheDirectory {
        path: cache_dir.clone(),
        source,
    })?;

    let cached = cache_dir.join(PRODOS_CACHE_FILENAME);
    if cached.exists() && !force {
        verify_sha256(&cached)?;
        read_boot_components(&cached)?;
        return Ok(cached);
    }

    let temporary = cache_dir.join(format!(
        "{PRODOS_CACHE_FILENAME}.tmp-{}",
        std::process::id()
    ));
    let download_result = (|| {
        let response =
            ureq::get(PRODOS_RELEASE_URL)
                .call()
                .map_err(|error| A2FuseError::Download {
                    url: PRODOS_RELEASE_URL.to_owned(),
                    reason: error.to_string(),
                })?;
        let mut reader = response.into_reader();
        let mut output = File::create(&temporary).map_err(|source| A2FuseError::WriteImage {
            path: temporary.clone(),
            source,
        })?;
        std::io::copy(&mut reader, &mut output).map_err(|source| A2FuseError::WriteImage {
            path: temporary.clone(),
            source,
        })?;
        output.flush().map_err(|source| A2FuseError::WriteImage {
            path: temporary.clone(),
            source,
        })?;

        verify_sha256(&temporary)?;
        read_boot_components(&temporary)?;
        std::fs::rename(&temporary, &cached).map_err(|source| A2FuseError::WriteImage {
            path: cached.clone(),
            source,
        })?;
        Ok(cached.clone())
    })();

    if download_result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    download_result
}

pub fn read_boot_components(path: impl AsRef<Path>) -> Result<BootComponents> {
    let path = path.as_ref();
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|source| A2FuseError::ReadImage {
            path: path.to_path_buf(),
            source,
        })?
        .read_to_end(&mut bytes)
        .map_err(|source| A2FuseError::ReadImage {
            path: path.to_path_buf(),
            source,
        })?;

    if bytes.len() < PRODOS_BOOT_BLOCK_BYTES {
        return Err(A2FuseError::InvalidBootBlocks(format!(
            "{} is only {} bytes; at least {PRODOS_BOOT_BLOCK_BYTES} are required",
            path.display(),
            bytes.len()
        )));
    }

    let volume = Volume::from_device(BlockDevice::from_bytes(bytes.clone())?)?;
    let prodos_node = volume.find("PRODOS", MetadataMode::Xattr)?;
    let basic_node = volume.find("BASIC.SYSTEM", MetadataMode::Xattr)?;
    Ok(BootComponents {
        boot_blocks: bytes[..PRODOS_BOOT_BLOCK_BYTES].to_vec(),
        prodos_system: BootFile {
            data: volume.read_entry(&prodos_node.entry)?,
            file_type: prodos_node.entry.file_type,
            aux_type: prodos_node.entry.aux_type,
            access: prodos_node.entry.access,
        },
        basic_system: BootFile {
            data: volume.read_entry(&basic_node.entry)?,
            file_type: basic_node.entry.file_type,
            aux_type: basic_node.entry.aux_type,
            access: basic_node.entry.access,
        },
    })
}

fn default_cache_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(path).join("a2fuse");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("a2fuse");
    }
    std::env::temp_dir().join("a2fuse-cache")
}

fn verify_sha256(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|source| A2FuseError::ReadImage {
        path: path.to_path_buf(),
        source,
    })?;
    let digest = Sha256::digest(&bytes);
    let actual = format!("{digest:x}");
    if actual != PRODOS_2_4_3_SHA256 {
        return Err(A2FuseError::Download {
            url: PRODOS_RELEASE_URL.to_owned(),
            reason: format!(
                "cached ProDOS image hash mismatch: expected {PRODOS_2_4_3_SHA256}, got {actual}"
            ),
        });
    }
    Ok(())
}

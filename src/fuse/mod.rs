use std::path::Path;

use crate::error::{A2FuseError, Result};
use crate::prodos::{MetadataMode, Volume};

#[cfg(feature = "macfuse")]
mod attrs;
#[cfg(feature = "macfuse")]
mod fs;
#[cfg(feature = "macfuse")]
mod inode;

#[cfg(feature = "macfuse")]
pub use fs::ReadOnlyFilesystem;

#[cfg(feature = "macfuse")]
pub fn spawn_mount(
    volume: Volume,
    mountpoint: &Path,
    metadata_mode: MetadataMode,
) -> Result<fuser::BackgroundSession> {
    use fuser::{Config, MountOption};

    let filesystem = ReadOnlyFilesystem::new(volume, metadata_mode);
    let mut config = Config::default();
    config.mount_options = vec![
        MountOption::RO,
        MountOption::FSName("a2fuse".to_owned()),
        MountOption::Subtype("prodos".to_owned()),
        MountOption::NoDev,
        MountOption::NoSuid,
        MountOption::NoExec,
        MountOption::NoAtime,
    ];
    fuser::spawn_mount2(filesystem, mountpoint, &config)
        .map_err(|error| A2FuseError::Fuse(error.to_string()))
}

#[cfg(not(feature = "macfuse"))]
pub fn spawn_mount(
    _volume: Volume,
    _mountpoint: &Path,
    _metadata_mode: MetadataMode,
) -> Result<()> {
    Err(A2FuseError::FuseDisabled)
}

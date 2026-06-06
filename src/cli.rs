use std::path::PathBuf;

use clap::Parser;

use a2fuse::prodos::MetadataMode;

#[derive(Debug, Parser)]
#[command(
    name = "a2fuse",
    version,
    about = "Mount a ProDOS disk image as a read-only filesystem"
)]
pub struct Cli {
    /// Explicitly request a read-only mount (currently always enabled).
    #[arg(long)]
    pub readonly: bool,

    /// Enable debug logging.
    #[arg(long)]
    pub debug: bool,

    /// Choose how ProDOS metadata is exposed.
    #[arg(long, value_enum, default_value_t = MetadataMode::Xattr)]
    pub metadata: MetadataMode,

    /// ProDOS-order disk image.
    pub image: PathBuf,

    /// Existing directory on which to mount the image.
    pub mountpoint: PathBuf,
}

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::prodos::MetadataMode;

#[derive(Debug, Parser)]
#[command(
    name = "a2fuse",
    version,
    about = "Mount and maintain Apple II ProDOS disk images",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Enable debug logging.
    #[arg(long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Command>,

    /// Explicitly request a read-only mount (mounts are always read-only).
    #[arg(long)]
    pub readonly: bool,

    /// Choose how mounted ProDOS metadata is exposed.
    #[arg(long, value_enum, default_value_t = MetadataMode::Xattr)]
    pub metadata: MetadataMode,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Mount an image as a read-only filesystem.
    Mount(MountArgs),

    /// Create an empty ProDOS-order disk image.
    Create(CreateArgs),

    /// List files using Unix-style output.
    Ls(ListArgs),

    /// Display an Apple II-style ProDOS catalogue.
    Catalog(CatalogArgs),

    /// Copy a file from the image to the host.
    Get(GetArgs),

    /// Copy a host file into the image root directory.
    #[command(visible_alias = "add")]
    Put(PutArgs),
}

#[derive(Debug, Args)]
pub struct MountArgs {
    pub image: PathBuf,
    pub mountpoint: PathBuf,

    /// Explicitly request a read-only mount (mounts are always read-only).
    #[arg(long)]
    pub readonly: bool,

    /// Choose how ProDOS metadata is exposed.
    #[arg(long, value_enum, default_value_t = MetadataMode::Xattr)]
    pub metadata: MetadataMode,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Destination `.po` image.
    pub image: PathBuf,

    /// ProDOS volume name.
    #[arg(long)]
    pub name: String,

    /// Image size in 512-byte ProDOS blocks.
    #[arg(long, default_value_t = 280)]
    pub blocks: u16,

    /// Replace an existing destination image.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    pub image: PathBuf,

    /// Directory or file path inside the image.
    pub path: Option<String>,

    /// Use a Unix-style long listing.
    #[arg(short, long)]
    pub long: bool,
}

#[derive(Debug, Args)]
pub struct CatalogArgs {
    pub image: PathBuf,

    /// Directory or file path inside the image.
    pub path: Option<String>,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub image: PathBuf,

    /// Source path inside the image.
    pub source: String,

    /// Host destination; defaults to the ProDOS filename. Use `-` for stdout.
    pub destination: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PutArgs {
    pub image: PathBuf,
    pub source: PathBuf,

    /// Destination filename in the image root; defaults to the host filename.
    pub destination: Option<String>,

    /// ProDOS file type, in decimal, `0xNN`, or `$NN` form.
    #[arg(long = "type", default_value = "0x06", value_parser = parse_u8)]
    pub file_type: u8,

    /// ProDOS auxiliary type, in decimal, `0xNNNN`, or `$NNNN` form.
    #[arg(long, default_value = "0", value_parser = parse_u16)]
    pub aux_type: u16,
}

fn parse_u8(value: &str) -> std::result::Result<u8, String> {
    parse_number(value).and_then(|number| {
        u8::try_from(number).map_err(|_| format!("{value:?} does not fit in one byte"))
    })
}

fn parse_u16(value: &str) -> std::result::Result<u16, String> {
    parse_number(value).and_then(|number| {
        u16::try_from(number).map_err(|_| format!("{value:?} does not fit in two bytes"))
    })
}

fn parse_number(value: &str) -> std::result::Result<u64, String> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .or_else(|| value.strip_prefix('$'))
    {
        u64::from_str_radix(hex, 16).map_err(|error| error.to_string())
    } else {
        value.parse::<u64>().map_err(|error| error.to_string())
    }
}

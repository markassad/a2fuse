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

    /// Download and cache the upstream ProDOS system image.
    FetchProdos(FetchProdosArgs),

    /// List files using Unix-style output.
    Ls(ListArgs),

    /// Display an Apple II-style ProDOS catalogue.
    Catalog(CatalogArgs),

    /// Copy a file from the image to the host.
    Get(GetArgs),

    /// Extract and untokenize an AppleSoft BASIC file from the image.
    BasicGet(BasicGetArgs),

    /// Create a directory inside an image.
    Mkdir(MkdirArgs),

    /// Remove a regular file from an image.
    Rm(RmArgs),

    /// Copy a host file into the image.
    #[command(visible_alias = "add")]
    Put(PutArgs),

    /// Tokenize and import an AppleSoft BASIC file into the image.
    BasicPut(BasicPutArgs),
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

    /// Install boot blocks plus `PRODOS` and `BASIC.SYSTEM` from a cached upstream image.
    #[arg(long)]
    pub bootable: bool,

    /// Cache directory for downloaded ProDOS system images.
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct FetchProdosArgs {
    /// Replace any existing cached copy.
    #[arg(long)]
    pub force: bool,

    /// Cache directory for downloaded ProDOS system images.
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
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
pub struct BasicGetArgs {
    pub image: PathBuf,

    /// Source path inside the image.
    pub source: String,

    /// Host destination; defaults to `<SOURCE>.txt`. Use `-` for stdout.
    pub destination: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MkdirArgs {
    pub image: PathBuf,

    /// Directory path inside the image.
    pub path: String,

    /// Create missing parent directories and accept existing directories.
    #[arg(short, long)]
    pub parents: bool,
}

#[derive(Debug, Args)]
pub struct RmArgs {
    pub image: PathBuf,

    /// File path inside the image.
    pub path: String,
}

#[derive(Debug, Args)]
pub struct PutArgs {
    pub image: PathBuf,
    /// Host source file. Use `-` to read bytes from stdin.
    pub source: PathBuf,

    /// Destination path in the image; defaults to the host filename in the root.
    pub destination: Option<String>,

    /// ProDOS file type, in decimal, `0xNN`, or `$NN` form.
    #[arg(long = "type", default_value = "0x06", value_parser = parse_u8)]
    pub file_type: u8,

    /// ProDOS auxiliary type, in decimal, `0xNNNN`, or `$NNNN` form.
    #[arg(long, default_value = "0", value_parser = parse_u16)]
    pub aux_type: u16,

    /// Replace an existing file at the destination path.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct BasicPutArgs {
    pub image: PathBuf,
    /// Host BASIC text file. Use `-` to read text from stdin.
    pub source: PathBuf,

    /// Destination path in the image.
    pub destination: String,

    /// ProDOS auxiliary type, in decimal, `0xNNNN`, or `$NNNN` form.
    #[arg(long, default_value = "0x0801", value_parser = parse_u16)]
    pub aux_type: u16,

    /// Replace an existing file at the destination path.
    #[arg(long)]
    pub force: bool,
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

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum A2FuseError {
    #[error("could not read disk image {path}: {source}")]
    ReadImage {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write disk image {path}: {source}")]
    WriteImage {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read host file {path}: {source}")]
    ReadHostFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("disk image already exists: {0}")]
    ImageExists(PathBuf),

    #[error("disk image length {length} is not a multiple of the ProDOS block size")]
    InvalidImageLength { length: usize },

    #[error("block {block} is outside the image ({block_count} blocks)")]
    BlockOutOfRange { block: u16, block_count: usize },

    #[error("invalid ProDOS volume: {0}")]
    InvalidVolume(String),

    #[error("invalid ProDOS directory: {0}")]
    InvalidDirectory(String),

    #[error("invalid ProDOS directory entry: {0}")]
    InvalidDirectoryEntry(String),

    #[error("unsupported ProDOS storage type {storage_type:#x} for {name}")]
    UnsupportedStorageType { storage_type: u8, name: String },

    #[error("path not found in image: {0}")]
    PathNotFound(String),

    #[error("path is not a regular file: {0}")]
    NotAFile(String),

    #[error("path is not a directory: {0}")]
    NotADirectory(String),

    #[error("invalid ProDOS name {name:?}: {reason}")]
    InvalidName { name: String, reason: String },

    #[error("file already exists in image: {0}")]
    FileExists(String),

    #[error("the ProDOS volume directory is full")]
    DirectoryFull,

    #[error("the ProDOS volume does not have enough free blocks")]
    DiskFull,

    #[error("file is too large for ProDOS: {size} bytes")]
    FileTooLarge { size: usize },

    #[error("invalid image size: {0}")]
    InvalidVolumeSize(String),

    #[error("could not write command output: {0}")]
    Output(#[source] std::io::Error),

    #[error("FUSE support was not compiled in; rebuild with --features macfuse")]
    FuseDisabled,

    #[error("FUSE mount failed: {0}")]
    Fuse(String),
}

pub type Result<T> = std::result::Result<T, A2FuseError>;

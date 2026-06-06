mod cli;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use a2fuse::error::Result;
use a2fuse::prodos::Volume;
use cli::Cli;

fn main() {
    if let Err(error) = run() {
        eprintln!("a2fuse: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let default_filter = if cli.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_target(false)
        .init();

    if cli.readonly {
        tracing::debug!("read-only mode explicitly requested");
    }
    let volume = Volume::open(&cli.image)?;
    tracing::info!(
        image = %cli.image.display(),
        volume = %volume.header.name,
        files = volume.header.file_count,
        "opened ProDOS image"
    );
    a2fuse::fuse::mount(volume, &cli.mountpoint, cli.metadata)
}

use std::io::Write;
use std::path::Path;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use a2fuse::cli::{Cli, Command, FetchProdosArgs, MountArgs};
use a2fuse::error::{A2FuseError, Result};
use a2fuse::prodos::{
    AccessFlags, CreateOptions, Image, MetadataMode, MkdirOptions, Node, ProdosTimestamp,
    PutOptions, Volume, ensure_cached_prodos, read_boot_components,
};

#[cfg(feature = "macfuse")]
use std::sync::mpsc;

fn main() {
    if let Err(error) = run() {
        eprintln!("a2fuse: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    initialise_tracing(cli.debug);

    match cli.command {
        Some(Command::Mount(args)) => mount(args),
        Some(Command::Create(args)) => {
            let mut image = Image::create(&CreateOptions {
                name: args.name,
                blocks: args.blocks,
            })?;
            if args.bootable {
                let cached = ensure_cached_prodos(false, args.cache_dir.as_deref())?;
                let components = read_boot_components(&cached)?;
                image.install_bootable_components(
                    &components.boot_blocks,
                    &components.prodos_system,
                    &components.basic_system,
                )?;
            }
            if args.force {
                image.save(&args.image)
            } else {
                image.save_new(&args.image)
            }
        }
        Some(Command::FetchProdos(args)) => fetch_prodos(args),
        Some(Command::Ls(args)) => list(&args.image, args.path.as_deref(), args.long),
        Some(Command::Catalog(args)) => catalog(&args.image, args.path.as_deref()),
        Some(Command::Get(args)) => get(&args.image, &args.source, args.destination.as_deref()),
        Some(Command::Mkdir(args)) => {
            let mut image = Image::open(&args.image)?;
            let mut options = MkdirOptions::new(args.path);
            options.parents = args.parents;
            image.create_directory(&options)?;
            image.save(&args.image)
        }

        Some(Command::Put(args)) => {
            let data = std::fs::read(&args.source).map_err(|source| A2FuseError::ReadHostFile {
                path: args.source.clone(),
                source,
            })?;
            let destination = match args.destination {
                Some(destination) => destination,
                None => args
                    .source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| A2FuseError::InvalidName {
                        name: args.source.display().to_string(),
                        reason: "the host filename is not valid UTF-8".to_owned(),
                    })?
                    .to_owned(),
            };
            let mut image = Image::open(&args.image)?;
            let mut options = PutOptions::new(destination);
            options.file_type = args.file_type;
            options.aux_type = args.aux_type;
            options.access = AccessFlags(0xe3);
            image.put_file(&data, &options)?;
            image.save(&args.image)
        }

        None => Err(A2FuseError::Fuse(
            "a subcommand is required; use `a2fuse mount IMAGE MOUNTPOINT`".to_owned(),
        )),
    }
}

fn fetch_prodos(args: FetchProdosArgs) -> Result<()> {
    let path = ensure_cached_prodos(args.force, args.cache_dir.as_deref())?;
    println!("{}", path.display());
    Ok(())
}

fn initialise_tracing(debug: bool) {
    let default_filter = if debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_target(false)
        .init();
}

fn mount(args: MountArgs) -> Result<()> {
    if args.readonly {
        tracing::debug!("read-only mode explicitly requested");
    }
    let volume = Volume::open(&args.image)?;
    tracing::info!(
        image = %args.image.display(),
        volume = %volume.header.name,
        files = volume.header.file_count,
        "opened ProDOS image"
    );
    #[cfg(feature = "macfuse")]
    {
        let session = a2fuse::fuse::spawn_mount(volume, &args.mountpoint, args.metadata)?;
        wait_for_shutdown_signal()?;
        session
            .umount_and_join()
            .map_err(|error| A2FuseError::Fuse(error.to_string()))
    }

    #[cfg(not(feature = "macfuse"))]
    {
        let _ = (volume, args);
        Err(A2FuseError::FuseDisabled)
    }
}

#[cfg(feature = "macfuse")]
fn wait_for_shutdown_signal() -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = sender.send(());
    })
    .map_err(|error| A2FuseError::Fuse(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| A2FuseError::Fuse(error.to_string()))?;
    Ok(())
}

fn list(image: &Path, path: Option<&str>, long: bool) -> Result<()> {
    let volume = Volume::open(image)?;
    let nodes = nodes_at_path(&volume, path)?;
    for node in nodes {
        print_unix_node(node, long);
    }
    Ok(())
}

fn catalog(image: &Path, path: Option<&str>) -> Result<()> {
    let volume = Volume::open(image)?;
    let nodes = nodes_at_path(&volume, path)?;
    let catalogue_path = path
        .filter(|path| !path.is_empty() && *path != "/")
        .map_or_else(
            || format!("/{}", volume.header.name),
            |path| format!("/{}/{}", volume.header.name, path.trim_matches('/')),
        );

    println!("{catalogue_path}");
    println!();
    println!(" NAME            TYPE BLOCKS  MODIFIED         CREATED          ENDFILE  SUBTYPE");
    for node in nodes {
        print_catalogue_node(node);
    }
    println!();
    println!("{} FILE(S)", nodes.len());
    Ok(())
}

fn nodes_at_path<'a>(volume: &'a Volume, path: Option<&str>) -> Result<&'a [Node]> {
    match path {
        None | Some("") | Some("/") => Ok(&volume.root),
        Some(path) => {
            let node = volume.find(path, MetadataMode::Xattr)?;
            if node.is_directory() {
                Ok(&node.children)
            } else {
                Ok(std::slice::from_ref(node))
            }
        }
    }
}

fn print_unix_node(node: &Node, long: bool) {
    if long {
        let permissions = unix_permissions(node);
        let modified = format_unix_timestamp(node.entry.modification);
        let links = if node.is_directory() { 2 } else { 1 };
        println!(
            "{permissions} {links:>2} {:<6} {:<6} {:>10} {modified} {}",
            "prodos",
            "prodos",
            node.effective_eof(),
            node.entry.name
        );
    } else {
        println!("{}", node.entry.name);
    }
}

fn print_catalogue_node(node: &Node) {
    println!(
        " {:<15} {:>4} {:>6}  {:<16} {:<16} {:>8}  ${:04X}",
        node.entry.name,
        prodos_type_name(node),
        node.effective_blocks_used(),
        format_catalogue_timestamp(node.entry.modification),
        format_catalogue_timestamp(node.entry.creation),
        node.effective_eof(),
        node.entry.aux_type
    );
}

fn unix_permissions(node: &Node) -> String {
    let kind = if node.is_directory() { 'd' } else { '-' };
    let read = if node.entry.access.readable() {
        'r'
    } else {
        '-'
    };
    let write = if node.entry.access.writable() {
        'w'
    } else {
        '-'
    };
    let execute = if node.is_directory() { 'x' } else { '-' };
    format!("{kind}{read}{write}{execute}{read}-{execute}{read}-{execute}")
}

fn format_unix_timestamp(timestamp: Option<ProdosTimestamp>) -> String {
    timestamp.map_or_else(
        || "---- -- -- --:--".to_owned(),
        |timestamp| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                timestamp.year, timestamp.month, timestamp.day, timestamp.hour, timestamp.minute
            )
        },
    )
}

fn format_catalogue_timestamp(timestamp: Option<ProdosTimestamp>) -> String {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    timestamp.map_or_else(
        || "<NO DATE>".to_owned(),
        |timestamp| {
            let month = MONTHS
                .get(usize::from(timestamp.month.saturating_sub(1)))
                .copied()
                .unwrap_or("???");
            format!(
                "{:02}-{month}-{:02} {:02}:{:02}",
                timestamp.day,
                timestamp.year % 100,
                timestamp.hour,
                timestamp.minute
            )
        },
    )
}

fn prodos_type_name(node: &Node) -> String {
    if node.is_directory() {
        return "DIR".to_owned();
    }
    match node.entry.file_type {
        0x00 => "NON".to_owned(),
        0x04 => "TXT".to_owned(),
        0x06 => "BIN".to_owned(),
        0xfc => "BAS".to_owned(),
        0xfd => "VAR".to_owned(),
        0xfe => "REL".to_owned(),
        0xff => "SYS".to_owned(),
        file_type => format!("${file_type:02X}"),
    }
}

fn get(image: &Path, source: &str, destination: Option<&Path>) -> Result<()> {
    let volume = Volume::open(image)?;
    let node = volume.find(source, MetadataMode::Xattr)?;
    let data = volume.read_entry(&node.entry)?;
    let destination = destination
        .map(Path::to_path_buf)
        .unwrap_or_else(|| Path::new(&node.entry.name).to_path_buf());

    if destination == Path::new("-") {
        std::io::stdout()
            .lock()
            .write_all(&data)
            .map_err(A2FuseError::Output)
    } else {
        std::fs::write(&destination, data).map_err(|source| A2FuseError::WriteHostFile {
            path: destination,
            source,
        })
    }
}

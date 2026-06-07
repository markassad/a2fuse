use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;

use a2fuse::cli::Cli;

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

#[test]
fn creates_imports_lists_and_reads_an_image() {
    let directory = temporary_directory();
    let image = directory.join("test.po");
    let source = directory.join("hello.txt");
    fs::write(&source, b"Hello from ProDOS\n").unwrap();

    command()
        .args(["create", image.to_str().unwrap(), "--name", "TestDisk"])
        .assert_success();
    command()
        .args([
            "put",
            image.to_str().unwrap(),
            source.to_str().unwrap(),
            "Hello",
            "--type",
            "$04",
            "--aux-type",
            "$0000",
        ])
        .assert_success();
    command()
        .args([
            "mkdir",
            image.to_str().unwrap(),
            "Games/Arcade",
            "--parents",
        ])
        .assert_success();
    command()
        .args([
            "put",
            image.to_str().unwrap(),
            source.to_str().unwrap(),
            "Games/Arcade/Nested",
            "--type",
            "$04",
        ])
        .assert_success();

    let listing = command()
        .args(["ls", image.to_str().unwrap(), "--long"])
        .output()
        .unwrap();
    assert!(listing.status.success());
    let listing = String::from_utf8(listing.stdout).unwrap();
    assert_eq!(
        listing,
        concat!(
            "-rw-r--r--  1 prodos prodos         18 ---- -- -- --:-- Hello\n",
            "drwxr-xr-x  2 prodos prodos        512 ---- -- -- --:-- Games\n",
        )
    );

    let catalogue = command()
        .args(["catalog", image.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(catalogue.status.success());
    let catalogue = String::from_utf8(catalogue.stdout).unwrap();
    assert!(catalogue.starts_with("/TestDisk\n\n NAME"));
    assert!(catalogue.contains("Hello            TXT"));
    assert!(catalogue.contains("Games            DIR"));
    assert!(catalogue.contains("<NO DATE>"));
    assert!(catalogue.contains("      18  $0000"));
    assert!(catalogue.ends_with("\n2 FILE(S)\n"));

    let nested_listing = command()
        .args(["ls", image.to_str().unwrap(), "Games/Arcade"])
        .output()
        .unwrap();
    assert!(nested_listing.status.success());
    assert_eq!(nested_listing.stdout, b"Nested\n");

    let nested_extracted = directory.join("nested.txt");
    command()
        .args([
            "get",
            image.to_str().unwrap(),
            "Games/Arcade/Nested",
            nested_extracted.to_str().unwrap(),
        ])
        .assert_success();
    assert_eq!(fs::read(&nested_extracted).unwrap(), b"Hello from ProDOS\n");

    let extracted = directory.join("extracted.txt");
    command()
        .args([
            "get",
            image.to_str().unwrap(),
            "Hello",
            extracted.to_str().unwrap(),
        ])
        .assert_success();
    assert_eq!(fs::read(&extracted).unwrap(), b"Hello from ProDOS\n");

    let mut default_get = command();
    default_get
        .current_dir(&directory)
        .args(["get", image.to_str().unwrap(), "Hello"])
        .assert_success();
    assert_eq!(
        fs::read(directory.join("Hello")).unwrap(),
        b"Hello from ProDOS\n"
    );

    let output = command()
        .args(["get", image.to_str().unwrap(), "Hello", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Hello from ProDOS\n");

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn mount_requires_the_subcommand() {
    let result = Cli::try_parse_from(["a2fuse", "image.po", "/mnt/apple2"]);
    assert!(result.is_err());
}

#[test]
fn mount_parses_with_the_subcommand() {
    let cli = Cli::try_parse_from(["a2fuse", "mount", "image.po", "/mnt/apple2"]).unwrap();
    assert!(matches!(cli.command, Some(a2fuse::cli::Command::Mount(_))));
}

#[test]
fn create_parses_bootable_flags() {
    let cli = Cli::try_parse_from([
        "a2fuse",
        "create",
        "image.po",
        "--name",
        "TEST",
        "--bootable",
        "--cache-dir",
        "/tmp/a2fuse-cache",
    ])
    .unwrap();
    assert!(matches!(cli.command, Some(a2fuse::cli::Command::Create(_))));
}

#[test]
fn fetch_prodos_parses() {
    let cli = Cli::try_parse_from([
        "a2fuse",
        "fetch-prodos",
        "--force",
        "--cache-dir",
        "/tmp/a2fuse-cache",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(a2fuse::cli::Command::FetchProdos(_))
    ));
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_a2fuse"))
}

fn temporary_directory() -> std::path::PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("a2fuse-cli-test-{}-{sequence}", std::process::id()));
    fs::create_dir(&path).unwrap();
    path
}

trait CommandAssertion {
    fn assert_success(&mut self);
}

impl CommandAssertion for Command {
    fn assert_success(&mut self) {
        let output = self.output().unwrap();
        assert!(
            output.status.success(),
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

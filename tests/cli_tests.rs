use std::fs;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
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

    command()
        .args(["rm", image.to_str().unwrap(), "Hello"])
        .assert_success();
    let after_rm = command()
        .args(["ls", image.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(after_rm.status.success());
    let listing = String::from_utf8(after_rm.stdout).unwrap();
    assert_eq!(listing, "Games\n");

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn basic_put_force_replaces_existing_file() {
    let directory = temporary_directory();
    let image = directory.join("replace.po");
    let first = directory.join("first.bas");
    let second = directory.join("second.bas");
    fs::write(&first, "10 PRINT \"ONE\"\n20 END\n").unwrap();
    fs::write(&second, "10 PRINT \"TWO\"\n20 END\n").unwrap();

    command()
        .args(["create", image.to_str().unwrap(), "--name", "REPL"])
        .assert_success();
    command()
        .args([
            "basic-put",
            image.to_str().unwrap(),
            first.to_str().unwrap(),
            "GAME",
        ])
        .assert_success();
    command()
        .args([
            "basic-put",
            image.to_str().unwrap(),
            second.to_str().unwrap(),
            "GAME",
            "--force",
        ])
        .assert_success();

    let output = command()
        .args(["basic-get", image.to_str().unwrap(), "GAME", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"10 PRINT \"TWO\"\n20 END\n");

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn imports_and_extracts_applesoft_basic_as_text() {
    let directory = temporary_directory();
    let image = directory.join("basic.po");
    let source = directory.join("program.txt");
    fs::write(&source, "10 PRINT \"HELLO\"\n20 GOTO 10\n").unwrap();

    command()
        .args(["create", image.to_str().unwrap(), "--name", "BASIC"])
        .assert_success();
    command()
        .args([
            "basic-put",
            image.to_str().unwrap(),
            source.to_str().unwrap(),
            "HELLO",
        ])
        .assert_success();

    let basic_stdout = command()
        .args(["basic-get", image.to_str().unwrap(), "HELLO", "-"])
        .output()
        .unwrap();
    assert!(basic_stdout.status.success());
    assert_eq!(basic_stdout.stdout, b"10 PRINT \"HELLO\"\n20 GOTO 10\n");

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn basic_get_then_basic_put_preserves_compact_token_bytes() {
    let directory = temporary_directory();
    let image = directory.join("compact.po");
    let source = directory.join("startup.bas");
    let original = [
        0x0e, 0x08, 0x0a, 0x00, 0x81, b'I', 0xd0, b'1', 0xc1, b'1', b'0', b'0', 0x00, 0x15, 0x08,
        0x14, 0x00, 0xba, b'I', 0x00, 0x1c, 0x08, 0x1e, 0x00, 0x82, b'I', 0x00, 0x00, 0x00,
    ];
    fs::write(&source, original).unwrap();

    command()
        .args(["create", image.to_str().unwrap(), "--name", "COMPACT"])
        .assert_success();
    command()
        .args([
            "put",
            image.to_str().unwrap(),
            source.to_str().unwrap(),
            "STARTUP",
            "--type",
            "$FC",
            "--aux-type",
            "$0801",
        ])
        .assert_success();

    let mut put = command();
    put.args(["basic-put", image.to_str().unwrap(), "-", "START2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = put.spawn().unwrap();
    let startup_text = command()
        .args(["basic-get", image.to_str().unwrap(), "STARTUP", "-"])
        .output()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&startup_text.stdout)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let startup_bytes = command()
        .args(["get", image.to_str().unwrap(), "STARTUP", "-"])
        .output()
        .unwrap();
    let start2_bytes = command()
        .args(["get", image.to_str().unwrap(), "START2", "-"])
        .output()
        .unwrap();
    assert!(startup_bytes.status.success());
    assert!(start2_bytes.status.success());
    assert_eq!(startup_bytes.stdout, start2_bytes.stdout);

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

#[test]
fn basic_commands_parse() {
    let get_cli = Cli::try_parse_from(["a2fuse", "basic-get", "image.po", "HELLO", "-"]).unwrap();
    assert!(matches!(
        get_cli.command,
        Some(a2fuse::cli::Command::BasicGet(_))
    ));

    let put_cli = Cli::try_parse_from([
        "a2fuse",
        "basic-put",
        "image.po",
        "program.txt",
        "HELLO",
        "--force",
        "--aux-type",
        "$0801",
    ])
    .unwrap();
    assert!(matches!(
        put_cli.command,
        Some(a2fuse::cli::Command::BasicPut(_))
    ));

    let put_cli = Cli::try_parse_from([
        "a2fuse",
        "put",
        "image.po",
        "program.bin",
        "HELLO",
        "--force",
    ])
    .unwrap();
    assert!(matches!(
        put_cli.command,
        Some(a2fuse::cli::Command::Put(_))
    ));
}

#[test]
fn rm_parses() {
    let cli = Cli::try_parse_from(["a2fuse", "rm", "image.po", "HELLO"]).unwrap();
    assert!(matches!(cli.command, Some(a2fuse::cli::Command::Rm(_))));
}

#[test]
fn put_and_basic_put_accept_stdin() {
    let directory = temporary_directory();
    let image = directory.join("stdin.po");

    command()
        .args(["create", image.to_str().unwrap(), "--name", "STDIN"])
        .assert_success();

    let mut put = command();
    put.args([
        "put",
        image.to_str().unwrap(),
        "-",
        "FROMSTDIN",
        "--type",
        "$04",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::piped());
    let mut child = put.spawn().unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"hello stdin")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let get = command()
        .args(["get", image.to_str().unwrap(), "FROMSTDIN", "-"])
        .output()
        .unwrap();
    assert!(get.status.success());
    assert_eq!(get.stdout, b"hello stdin");

    let mut basic_put = command();
    basic_put
        .args(["basic-put", image.to_str().unwrap(), "-", "BASSTDIN"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = basic_put.spawn().unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"10 PRINT \"OK\"\n20 GOTO 10\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let get = command()
        .args(["basic-get", image.to_str().unwrap(), "BASSTDIN", "-"])
        .output()
        .unwrap();
    assert!(get.status.success());
    assert_eq!(get.stdout, b"10 PRINT \"OK\"\n20 GOTO 10\n");

    fs::remove_dir_all(directory).unwrap();
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

use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

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

    let listing = command()
        .args(["ls", image.to_str().unwrap(), "--long"])
        .output()
        .unwrap();
    assert!(listing.status.success());
    let listing = String::from_utf8(listing.stdout).unwrap();
    assert_eq!(
        listing,
        "-rw-r--r--  1 prodos prodos         18 ---- -- -- --:-- Hello\n"
    );

    let catalogue = command()
        .args(["catalog", image.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(catalogue.status.success());
    let catalogue = String::from_utf8(catalogue.stdout).unwrap();
    assert!(catalogue.starts_with("/TestDisk\n\n NAME"));
    assert!(catalogue.contains("Hello            TXT"));
    assert!(catalogue.contains("<NO DATE>"));
    assert!(catalogue.contains("      18  $0000"));
    assert!(catalogue.ends_with("\n1 FILE(S)\n"));

    let output = command()
        .args(["cat", image.to_str().unwrap(), "Hello"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Hello from ProDOS\n");

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

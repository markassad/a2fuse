# a2fuse

`a2fuse` is a Rust command-line toolkit for Apple II ProDOS disk images. It can
create and inspect `.po` images, import files, and mount images as a read-only
filesystem on macOS using macFUSE.

The project is deliberately library-first: ProDOS parsing and file block
allocation live independently of the CLI and FUSE adapter, so they can be
tested with artificial byte arrays and used by other tools.

## Project documents

- [Documentation index](docs/README.md)
- [Design](docs/design.md)
- [Supported ProDOS format](docs/prodos-format.md)
- [ProDOS format references](docs/prodos-references.md)
- [Roadmap](docs/roadmap.md)
- [Contributing](docs/development/contributing.md)

## Status

The mounted filesystem is read-only. The parser and mount foundation requested
for the first milestone is complete, and a small experimental offline image
maintenance layer is now under development.

The project currently:

- accepts ProDOS-order images made from 512-byte blocks, commonly named `.po`;
- creates empty ProDOS volumes;
- imports files into the volume root or existing subdirectories;
- lists files and extracts files to the host;
- parses the volume directory and chained subdirectories;
- exposes seedling, sapling, and tree files;
- preserves ProDOS file type, auxiliary type, access flags, storage type,
  timestamps, EOF, key pointer, and block usage internally;
- supports `getattr`, `lookup`, `readdir`, `open`, `read`, and `statfs`;
- returns read-only errors for common mutation operations;
- exposes metadata as read-only extended attributes, or as filename suffixes.

The FUSE layer never uses the offline mutation API. Mounts remain strictly
read-only.

Normal filenames do not contain metadata suffixes. In the default xattr mode,
regular entries expose:

```text
prodos.type
prodos.aux_type
prodos.access
prodos.storage_type
```

Image maintenance commands only require Rust. Mounting additionally requires
macOS and macFUSE.

## Mount Requirements

- macFUSE

Install macFUSE with Homebrew:

```sh
brew install --cask macfuse
```

macFUSE may require approval in System Settings after installation. Follow the
installer's instructions and restart macOS if requested. The upstream `fuser`
crate also documents `brew install macfuse pkgconf` for development setups.

## Build

```sh
cargo build
cargo test
```

Enable the repository's native pre-commit and pre-push checks once per clone:

```sh
./scripts/install-git-hooks.sh
```

See the [contributing guide](docs/development/contributing.md#git-hooks) for
the commands run by each hook.

The default build deliberately excludes the macFUSE runtime so parser tests do
not require a mounted or installed FUSE environment. Build the mountable binary
with:

```sh
cargo build --features macfuse
```

## Image Commands

Create a standard 140 KB, 280-block image:

```sh
cargo run -- create work.po --name WORK
```

Create a bootable image (downloads and caches the upstream ProDOS 2.4.3 image if needed, then copies boot blocks, `PRODOS`, and `BASIC.SYSTEM`):

```sh
cargo run -- create boot.po --name BOOT --bootable
```

Download or refresh the cached upstream ProDOS image explicitly:

```sh
cargo run -- fetch-prodos
cargo run -- fetch-prodos --force
```

By default, cached files are stored in:

- `$XDG_CACHE_HOME/a2fuse` when `XDG_CACHE_HOME` is set;
- otherwise `$HOME/.cache/a2fuse`;
- otherwise the platform temporary directory.

Choose another size in 512-byte blocks:

```sh
cargo run -- create archive.po --name ARCHIVE --blocks 1600
```

Import host files into the root directory or an existing subdirectory:

```sh
cargo run -- put work.po README.txt README --type '$04'
cargo run -- put work.po PROGRAM.BIN PROGRAM --type '$06' --aux-type '$2000'
cargo run -- mkdir --parents work.po GAMES/ARCADE
cargo run -- put work.po GAME.BIN GAMES/ARCADE/GAME --type '$06'
```

Create directories:

```sh
cargo run -- mkdir work.po GAMES
cargo run -- mkdir --parents work.po GAMES/ARCADE
```

Without `--parents`, every parent directory must already exist. With
`--parents`, missing parents are created and existing directories are accepted.

For `put`, the destination path defaults to the host filename in the volume
root. Parent directories in an explicit destination path must already exist.
ProDOS path components must contain 1 to 15 ASCII characters, begin with a
letter, and otherwise contain only letters, digits, or periods.

List or extract files:

```sh
cargo run -- ls work.po
cargo run -- ls work.po --long
cargo run -- catalog work.po
cargo run -- get work.po README README.txt
cargo run -- get work.po PROGRAM PROGRAM.BIN
cargo run -- get work.po GAMES/ARCADE/GAME GAME.BIN
cargo run -- get work.po README -
```

`ls` uses host-oriented Unix-style output. Its `--long` form shows permissions,
link count, synthetic owner and group names, byte size, and modification time.
`catalog` uses an Apple II-style ProDOS catalogue with file types, allocated
blocks, ProDOS timestamps, EOF, and auxiliary types.

When no destination is supplied, `get` uses the ProDOS filename in the current
directory. A destination of `-` writes the file to standard output.

`add` is an alias for `put`.

## macOS Mount Usage

```sh
mkdir -p ~/mnt/apple2
cargo run --features macfuse -- mount image.po ~/mnt/apple2
```

The filesystem remains mounted while `a2fuse` is running. Press `Ctrl-C` to
unmount cleanly.

## Linux Mount Usage

Install FUSE 3 first, then mount with the same command. The `macfuse` Cargo
feature name is historical; it enables the Unix FUSE mount backend on Linux too.

```sh
sudo apt install fuse3 pkg-config   # Debian/Ubuntu example
mkdir -p ~/mnt/apple2
cargo run --features macfuse -- mount image.po ~/mnt/apple2
```

If you need to unmount from another terminal:

```sh
fusermount3 -u ~/mnt/apple2
```

If shutdown reports `Resource busy`, close any app or shell using the mount and
try again.

Available options:

```sh
a2fuse mount image.po ~/mnt/apple2
a2fuse mount --readonly image.po ~/mnt/apple2
a2fuse mount --debug image.po ~/mnt/apple2
a2fuse mount --metadata=xattr image.po ~/mnt/apple2
a2fuse mount --metadata=filename image.po ~/mnt/apple2
```

Filename metadata mode produces names such as:

```text
NAME,t$ff,a$2000
```

## macOS Gatekeeper Warning

When downloading a binary from GitHub Releases, macOS may display:

> Apple could not verify "a2fuse" is free of malware that may harm your Mac or compromise your privacy.

This occurs because the binary is unsigned. To resolve:

**Option 1: Remove the quarantine flag**

```sh
xattr -d com.apple.quarantine ./a2fuse
./a2fuse mount image.po ~/mnt/apple2
```

**Option 2: Use Finder**

Right-click the downloaded binary and select "Open". macOS will ask for confirmation once, then allow future runs.

**Option 3: Install via Homebrew** (planned)

```sh
brew install markassad/a2fuse/a2fuse
```

## Current limitations

- Only ProDOS block-order images are supported.
- Images with 2MG headers, DOS 3.3 sector ordering, nibble encoding, or other
  container formats are not detected or converted.
- The filesystem is strictly read-only.
- Image mutation currently supports creating directories and adding regular
  files to the root or existing subdirectories.
- Replacing, renaming, deleting, recursively importing or extracting directory
  trees, and changing metadata are not yet implemented.
- New image files have zeroed ProDOS timestamps.
- The project does not bundle ProDOS system files; `fetch-prodos` downloads them
  into a local cache on demand.
- Extended-file data and resource forks can be read, but cannot yet be written.
- ProDOS sparse file blocks are returned as zero-filled data.
- Finder-specific metadata writes are rejected; command-line use is the
  primary target for this version.
- Image validation is intentionally conservative but is not yet a complete
  ProDOS filesystem checker.

## Roadmap

1. Add recursive directory import and extraction, replacement, rename, and
   deletion.
2. Add automatic host-file metadata inference and preservation sidecars.
3. Add more corrupt-image and real-world compatibility tests using freely
   redistributable fixtures.
4. Improve Finder interoperability without accepting metadata writes.
5. Add write support for ProDOS extended files and resource forks.

No copyrighted disk images are included. Tests construct small artificial
fixtures in memory.

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
- [Roadmap](docs/roadmap.md)
- [Contributing](docs/development/contributing.md)

## Status

The mounted filesystem is read-only. The parser and mount foundation requested
for the first milestone is complete, and a small experimental offline image
maintenance layer is now under development.

The project currently:

- accepts ProDOS-order images made from 512-byte blocks, commonly named `.po`;
- creates empty ProDOS volumes;
- imports files into the volume root directory;
- lists files and writes file contents to standard output;
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

Choose another size in 512-byte blocks:

```sh
cargo run -- create archive.po --name ARCHIVE --blocks 1600
```

Import a host file into the root directory:

```sh
cargo run -- put work.po README.txt README --type '$04'
cargo run -- put work.po PROGRAM.BIN PROGRAM --type '$06' --aux-type '$2000'
```

The destination name defaults to the host filename. ProDOS names must contain
1 to 15 ASCII characters, begin with a letter, and otherwise contain only
letters, digits, or periods.

List or read files:

```sh
cargo run -- ls work.po
cargo run -- ls work.po --long
cargo run -- catalog work.po
cargo run -- cat work.po README
cargo run -- cat work.po PROGRAM > PROGRAM.BIN
```

`ls` uses host-oriented Unix-style output. Its `--long` form shows permissions,
link count, synthetic owner and group names, byte size, and modification time.
`catalog` uses an Apple II-style ProDOS catalogue with file types, allocated
blocks, ProDOS timestamps, EOF, and auxiliary types.

`view` and `add` are aliases for `cat` and `put`.

## Mount Usage

```sh
mkdir -p ~/mnt/apple2
cargo run --features macfuse -- image.po ~/mnt/apple2
```

The filesystem remains mounted while `a2fuse` is running. Unmount it from
another terminal:

```sh
diskutil unmount ~/mnt/apple2
```

Available options:

```sh
a2fuse mount image.po ~/mnt/apple2
a2fuse --readonly image.po ~/mnt/apple2
a2fuse --debug image.po ~/mnt/apple2
a2fuse --metadata=xattr image.po ~/mnt/apple2
a2fuse --metadata=filename image.po ~/mnt/apple2
```

Filename metadata mode produces names such as:

```text
NAME,t$ff,a$2000
```

## Current limitations

- Only ProDOS block-order images are supported.
- Images with 2MG headers, DOS 3.3 sector ordering, nibble encoding, or other
  container formats are not detected or converted.
- The filesystem is strictly read-only.
- Image mutation currently supports adding regular files to the root directory.
- Creating subdirectories, replacing, renaming, deleting, and changing metadata
  are not yet implemented.
- New image files have zeroed ProDOS timestamps and no boot loader.
- Extended files and resource forks are not yet supported.
- ProDOS sparse file blocks are returned as zero-filled data.
- Finder-specific metadata writes are rejected; command-line use is the
  primary target for this version.
- Image validation is intentionally conservative but is not yet a complete
  ProDOS filesystem checker.

## Roadmap

1. Add directory creation, extraction, replacement, rename, and deletion.
2. Add automatic host-file metadata inference and preservation sidecars.
3. Add more corrupt-image and real-world compatibility tests using freely
   redistributable fixtures.
4. Improve Finder interoperability without accepting metadata writes.
5. Support ProDOS extended files and resource forks.

No copyrighted disk images are included. Tests construct small artificial
fixtures in memory.

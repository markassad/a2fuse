# a2fuse

`a2fuse` is a small Rust command-line tool that mounts an Apple II ProDOS
disk image as a read-only filesystem on macOS using macFUSE.

The project is deliberately library-first: ProDOS parsing and file block
resolution live independently of the FUSE adapter, so they can be tested with
artificial byte arrays and used by other tools.

## Status

This is an early read-only implementation. It currently:

- accepts ProDOS-order images made from 512-byte blocks, commonly named `.po`;
- parses the volume directory and chained subdirectories;
- exposes seedling, sapling, and tree files;
- preserves ProDOS file type, auxiliary type, access flags, storage type,
  timestamps, EOF, key pointer, and block usage internally;
- supports `getattr`, `lookup`, `readdir`, `open`, `read`, and `statfs`;
- returns read-only errors for common mutation operations;
- exposes metadata as read-only extended attributes, or as filename suffixes.

Normal filenames do not contain metadata suffixes. In the default xattr mode,
regular entries expose:

```text
prodos.type
prodos.aux_type
prodos.access
prodos.storage_type
```

## Requirements

- macOS
- a current stable Rust toolchain
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

## Usage

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
- Extended files and resource forks are not yet supported.
- ProDOS sparse file blocks are returned as zero-filled data.
- Finder-specific metadata writes are rejected; command-line use is the
  primary target for this version.
- Image validation is intentionally conservative but is not yet a complete
  ProDOS filesystem checker.

## Roadmap

1. Add more corrupt-image and real-world compatibility tests using freely
   redistributable fixtures.
2. Improve Finder interoperability without accepting metadata writes.
3. Support ProDOS extended files and resource forks.
4. Add richer read-only metadata presentation.
5. Consider write support only after the parser and allocation logic have
   comprehensive tests.

No copyrighted disk images are included. Tests construct small artificial
fixtures in memory.

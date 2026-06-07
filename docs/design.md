# a2fuse design

## Goals

`a2fuse` is a library-first Rust toolkit for Apple II ProDOS disk images. Its
primary goals are:

- parse ProDOS-order images safely and report corrupt data clearly;
- keep ProDOS knowledge independent from FUSE and command-line presentation;
- mount images read-only through a small, replaceable FUSE adapter;
- make parser behaviour testable without macFUSE or copyrighted disk images;
- grow into an image-maintenance tool without weakening read-only mount safety.

Supporting every Apple II disk format is not an initial goal. The current scope
is raw ProDOS block-order images with 512-byte blocks.

## Architecture

The crate is divided into three layers.

### ProDOS library

`src/prodos` owns all filesystem-format behaviour:

- `block.rs`: validated 512-byte block access;
- `types.rs`: storage types, access flags, and timestamps;
- `path.rs`: ProDOS filename and host-name conversion;
- `directory.rs`: directory entry and linked-directory parsing;
- `file.rs`: seedling, sapling, and tree file resolution;
- `volume.rs`: volume header parsing and the in-memory directory tree;
- `writer.rs`: experimental offline image creation, directories, and file
  import.

Code in this layer must not depend on FUSE. Parsing APIs operate on byte-backed
block devices and return structured errors.

### FUSE adapter

`src/fuse` translates the parsed volume into `fuser` operations:

- `inode.rs` creates stable in-memory inode mappings;
- `attrs.rs` maps ProDOS metadata to host attributes and read-only xattrs;
- `fs.rs` implements lookup, attributes, directory listing, open, read, and
  filesystem statistics;
- `mod.rs` contains the narrow mount boundary.

The adapter is enabled by the `macfuse` Cargo feature. The default build excludes
it so parser and image-tool tests do not require a FUSE runtime.

Mounted filesystems are always read-only. Mutation requests return `EROFS`; the
offline writer is not reachable from the FUSE adapter.

### Command-line application

`src/cli.rs` defines arguments and subcommands. `src/main.rs` performs command
dispatch and presentation only. It should not contain ProDOS parsing or block
allocation rules.

Explicit image commands use subcommands such as `create`, `mkdir`, `ls`,
`catalog`, `get`, and `put`. `ls` presents host-oriented Unix-style output,
while `catalog` presents ProDOS-native metadata in an Apple II-style layout.

## Error handling

Library functions return `Result<T, A2FuseError>`. Invalid image structure,
unsupported storage types, bad names, full directories, and allocation
failures must produce useful errors rather than panics or partial success.

Offline image saves use a temporary file and rename so a failed write does not
truncate the original image. New mutation operations should preserve this
transactional approach.

## Metadata

ProDOS metadata is represented explicitly in `DirectoryEntry`. Normal mounted
filenames do not contain metadata. The mount adapter can expose metadata as
read-only extended attributes:

```text
prodos.type
prodos.aux_type
prodos.access
prodos.storage_type
```

Filename suffix mode is intended for debugging and interoperability:

```text
NAME,t$ff,a$2000
```

## Testing strategy

Most tests must run without FUSE. Tests construct artificial blocks and images
in memory to cover:

- image length and block bounds;
- filename and lowercase-flag decoding;
- directory entry parsing and linked blocks;
- seedling, sapling, tree, and sparse-file reads;
- image creation, allocation, root and nested import, and parser round trips;
- command-line create/import/list/read workflows.

FUSE integration tests are separate because they require macOS, macFUSE, and a
mount-capable environment.

## Safety and compatibility

Unsafe Rust is not expected. Corrupt images must be treated as untrusted input.
All offsets, block pointers, directory chains, bitmap locations, and declared
lengths require bounds or consistency checks before use.

The `fuser` dependency is isolated behind a Cargo feature and a narrow adapter
so it can be upgraded or replaced without changing the ProDOS library.

# Roadmap

The roadmap is deliberately staged. Parser correctness and reproducible tests
come before broad format support or mounted writes.

## Milestone 1: parser foundation

Status: complete.

- create the Rust crate and library-first module structure;
- add `clap`, `thiserror`, and `tracing`;
- implement validated 512-byte block reading;
- decode ProDOS filenames and lowercase flags;
- parse volume headers and directory entries;
- read linked directories;
- resolve seedling and sapling files;
- add parser-focused tests using artificial byte arrays;
- document the absence of copyrighted disk images.

The original plan placed FUSE behind this milestone. The repository has since
advanced beyond that placeholder stage.

## Milestone 2: read-only filesystem

Status: implemented, with real mount testing still environment-dependent.

- isolate `fuser` behind the `macfuse` Cargo feature;
- implement inode and attribute mapping;
- implement `lookup`, `getattr`, `readdir`, `open`, `read`, and `statfs`;
- return read-only errors for mutation operations;
- expose ProDOS metadata through read-only xattrs or debug filenames;
- add tree-file and nested-directory parsing.

Mounted writes and Finder metadata writes remain out of scope.

## Milestone 3: offline image inspection

Status: implemented.

- list image contents;
- provide separate Unix-style `ls` and ProDOS-style `catalog` views;
- display detailed ProDOS metadata;
- write a selected file to standard output;
- keep inspection independent from FUSE.

Planned additions:

- recursive listings;
- extraction to host directories;
- optional metadata sidecars;
- machine-readable output.

## Milestone 4: conservative offline maintenance

Status: experimental first slice implemented.

- create raw ProDOS-order images;
- add root-directory regular files;
- allocate seedling, sapling, and tree storage;
- validate bitmap placement;
- save through temporary-file replacement;
- round-trip written images through the read-only parser.

Before expanding this layer:

- add rollback within in-memory mutations;
- test more fragmented and sparse allocation patterns;
- verify generated images with independent ProDOS tools;
- add freely redistributable compatibility fixtures.

Next operations:

1. create subdirectories;
2. extract directory trees;
3. replace files safely;
4. rename entries;
5. delete entries and release blocks;
6. write and preserve timestamps;
7. infer or preserve Apple II file metadata.

## Milestone 5: broader compatibility

- support extended files and resource forks;
- detect or explicitly handle 2MG containers;
- improve Finder interoperability without accepting mounted writes;
- add filesystem consistency checking and diagnostics;
- consider additional Apple II formats only behind separate adapters.

## Non-goals for the current releases

- writable FUSE mounts;
- pretending unsupported mutations succeeded;
- automatic repair of corrupt images;
- bundling copyrighted disk images;
- claiming full CiderPress format coverage.

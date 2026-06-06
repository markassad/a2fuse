# Supported ProDOS format

This document describes the subset of ProDOS currently understood by
`a2fuse`. It is an implementation guide, not a replacement for the ProDOS
technical reference manuals.

## Image container

Supported images are raw ProDOS block-order files, commonly using the `.po`
extension:

- block size: 512 bytes;
- block numbers are stored as little-endian 16-bit values;
- the image length must be an exact multiple of 512 bytes;
- the volume directory key block is block 2.

The following are currently out of scope:

- DOS 3.3 sector-order images;
- nibble, WOZ, and 2MG containers;
- partition maps and automatic container detection;
- damaged-image repair.

## Volume directory

Directories are linked lists of blocks. Every directory block starts with:

| Offset | Size | Meaning |
| --- | ---: | --- |
| `$00` | 2 | Previous directory block |
| `$02` | 2 | Next directory block |
| `$04` | 39 | First directory entry |

There are 13 entries per block and each entry is 39 bytes. The first entry in
the volume directory key block is a volume header. The implementation expects:

- storage type `$F`;
- entry length `$27`;
- 13 entries per block;
- a bitmap pointer and total block count within the image.

The initial image formatter reserves blocks 2 through 5 for the root directory
and starts the volume bitmap at block 6. Boot blocks 0 and 1 are left zeroed.

## Directory entries

The first byte combines a four-bit storage type and four-bit filename length.
The parser preserves:

- filename and lowercase flags;
- file type and auxiliary type;
- key pointer and blocks used;
- 24-bit EOF;
- access flags;
- creation and modification timestamps;
- header pointer.

Recognised storage types include:

| Value | Meaning |
| --- | --- |
| `$0` | Inactive entry |
| `$1` | Seedling file |
| `$2` | Sapling file |
| `$3` | Tree file |
| `$4` | Pascal area |
| `$5` | Extended file |
| `$D` | Subdirectory |
| `$E` | Subdirectory header |
| `$F` | Volume header |

Extended files and Pascal areas are recognised but not currently read as
regular files.

## Filenames

ProDOS names contain 1 to 15 characters. Newly written names must:

- begin with an ASCII letter;
- otherwise contain ASCII letters, digits, or periods.

Names are stored uppercase with ProDOS lowercase flags when lowercase
characters were supplied. When presenting existing names to a host, slash,
colon, NUL, and non-printable characters are replaced with underscores.

## File storage

### Seedling

A seedling entry points directly to one data block. Empty files use no data
block and have a zero key pointer.

### Sapling

A sapling entry points to one index block. The low bytes of its 256 data-block
pointers occupy bytes 0 through 255; high bytes occupy bytes 256 through 511.
Zero pointers are treated as sparse, zero-filled blocks when reading.

### Tree

A tree entry points to a master index block. Its pointers identify sapling
index blocks, each of which addresses up to 256 data blocks. This supports the
24-bit ProDOS EOF range.

## Allocation bitmap

Each bitmap bit describes one volume block:

- `1`: free;
- `0`: allocated.

Bits are ordered most-significant-bit first within each byte. One bitmap block
covers 4096 volume blocks. The experimental writer validates the declared
bitmap range before mutation and allocates the first available blocks.

## Write scope

Mounted images remain read-only. The experimental offline writer currently
supports:

- creating a new raw ProDOS volume;
- adding a regular seedling, sapling, or tree file to the root directory;
- updating the allocation bitmap and root file count;
- preserving explicit file type, auxiliary type, and access flags.

It does not yet support subdirectory creation, replacement, deletion, rename,
resource forks, timestamp writing, boot loaders, or repair.

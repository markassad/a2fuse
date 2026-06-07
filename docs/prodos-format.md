# Supported ProDOS format

This document describes the subset of ProDOS currently understood by
`a2fuse`. It is an implementation guide, not a replacement for the ProDOS
technical reference manuals.

For the manuals and technical notes used to derive these structures, see
[ProDOS format references](prodos-references.md). The primary source is
[Appendix B: File Organization][techref-b] in Apple's *ProDOS 8 Technical
Reference Manual*. The [CiderPress II ProDOS notes][ciderpress-prodos] are a
useful implementation-oriented companion.

## Image container

Supported images are raw ProDOS block-order files, commonly using the `.po`
extension:

- block size: 512 bytes;
- block numbers are stored as little-endian 16-bit values;
- the image length must be an exact multiple of 512 bytes;
- the volume directory key block is block 2.

See the Technical Reference, sections B.1 and B.2, for logical blocks and
volume organisation. See the [CiderPress II unadorned image
notes][ciderpress-unadorned] for the raw `.po` container and the distinction
between block order and sector order.

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

See the Technical Reference, sections B.2 through B.2.3, especially Figures
B-2 through B-4.

The initial image formatter reserves blocks 2 through 5 for the root directory
and starts the volume bitmap at block 6. By default, boot blocks 0 and 1 are
left zeroed. When bootable creation is requested, they are copied from a cached
upstream ProDOS image, and the root directory receives `PRODOS` plus
`BASIC.SYSTEM` copied from that same cached source.

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

Extended-file data and resource forks are parsed and can be read. Pascal areas
are recognised but are not exposed as regular files.

See the Technical Reference, section B.2.4 and Figure B-5. Storage types `$4`
and `$5` are defined further in [ProDOS 8 Technical Note #25][tn25].

## Filenames

ProDOS names contain 1 to 15 characters. Newly written names must:

- begin with an ASCII letter;
- otherwise contain ASCII letters, digits, or periods.

Names are stored uppercase with ProDOS lowercase flags when lowercase
characters were supplied. When presenting existing names to a host, slash,
colon, NUL, and non-printable characters are replaced with underscores.

Filename syntax and pathnames are described in the Technical Reference,
sections 2.1 and 2.1.1. The case-preserving bit field is a later extension
documented by [GS/OS Technical Note #8][tn-gsos-8].

## File storage

See the Technical Reference, section B.3 and Figures B-6 through B-9, for the
three standard storage forms and sparse files.

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

See the Technical Reference, sections B.1 and B.2.2. The volume-header
description defines the bitmap pointer, bit ordering, and free-bit meaning.

## Write scope

Mounted images remain read-only. The experimental offline writer currently
supports:

- creating a new raw ProDOS volume;
- creating root and nested subdirectories;
- adding a regular seedling, sapling, or tree file to the root or an existing
  subdirectory;
- deleting regular seedling, sapling, or tree files;
- tokenizing host AppleSoft BASIC text into ProDOS BAS files and untokenizing
  BAS files back to host text through CLI commands;
- updating the allocation bitmap and affected directory file counts;
- preserving explicit file type, auxiliary type, and access flags.

New subdirectories use one allocated key block. Their parent entry uses storage
type `$D`, file type `$0F`, one block used, and EOF 512. The first entry in the
key block is a `$E` subdirectory header containing the parent block, 1-based
parent entry number, and parent entry length `$27`.

Regular-file entries store the key block of their containing directory in the
header pointer field. Each successful import increments that directory's file
count; the volume header count therefore continues to describe root entries
only.

The writer does not yet grow a full directory chain. It also does not support
replacement, directory deletion, rename, resource-fork writing, timestamp
writing, or repair.

[techref-b]: https://prodos8.com/docs/techref/file-organization/
[ciderpress-prodos]: https://ciderpress2.com/formatdoc/ProDOS-notes.html
[ciderpress-unadorned]: https://ciderpress2.com/formatdoc/Unadorned-notes.html
[tn25]: https://prodos8.com/docs/technote/25/
[tn-gsos-8]: https://prodos8.com/docs/technote/gsos/08/

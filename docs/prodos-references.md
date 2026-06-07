# ProDOS format references

This page records the documentation used to understand and implement the
ProDOS structures in `a2fuse`. Start with the Apple technical reference, then
use the technical notes for later extensions and the CiderPress II notes for
implementation details and real-world qualifications.

The repository's own [supported format guide](prodos-format.md) describes what
`a2fuse` currently implements. The sources below describe the wider format.

## Primary reference

### ProDOS 8 Technical Reference Manual

Apple's *ProDOS 8 Technical Reference Manual* is the primary source for the
filesystem layout:

- [HTML contents](https://prodos8.com/docs/techref/)
- [Appendix B: File Organization](https://prodos8.com/docs/techref/file-organization/)
- [scanned PDF](https://mirrors.apple2.org.za/ftp.apple.asimov.net/documentation/os/prodos/ProDOS-8-Tech-Ref.pdf)

Appendix B is the most useful part for this project. The printed page numbers
below are the page numbers shown in the manual, which may differ from a PDF
viewer's page counter.

| Topic | Manual section | Printed pages |
| --- | --- | ---: |
| Logical blocks and overall volume layout | B.1 | 146 |
| Directory block chains | B.2 and B.2.1 | 147-148 |
| Volume directory header | B.2.2 | 148-150 |
| Subdirectory header | B.2.3 | 151-153 |
| File directory entry | B.2.4 | 154-156 |
| Scanning directory files | B.2.5 | 157-158 |
| Seedling, sapling, and tree files | B.3 | 159-166 |
| Sparse files | B.3.6 | 164-165 |
| Field summaries, dates, access, and file types | B.4 | 167-173 |

Chapter 2, especially sections 2.1 and 2.2, is also useful for filenames,
pathnames, file attributes, directories, and the user-visible model behind the
on-disk structures.

## Apple extensions

The original manual predates several format extensions encountered on later
ProDOS and GS/OS volumes:

- [GS/OS Technical Note #8: Filenames With More Than CAPS and
  Numerals](https://prodos8.com/docs/technote/gsos/08/) defines the lowercase
  filename flags stored in directory entries and volume headers.
- [ProDOS 8 Technical Note #25: Non-Standard Storage
  Types](https://prodos8.com/docs/technote/25/) documents storage type `$4`
  Pascal areas and storage type `$5` extended files, including data and
  resource fork mini-entries.
- [ProDOS 8 Technical Note #28: ProDOS Dates -- 2000 and
  Beyond](https://prodos8.com/docs/technote/28/) defines how the seven-bit
  ProDOS year field maps to 1940-2039.
- [ProDOS 8 Technical Note #30: Sparse
  Station](https://prodos8.com/docs/technote/30/) discusses sparse-file
  behaviour and EOF limits.
- [Technical Notes index](https://prodos8.com/docs/technote/) provides the
  surrounding Apple II technical-note collection.

## Implementation-oriented references

The [CiderPress II ProDOS format
notes](https://ciderpress2.com/formatdoc/ProDOS-notes.html) provide a compact
byte-offset reference and call out differences between the original
specification and files found in practice. They are particularly useful for:

- checking directory-entry and header offsets;
- understanding extended files and Finder information;
- interpreting sparse blocks and timestamp edge cases;
- identifying fields that older and newer software use differently.

The [CiderPress II unadorned disk image
notes](https://ciderpress2.com/formatdoc/Unadorned-notes.html) explain raw
block images such as `.po`: block 0 is followed by block 1 and so on, with no
container header or footer. They also explain why a filename extension alone
does not reliably identify a disk's filesystem or sector ordering.

*Beneath Apple ProDOS*, referenced by the CiderPress notes, is another detailed
historical description:

- [Beneath Apple ProDOS, second
  printing](https://archive.org/details/Beneath_Apple_ProDOS_Alt/mode/1up)

It is useful as supporting material, but this project treats Apple's technical
reference and technical notes as authoritative when sources disagree.

## Source map for a2fuse

| `a2fuse` area | Main documentation |
| --- | --- |
| Raw `.po` block ordering | CiderPress II unadorned disk image notes |
| `block.rs` | Technical Reference B.1 |
| `volume.rs` | Technical Reference B.1 and B.2.2 |
| `directory.rs` | Technical Reference B.2 |
| `file.rs` seedling/sapling/tree support | Technical Reference B.3 |
| `file.rs` extended-file support | Technical Note #25 |
| `types.rs` timestamps and access flags | Technical Reference B.4; Technical Note #28 |
| `path.rs` lowercase flags | GS/OS Technical Note #8 |
| `writer.rs` directory and allocation layout | Technical Reference B.1-B.4 |

## Working with conflicting sources

Historical documentation sometimes describes the original format while later
technical notes describe extensions or corrected behaviour. When adding format
support:

1. Use the ProDOS 8 Technical Reference Manual for the base layout.
2. Apply later Apple technical notes where they explicitly extend or revise it.
3. Consult CiderPress II for known real-world variations.
4. Validate assumptions with artificial fixtures and freely redistributable
   images rather than copyrighted software images.
5. Record any deliberate compatibility interpretation in
   [prodos-format.md](prodos-format.md) and its tests.

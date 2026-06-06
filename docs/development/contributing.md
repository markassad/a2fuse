# Contributing

Contributions are welcome. Keep changes focused, testable, and consistent with
the library-first design described in [design.md](../design.md).

## Development setup

Parser and image-tool development requires a current stable Rust toolchain:

```sh
cargo build
cargo test
cargo clippy --all-targets --no-default-features -- -D warnings
```

To compile the macFUSE adapter on macOS:

```sh
brew install --cask macfuse
cargo check --features macfuse
cargo clippy --all-targets --features macfuse -- -D warnings
```

Mount tests require a working local macFUSE installation and should not be
required for ordinary parser tests.

## Code organisation

- keep ProDOS format logic in `src/prodos`;
- keep FUSE translation in `src/fuse`;
- keep CLI parsing in `src/cli.rs`;
- keep command dispatch and output formatting in `src/main.rs`;
- return `A2FuseError` from library operations;
- avoid unsafe code unless there is no reasonable alternative.

Do not add format rules to the FUSE layer or make parser tests depend on FUSE.

## Tests

Every parser or writer change should include focused tests. Prefer small byte
arrays or images constructed in memory. Useful test categories include:

- valid and invalid block pointers;
- truncated and corrupt directory structures;
- filename and lowercase-bit edge cases;
- seedling, sapling, and tree boundaries;
- sparse block pointers;
- allocation bitmap boundaries;
- mutation round trips through the read-only parser;
- command exit status and byte-for-byte output.

Run formatting before submitting:

```sh
cargo fmt -- --check
```

## Test data

Do not commit copyrighted Apple II disk images. Artificial fixtures are
preferred. Any external fixture must have a clear redistribution licence and a
short provenance note in [`testdata/README.md`](../../testdata/README.md).

## Write support

Mounted filesystems are read-only. Do not connect offline mutation APIs to
FUSE.

Offline write changes need stricter review than read-only parsing changes.
They should:

- validate all affected pointers and bitmap ranges before mutation;
- fail without modifying the destination image when possible;
- update directory counts, block counts, EOF, and allocation bits together;
- reopen the result through the parser in tests;
- include disk-full and corrupt-image cases.

## Documentation

Use UK English in comments and documentation. Update:

- [`docs/prodos-format.md`](../prodos-format.md) when supported on-disk
  structures change;
- [`docs/design.md`](../design.md) when component boundaries or safety rules
  change;
- [`docs/roadmap.md`](../roadmap.md) when a milestone materially advances;
- [`README.md`](../../README.md) when user-visible commands or requirements
  change.

# gitr Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-07

## Active Technologies
- Rust 1.75+ (Cargo workspace, 19 crates) + `bstr` 1 (byte strings), `sha1`/`sha2` 0.10 (hashing), `flate2` 1 (zlib), `memmap2` 0.9 (packfile mmap), `crc32fast` 1 (pack index), `clap` 4 (CLI), `thiserror` 2 / `anyhow` 1 (errors), `tempfile` 3 (test isolation), `rayon` 1 (parallelism) (020-git-parity)
- Git on-disk format (loose objects, packfiles, refs, index, config) — all file-based (020-git-parity)
- Rust 1.75+ (Cargo workspace, 16 crates) + `bstr` 1, `sha1`/`sha2` 0.10, `flate2` 1, `memmap2` 0.9, `crc32fast` 1, `clap` 4, `thiserror` 2 / `anyhow` 1, `rayon` 1 (020-git-parity)
- Git on-disk format (loose objects, packfiles v2, refs, index, config) — all file-based (020-git-parity)
- Rust 1.75+ (Cargo workspace, 19 crates) + `tempfile` 3 (test isolation), `std::process::Command` (subprocess execution) (021-e2e-interop-coverage)
- N/A (test-only feature) (021-e2e-interop-coverage)

- Rust 1.75+ + `tempfile` (test isolation), `std::process::Command` (subprocess execution) (019-e2e-interop-tests)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test [ONLY COMMANDS FOR ACTIVE TECHNOLOGIES][ONLY COMMANDS FOR ACTIVE TECHNOLOGIES] cargo clippy

## Code Style

Rust 1.75+: Follow standard conventions

## Recent Changes
- 021-e2e-interop-coverage: Added Rust 1.75+ (Cargo workspace, 19 crates) + `tempfile` 3 (test isolation), `std::process::Command` (subprocess execution)
- 020-git-parity: Added Rust 1.75+ (Cargo workspace, 16 crates) + `bstr` 1, `sha1`/`sha2` 0.10, `flate2` 1, `memmap2` 0.9, `crc32fast` 1, `clap` 4, `thiserror` 2 / `anyhow` 1, `rayon` 1
- 020-git-parity: Added Rust 1.75+ (Cargo workspace, 19 crates) + `bstr` 1 (byte strings), `sha1`/`sha2` 0.10 (hashing), `flate2` 1 (zlib), `memmap2` 0.9 (packfile mmap), `crc32fast` 1 (pack index), `clap` 4 (CLI), `thiserror` 2 / `anyhow` 1 (errors), `tempfile` 3 (test isolation), `rayon` 1 (parallelism)


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

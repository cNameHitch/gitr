# gitr Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-07

## Active Technologies
- Rust 1.75+ (Cargo workspace, 19 crates) + `bstr` 1 (byte strings), `sha1`/`sha2` 0.10 (hashing), `flate2` 1 (zlib), `memmap2` 0.9 (packfile mmap), `crc32fast` 1 (pack index), `clap` 4 (CLI), `thiserror` 2 / `anyhow` 1 (errors), `tempfile` 3 (test isolation), `rayon` 1 (parallelism) (020-git-parity)
- Git on-disk format (loose objects, packfiles, refs, index, config) — all file-based (020-git-parity)
- Rust 1.75+ (Cargo workspace, 16 crates) + `bstr` 1, `sha1`/`sha2` 0.10, `flate2` 1, `memmap2` 0.9, `crc32fast` 1, `clap` 4, `thiserror` 2 / `anyhow` 1, `rayon` 1 (020-git-parity)
- Git on-disk format (loose objects, packfiles v2, refs, index, config) — all file-based (020-git-parity)
- Rust 1.75+ (Cargo workspace, 19 crates) + `tempfile` 3 (test isolation), `std::process::Command` (subprocess execution) (021-e2e-interop-coverage)
- N/A (test-only feature) (021-e2e-interop-coverage)
- Rust 1.75+ (Cargo workspace, 16 crates) + clap 4 (CLI), bstr 1 (byte strings), chrono (date formatting via git-utils), thiserror 2 / anyhow 1 (errors), git-diff (diffstat), git-revwalk (format/graph), git-ref (reflog) (025-git-parity-phase2)
- Git on-disk format (loose objects, packfiles, refs, index, config, reflog) — all file-based (025-git-parity-phase2)

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
- 025-git-parity-phase2: Added Rust 1.75+ (Cargo workspace, 16 crates) + clap 4 (CLI), bstr 1 (byte strings), chrono (date formatting via git-utils), thiserror 2 / anyhow 1 (errors), git-diff (diffstat), git-revwalk (format/graph), git-ref (reflog)
- 024-git-parity-polish: 42 behavioral parity fixes across `git-cli`, `git-revwalk`, `git-utils`, `git-config`, `git-repository` — pathspec disambiguation, format string completeness, exit code mapping, date padding, system config cascade, macOS init defaults
- 022-perf-optimization: Added `rayon` 1 (parallel stat), `lru` 0.12 (object cache), commit-graph v1 read/write support, generation-number pruning in git-revwalk


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

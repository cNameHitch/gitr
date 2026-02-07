# Implementation Plan: Loose Object Storage

**Branch**: `004-loose-object-storage` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/004-loose-object-storage/spec.md`

## Summary

Implement the `git-loose` crate for reading and writing individual zlib-compressed objects in the `.git/objects/` directory. This is the simpler of the two storage backends (the other being packfiles). Every new object starts life as a loose object.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-object`, `flate2` (zlib), `thiserror`
**Storage**: File system — `.git/objects/XX/YYYY...`
**Testing**: `cargo test`, interop tests with C git
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Read < 100μs for typical objects, write < 200μs
**Constraints**: Must produce files identical to C git's loose objects.
**Scale/Scope**: ~2 C files (object-file.c, loose.c) → ~800 lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Atomic writes, hash verification on read |
| C-Compatibility | ✅ Pass | zlib format identical, tested against C git |
| Modular Crates | ✅ Pass | `git-loose` depends on utils/hash/object only |
| Trait-Based | ✅ Pass | Implements object storage trait from spec 006 |
| Test-Driven | ✅ Pass | Interop tests with C git cat-file/hash-object |

## Project Structure

### Source Code

```text
crates/git-loose/
├── Cargo.toml
├── src/
│   ├── lib.rs          # LooseObjectStore public API
│   ├── read.rs         # Reading and decompressing loose objects
│   ├── write.rs        # Writing and compressing loose objects
│   ├── stream.rs       # Streaming reader for large objects
│   └── enumerate.rs    # Walking objects/ directory
├── tests/
│   ├── interop.rs      # Read/write interop with C git
│   └── streaming.rs    # Large object streaming tests
└── benches/
    └── loose_bench.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Compression | `flate2` crate with zlib backend | Industry standard, C-compatible zlib output |
| Atomic write | Write to `.git/objects/tmp_obj_XXXXXX`, rename on success | Matches C git behavior |
| Hash verification | Optional on read, configurable | Performance vs safety trade-off |
| Streaming | Custom `Read` impl over `flate2::read::ZlibDecoder` | Standard trait, composable |
| Temp directory | Same directory as target (ensures same filesystem for rename) | Atomic rename requires same mount |

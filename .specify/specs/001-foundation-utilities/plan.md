# Implementation Plan: Foundation Utilities

**Branch**: `001-foundation-utilities` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/001-foundation-utilities/spec.md`

## Summary

Implement the foundational utility crate (`git-utils`) that all other gitr crates depend on. This crate provides byte string handling, collections, path manipulation, glob matching, CLI framework setup, subprocess management, lock files, progress display, date parsing, and the error handling framework. It replaces ~22 C source files from the git codebase.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `bstr` 1.x, `clap` 4.x, `thiserror` 2.x, `crossbeam` 0.8, `chrono` 0.4
**Storage**: File system (lock files, temp files)
**Testing**: `cargo test`, property-based tests with `proptest`
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Path operations < 1μs, date parsing < 10μs, hashmap lookup O(1) amortized
**Constraints**: No `unsafe` without justification. No panics. No allocator-global state.
**Scale/Scope**: ~22 C files replaced, ~8K lines of C → ~4K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | All APIs return Result, no unsafe planned |
| C-Compatibility | Pass | Date formats and wildmatch tested against C git |
| Modular Crates | Pass | Single `git-utils` crate, no dependencies on other gitr crates |
| Trait-Based | Pass | Progress uses trait for output sink, hash function is generic |
| Test-Driven | Pass | Property tests for date parsing, wildmatch corpus from C git |

## Project Structure

### Documentation (this feature)

```text
specs/001-foundation-utilities/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/git-utils/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API re-exports
│   ├── bstring.rs          # Byte string extensions
│   ├── collections/
│   │   ├── mod.rs
│   │   ├── hashmap.rs      # Git-style hashmap
│   │   ├── string_list.rs  # Sorted string list
│   │   └── prio_queue.rs   # Priority queue
│   ├── path.rs             # Git path manipulation
│   ├── wildmatch.rs        # Glob pattern matching
│   ├── date.rs             # Date parsing and formatting
│   ├── lockfile.rs         # Lock file and atomic writes
│   ├── tempfile.rs         # Temp file with RAII cleanup
│   ├── subprocess.rs       # Subprocess spawning
│   ├── progress.rs         # Progress display
│   ├── color.rs            # ANSI color and terminal detection
│   ├── pager.rs            # Pager integration
│   └── error.rs            # Error types and macros
├── tests/
│   ├── date_compat.rs      # Date format compatibility with C git
│   ├── wildmatch_corpus.rs # Wildmatch test corpus from C git
│   └── lockfile_stress.rs  # Concurrent lock file tests
└── benches/
    └── path_bench.rs       # Path operation benchmarks
```

**Structure Decision**: Single library crate (`git-utils`) as the foundation layer. All other workspace crates depend on this.

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Byte strings | Re-export `bstr` types + extension traits | Git paths aren't UTF-8; bstr is the Rust ecosystem standard |
| CLI framework | `clap` 4.x with derive macros | Declarative, widely adopted, replaces C parse-options.c |
| Hashmap | `std::collections::HashMap` with configurable hasher | Rust's built-in HashMap is already excellent; use `BuildHasher` trait |
| Date parsing | Custom parser + `chrono` for formatting | Git's date formats are unique; chrono handles output formatting |
| Glob matching | Custom implementation porting wildmatch.c | Must match C git behavior exactly; no existing Rust crate is compatible |
| Error types | `thiserror` derive macros | Type-safe, zero-cost, widely adopted |
| Lock files | Custom implementation | Must match C git's lock protocol exactly (.lock suffix, atomic rename) |
| Subprocess | `std::process::Command` wrapper | Standard library is sufficient; add timeout and pipe management |

## Complexity Tracking

No constitution violations anticipated for this crate.

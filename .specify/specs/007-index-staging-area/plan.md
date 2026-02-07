# Implementation Plan: Index / Staging Area

**Branch**: `007-index-staging-area` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/007-index-staging-area/spec.md`

## Summary

Implement the `git-index` crate providing reading, writing, and manipulation of the git index (staging area). This includes the index file format, cache entries, extensions (TREE, REUC, UNTR), gitignore/gitattributes processing, and pathspec matching. The index is central to git's workflow — it sits between the working tree and the object database.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils` (paths, lock files, wildmatch), `git-hash`, `git-object`, `git-odb`, `bstr`, `thiserror`
**Storage**: `.git/index` file
**Testing**: `cargo test`, interop with C git `ls-files --stage`
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Read 100K-entry index < 200ms, write < 200ms
**Constraints**: Must handle index v2/v3/v4. Must preserve extensions.
**Scale/Scope**: ~12 C files (~10K lines) → ~5K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | Checksum verification, lock file writes |
| C-Compatibility | Pass | Byte-compatible index format, tested against C git |
| Modular Crates | Pass | `git-index` depends on utils/hash/object/odb |
| Trait-Based | Pass | Pathspec and ignore patterns use trait-based matching |
| Test-Driven | Pass | Round-trip tests, interop with C git |

## Project Structure

### Source Code

```text
crates/git-index/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Index struct, public API
│   ├── read.rs             # Index file reading (v2/v3/v4)
│   ├── write.rs            # Index file writing
│   ├── entry.rs            # IndexEntry, CacheEntryFlags
│   ├── extensions/
│   │   ├── mod.rs
│   │   ├── tree.rs         # TREE (cache tree) extension
│   │   ├── resolve_undo.rs # REUC extension
│   │   └── untracked.rs    # UNTR extension
│   ├── ignore.rs           # Gitignore pattern matching
│   ├── attributes.rs       # Gitattributes processing
│   ├── pathspec.rs         # Pathspec matching
│   └── dir.rs              # Directory/working tree scanning
├── tests/
│   ├── read_write_roundtrip.rs
│   ├── ignore_compat.rs
│   └── pathspec_compat.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Index version | Read v2/v3/v4, write v2 by default | v2 is most compatible |
| Entry storage | Vec<IndexEntry> sorted by path | Simple, matches C git's internal representation |
| Extensions | Parsed into typed structs | Type safety, but serialize back to original bytes for unknown extensions |
| Ignore matching | Layered stack (global → repo → directory) | Matches C git's search order |
| Pathspec | Parsed into a Pathspec struct with compiled patterns | Efficient repeated matching |
| Stat data | Preserve all fields even on platforms where some are meaningless | Round-trip compatibility |

# Implementation Plan: Object Model

**Branch**: `003-object-model` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/003-object-model/spec.md`

## Summary

Implement the `git-object` crate providing Rust types for git's four object types (blob, tree, commit, tag), their parsing and serialization, object type system, and name resolution. This is the third layer — it depends on foundation utilities (byte strings, signatures) and hash infrastructure (ObjectId, Hasher).

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils` (BString, Signature), `git-hash` (ObjectId, Hasher), `bstr`, `thiserror`
**Storage**: N/A (pure data model — storage is in specs 004-006)
**Testing**: `cargo test`, comparison against `git cat-file` output
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Parse commit < 1μs, parse tree entry < 100ns, serialize commit < 1μs
**Constraints**: Byte-identical serialization with C git. Zero-copy parsing where possible.
**Scale/Scope**: ~7 C files replaced, ~4K lines of C → ~2.5K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | All parsing returns Result, invalid data rejected |
| C-Compatibility | Pass | Serialization tested against C git byte-for-byte |
| Modular Crates | Pass | `git-object` depends only on `git-utils` and `git-hash` |
| Trait-Based | Pass | Object trait for common operations (type, size, serialize) |
| Test-Driven | Pass | Test corpus from git.git repository |

## Project Structure

### Source Code

```text
crates/git-object/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Object enum, ObjectType, public API
│   ├── blob.rs             # Blob type
│   ├── tree.rs             # Tree type, TreeEntry, FileMode, sorting
│   ├── commit.rs           # Commit type, parsing, serialization
│   ├── tag.rs              # Tag type, parsing, serialization
│   ├── header.rs           # Object header parsing ("type size\0")
│   ├── name.rs             # Object name resolution (rev-parse logic)
│   └── cache.rs            # LRU object cache
├── tests/
│   ├── parse_real_objects.rs  # Parse objects from git.git
│   ├── serialize_roundtrip.rs # Serialize → parse → verify
│   └── tree_sorting.rs       # Tree entry sort order tests
└── benches/
    └── parse_bench.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Object representation | Enum with owned data | Clear ownership, no lifetime complexity in public API |
| Tree entry parsing | Zero-copy where possible using BStr references | Performance for large trees |
| Commit parsing | Eager parse all fields | Commits are small, lazy parsing adds complexity |
| Object cache | `lru` crate or custom LRU | Simple, bounded memory usage |
| Name resolution | Separate module, depends on ODB trait | Needs object lookup, deferred to when ODB is available |
| FileMode | Enum with known variants + Unknown(u32) | Forward compatibility |

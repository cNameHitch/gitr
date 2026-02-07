# Implementation Plan: Object Database

**Branch**: `006-object-database` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/006-object-database/spec.md`

## Summary

Implement the `git-odb` crate providing the unified object database abstraction that sits above loose objects and packfiles. This is the primary interface that all higher-level git operations use to read and write objects. It handles searching across storage backends, alternates, caching, and prefix resolution.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-object`, `git-loose`, `git-pack`, `thiserror`
**Storage**: Delegates to `git-loose` and `git-pack`
**Testing**: `cargo test`, integration tests with real repositories
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Object read < 200μs, existence check < 1μs, thread-safe reads
**Constraints**: Must be thread-safe. Must support alternates.
**Scale/Scope**: ~1 C directory (odb/) + portions of object-file.c → ~1.5K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Thread-safe via read-write locks |
| C-Compatibility | ✅ Pass | Same search order and alternates behavior as C git |
| Modular Crates | ✅ Pass | `git-odb` composes `git-loose` + `git-pack` |
| Trait-Based | ✅ Pass | OdbBackend trait for pluggable storage |
| Test-Driven | ✅ Pass | Integration tests with real git repos |

## Project Structure

### Source Code

```text
crates/git-odb/
├── Cargo.toml
├── src/
│   ├── lib.rs          # ObjectDatabase, public API
│   ├── backend.rs      # OdbBackend trait
│   ├── search.rs       # Multi-source search logic
│   ├── alternates.rs   # Alternates file parsing and resolution
│   ├── prefix.rs       # OID prefix/abbreviation resolution
│   └── error.rs        # Error types
├── tests/
│   ├── unified_read.rs    # Read from mixed loose/packed
│   ├── alternates.rs      # Alternates chain tests
│   └── concurrent.rs      # Thread-safety stress tests
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Search order | Loose → packs (newest first) → alternates | Matches C git; loose may have newer versions |
| Thread safety | `RwLock` on pack list, lock-free loose reads | Packs can be added during gc; loose reads are inherently safe |
| Backend trait | `OdbBackend` trait with read/write/contains | Extensibility for future backends (e.g., cloud storage) |
| Caching | Delegate to ObjectCache in git-object | Single cache layer, not per-backend |
| Prefix resolution | Check all backends, collect matches, error on ambiguity | Must match C git's behavior exactly |

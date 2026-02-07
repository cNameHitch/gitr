# Implementation Plan: Reference System

**Branch**: `008-reference-system` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/008-reference-system/spec.md`

## Summary

Implement the `git-ref` crate providing reference resolution, creation, update, deletion, enumeration, reflogs, and pluggable backends. The default backend is the files backend (loose refs in `.git/refs/` + `packed-refs` file). This crate uses lock files from git-utils for atomic operations.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils` (lock files, paths, byte strings), `git-hash` (ObjectId), `thiserror`
**Storage**: File system — `.git/refs/`, `.git/packed-refs`, `.git/logs/`
**Testing**: `cargo test`, interop with C git ref operations
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Resolve < 10μs (files), enumerate all refs < 1ms (packed)
**Constraints**: Must be safe under concurrent access. Lock file protocol.
**Scale/Scope**: ~5 C files (refs.c, refs/*.c) ~5K lines → ~3K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Lock files for all writes, CAS for updates |
| C-Compatibility | ✅ Pass | Same ref resolution, packed-refs format |
| Modular Crates | ✅ Pass | `git-ref` depends only on `git-utils` and `git-hash` |
| Trait-Based | ✅ Pass | `RefStore` trait for pluggable backends |
| Test-Driven | ✅ Pass | Interop tests with C git for all ref operations |

## Project Structure

### Source Code

```text
crates/git-ref/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API, Reference types
│   ├── store.rs            # RefStore trait
│   ├── files/
│   │   ├── mod.rs          # Files backend
│   │   ├── loose.rs        # Loose ref resolution
│   │   ├── packed.rs       # packed-refs file read/write
│   │   └── transaction.rs  # Atomic ref transactions
│   ├── reflog.rs           # Reflog reading/writing
│   ├── name.rs             # Ref name validation
│   ├── iter.rs             # Ref iteration with prefix filtering
│   └── error.rs            # Error types
├── tests/
│   ├── resolve_interop.rs  # Resolve refs, compare with C git
│   ├── update_interop.rs   # Create/update/delete, verify with C git
│   ├── reflog_interop.rs   # Reflog compatibility
│   └── concurrent.rs       # Concurrent update stress test
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Backend trait | `RefStore` with resolve, update, iter, reflog methods | Supports files, packed, reftable backends |
| Default backend | Files (loose + packed-refs) | C git's default; most compatible |
| Transactions | Lock-based CAS (compare-and-swap) | Matches C git's ref_transaction API |
| Packed refs | Parse on first access, cache in memory | Small enough to fit in memory |
| Reflog format | One line per entry, same format as C git | Byte-compatible |
| Reftable | Deferred to spec 018 | Not needed for core functionality |

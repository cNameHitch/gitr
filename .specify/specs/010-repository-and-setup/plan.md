# Implementation Plan: Repository & Setup

**Branch**: `010-repository-and-setup` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/010-repository-and-setup/spec.md`

## Summary

Implement the `git-repository` crate providing the central `Repository` struct that ties together the object database, reference system, configuration, and index. This crate also handles repository discovery (finding .git), initialization, worktree support, and environment variable processing.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-odb`, `git-ref`, `git-config`, `git-index`, `thiserror`
**Storage**: File system (`.git/` structure)
**Testing**: `cargo test`, interop with C git
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Repository open < 10ms, discovery < 1ms
**Constraints**: Must match C git discovery behavior exactly.
**Scale/Scope**: ~4 C files (repository.c, setup.c, environment.c, common-main.c) → ~2K lines Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | Path validation, no unsafe |
| C-Compatibility | Pass | Discovery and init match C git exactly |
| Modular Crates | Pass | `git-repository` composes all subsystem crates |
| Trait-Based | Pass | Uses trait-based subsystems (RefStore, OdbBackend) |
| Test-Driven | Pass | Interop tests with C git for discovery and init |

## Project Structure

### Source Code

```text
crates/git-repository/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Repository struct, public API
│   ├── discover.rs     # Repository discovery (find .git)
│   ├── init.rs         # Repository initialization
│   ├── worktree.rs     # Worktree support
│   ├── env.rs          # Environment variable handling
│   └── error.rs        # Error types
├── tests/
│   ├── discover_interop.rs
│   ├── init_interop.rs
│   └── worktree_tests.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Subsystem loading | Lazy (loaded on first access) | Fast open, don't parse index until needed |
| Repository ownership | Owns ODB, refs, config; borrows index | ODB/refs/config are long-lived; index is transient |
| Discovery | Walk up directories, check for .git dir or file | Matches C git's setup_git_directory() |
| Init structure | Create .git/{HEAD,objects,refs,config,hooks,info} | Minimal C git-compatible structure |
| Worktrees | Detect via .git file with gitdir: redirect | Matches C git convention |

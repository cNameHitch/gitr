# Implementation Plan: Advanced Features

**Branch**: `018-advanced-features` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/018-advanced-features/spec.md`

## Summary

Implement advanced git features: garbage collection, fsck, submodules, worktree management, notes, replace objects, archive generation, GPG signing, credential helpers, hooks, fsmonitor, fast-import, bundles, and daemon. These features build on all prior library crates and round out the full git feature set.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: All `git-*` crates, `clap`, `anyhow`, `tar`, `zip`, `gpgme` or GPG subprocess
**Testing**: Integration tests against C git
**Target Platform**: Linux, macOS, Windows
**Project Type**: Various crates + CLI
**Scale/Scope**: ~18 commands/features, ~8K lines

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | GC is careful about unreachable objects, fsck validates |
| C-Compatibility | ✅ Pass | All operations interoperate with C git |
| Modular Crates | ✅ Pass | Features go in appropriate existing crates or new focused crates |
| Trait-Based | ✅ Pass | Hook runner uses trait, credential helper is trait-based |
| Test-Driven | ✅ Pass | Comprehensive tests for each feature |

## Project Structure

### Source Code

```text
crates/
├── git-pack/         # pack-objects and index-pack already here
├── git-repository/   # gc, repack, prune, fsck added here
├── git-archive/      # NEW: archive generation
│   └── src/
│       ├── lib.rs
│       ├── tar.rs
│       └── zip.rs
└── git-submodule/    # NEW: submodule support
    └── src/
        ├── lib.rs
        ├── config.rs   # .gitmodules parsing
        └── update.rs

src/commands/
├── gc.rs
├── repack.rs
├── prune.rs
├── fsck.rs
├── pack_objects.rs
├── index_pack.rs
├── submodule.rs
├── worktree.rs
├── notes.rs
├── replace.rs
├── archive.rs
├── credential.rs
├── fast_import.rs
├── bundle.rs
└── daemon.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| GPG signing | Subprocess (call gpg/gpg2) | Avoids linking to GPG library; uses user's GPG setup |
| Archive format | `tar` and `zip` crates | Standard Rust crates for archive formats |
| Hook execution | Subprocess via git-utils | Hooks are external scripts |
| Fsmonitor | Subprocess (call fsmonitor daemon) | Follows C git's approach |
| Daemon | Simple TCP server | Low priority, can be very basic initially |
| Submodules | New `git-submodule` crate | Complex enough to warrant its own crate |
| Fast-import | Streaming parser | Must handle very large imports |

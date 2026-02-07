# Implementation Plan: Merge Engine

**Branch**: `012-merge-engine` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/012-merge-engine/spec.md`

## Summary

Implement the `git-merge` crate providing three-way content merge, the ORT tree merge strategy, conflict handling, merge strategies, cherry-pick, revert, and the sequencer. This is one of the most complex subsystems in git.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-object`, `git-odb`, `git-index`, `git-diff`, `git-repository`, `thiserror`
**Storage**: Working tree + index for conflicts, `.git/sequencer/` for multi-commit state
**Testing**: `cargo test`, merge scenario comparison against C git
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Content merge < 10ms per file, tree merge < 5s for large repos
**Constraints**: Must match C git merge results exactly, including conflict markers.
**Scale/Scope**: ~6 C files (~10K lines) → ~5K lines Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | No unsafe, proper error handling for all conflict types |
| C-Compatibility | Pass | Conflict markers and merge results match C git |
| Modular Crates | Pass | `git-merge` depends on diff, object, index |
| Trait-Based | Pass | MergeStrategy trait for pluggable strategies |
| Test-Driven | Pass | Extensive merge scenario test suite |

## Project Structure

### Source Code

```text
crates/git-merge/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API, MergeResult, MergeOptions
│   ├── content.rs          # Three-way content merge
│   ├── tree.rs             # Tree-level ORT merge
│   ├── conflict.rs         # Conflict detection and recording
│   ├── strategy/
│   │   ├── mod.rs          # MergeStrategy trait
│   │   ├── ort.rs          # ORT merge strategy
│   │   ├── ours.rs         # Ours strategy
│   │   └── subtree.rs      # Subtree strategy
│   ├── cherry_pick.rs      # Cherry-pick implementation
│   ├── revert.rs           # Revert implementation
│   ├── sequencer.rs        # Multi-commit sequencer
│   └── apply.rs            # Patch application (git apply)
├── tests/
│   ├── content_merge.rs
│   ├── tree_merge.rs
│   ├── conflict_tests.rs
│   └── cherry_pick_tests.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Default strategy | ORT (not legacy recursive) | ORT is the modern default in C git, better performance and correctness |
| Conflict markers | Match C git format exactly (7 chars of marker) | Byte-compatibility |
| Sequencer state | Files in .git/sequencer/ matching C git layout | Interop: allow starting operation in C git and continuing in Rust |
| Content merge | Uses diff algorithms from git-diff | Reuse, not duplicate |
| Virtual merge base | Recursive merge of multiple bases | Required for criss-cross merges |

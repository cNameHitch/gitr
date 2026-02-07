# Implementation Plan: Diff Engine

**Branch**: `011-diff-engine` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/011-diff-engine/spec.md`

## Summary

Implement the `git-diff` crate providing line-level diff algorithms (Myers, histogram, patience), tree diffing, rename/copy detection, the diffcore transformation pipeline, and all diff output formats. The diff engine is central to many git operations including status, log, merge, and rebase.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-object`, `git-odb`, `git-index`, `git-repository`, `bstr`, `thiserror`
**Storage**: N/A (pure computation)
**Testing**: `cargo test`, byte-comparison against C git diff output
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Line diff < 10ms for typical files, tree diff < 1s for 60K-file trees
**Constraints**: Must match C git output byte-for-byte.
**Scale/Scope**: ~6 C files + xdiff/ (~8K lines) → ~4K lines Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | No unsafe, bounded memory for large diffs |
| C-Compatibility | ✅ Pass | Byte-identical output verified against C git |
| Modular Crates | ✅ Pass | `git-diff` with clear dependencies |
| Trait-Based | ✅ Pass | DiffAlgorithm trait, output formatter trait |
| Test-Driven | ✅ Pass | Large corpus of diff comparisons against C git |

## Project Structure

### Source Code

```text
crates/git-diff/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API, DiffResult, DiffOptions
│   ├── algorithm/
│   │   ├── mod.rs          # DiffAlgorithm trait
│   │   ├── myers.rs        # Myers diff
│   │   ├── histogram.rs    # Histogram diff
│   │   └── patience.rs     # Patience diff
│   ├── tree.rs             # Tree-to-tree diff
│   ├── worktree.rs         # Working tree diff
│   ├── rename.rs           # Rename/copy detection
│   ├── diffcore.rs         # Diffcore pipeline
│   ├── format/
│   │   ├── mod.rs
│   │   ├── unified.rs      # Unified diff format
│   │   ├── stat.rs         # --stat format
│   │   ├── raw.rs          # --raw format
│   │   ├── nameonly.rs     # --name-only, --name-status
│   │   └── combined.rs     # Combined diff for merges
│   └── binary.rs           # Binary file detection
├── tests/
│   ├── algorithm_tests.rs
│   ├── output_compat.rs    # Compare against C git output
│   └── rename_tests.rs
└── benches/
    └── diff_bench.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Diff algorithms | Custom implementation of Myers/Histogram/Patience | Must match C git's xdiff behavior exactly |
| Line representation | Byte slices with hash for fast comparison | Performance: avoid string allocation |
| Rename detection | Port C git's diffcore-rename algorithm | Must produce identical results |
| Output formatting | Trait-based formatters | Extensible, testable |
| Binary detection | Check for null bytes in first 8KB | Matches C git's heuristic |
| Working tree diff | Compare stat data first, content only if stat differs | Performance optimization (racily clean handling) |

# Implementation Plan: Revision Walking

**Branch**: `013-revision-walking` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/013-revision-walking/spec.md`

## Summary

Implement the `git-revwalk` crate providing commit history traversal, revision range parsing, merge base computation, commit-graph acceleration, pretty-printing, and object enumeration. This is a core library used by log, diff, merge, fetch, and gc operations.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-object`, `git-odb`, `git-ref`, `git-repository`, `bstr`, `thiserror`
**Storage**: Reads commit-graph file from `.git/objects/info/commit-graph` or `.git/objects/info/commit-graphs/`
**Testing**: `cargo test`, comparison against `git rev-list` output
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Walk 100K commits < 1s with commit-graph, merge-base < 100ms
**Constraints**: Must match C git's commit ordering exactly.
**Scale/Scope**: ~8 C files (~7K lines) → ~4K lines Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Iterative traversal (no recursion stack overflow) |
| C-Compatibility | ✅ Pass | Walk order verified against git rev-list |
| Modular Crates | ✅ Pass | `git-revwalk` depends on odb, ref, repository |
| Trait-Based | ✅ Pass | Walk output configurable via traits |
| Test-Driven | ✅ Pass | Comparison tests against git rev-list |

## Project Structure

### Source Code

```text
crates/git-revwalk/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API, RevWalk
│   ├── walk.rs             # Core traversal logic
│   ├── range.rs            # Revision range parsing
│   ├── sort.rs             # Sorting strategies
│   ├── merge_base.rs       # Merge base computation
│   ├── commit_graph/
│   │   ├── mod.rs          # CommitGraph reader
│   │   ├── parse.rs        # Commit-graph format parsing
│   │   └── generation.rs   # Generation number handling
│   ├── pretty.rs           # Pretty-print formatting
│   ├── graph_draw.rs       # ASCII graph drawing
│   ├── objects.rs          # Object listing (rev-list --objects)
│   └── filter.rs           # Object filters (partial clone)
├── tests/
│   ├── walk_order.rs       # Walk order comparison with git rev-list
│   ├── merge_base_tests.rs
│   ├── pretty_tests.rs
│   └── graph_tests.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Traversal | Iterative with priority queue | Avoid stack overflow on deep histories |
| Commit access | Commit-graph preferred, ODB fallback | 10x+ speedup for large repos |
| Sorting | Priority queue with configurable comparator | Supports all sort orders efficiently |
| Merge base | Standard LCA algorithm with paint (in-degree) | Well-known, matches C git behavior |
| Pretty-print | Custom formatter matching C git's format specifiers | Must be byte-identical |
| Graph drawing | Port C git's graph.c algorithm | ASCII art must match exactly |

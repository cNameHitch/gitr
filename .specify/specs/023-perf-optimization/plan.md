# Implementation Plan: Large-Repo Performance Optimization

**Branch**: `022-perf-optimization` | **Date**: 2026-02-08 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/022-perf-optimization/spec.md`

## Summary

Close the performance gap between gitr and C git on large repositories (10,000+ commits, 5,000+ files) for history-heavy operations (`log`, `rev-list`, `blame`, `status`) while maintaining 100% byte-identical output. The primary optimizations are: (1) leveraging commit-graph files for zero-copy commit metadata access, (2) generation-number-based revision walk pruning, (3) parallel stat() calls for status, and (4) diff-based blame attribution. Research confirmed that the existing codebase has commit-graph read support and generation number fields already in place but unused — the implementation activates and extends these foundations.

## Technical Context

**Language/Version**: Rust 1.75+ (Cargo workspace, 16 crates)
**Primary Dependencies**: bstr 1, sha1/sha2 0.10, flate2 1, memmap2 0.9, crc32fast 1, clap 4, rayon 1, lru 0.12, thiserror 2 / anyhow 1
**Storage**: Git on-disk format (loose objects, packfiles v2, index v2/v3, commit-graph v1, refs)
**Testing**: cargo test, criterion 0.5 benchmarks, byte-comparison against C git
**Target Platform**: Linux, macOS (same as existing)
**Project Type**: Cargo workspace — 16 member crates
**Performance Goals**: log within 1.2x of C git (~26ms), status within 1.2x (~33ms), rev-list within 1.2x (~21ms), blame within 1.5x (~80ms)
**Constraints**: 100% byte-identical output, safe Rust, no new external dependencies, no regression on small/medium repos (< 5%)
**Scale/Scope**: Targets repos with 10,000+ commits, 5,000+ tracked files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Safety-First** | PASS | All changes use safe Rust. Existing `unsafe` mmap in commit-graph and packfile is retained, not expanded. No new unsafe blocks. All public APIs return `Result<T, E>`. |
| **II. C-Compatibility** | PASS | Core requirement of this feature. All output must be byte-identical. Commit-graph format matches Git specification exactly. |
| **III. Modular Crates** | PASS | Changes are scoped to existing crates (git-revwalk, git-diff, git-cli, git-odb). No new crates introduced. Dependency direction preserved (git-cli → git-diff → git-index, git-revwalk → git-odb). |
| **IV. Trait-Based Abstraction** | PASS | No new trait boundaries needed. CommitGraph is used as an optional accelerator behind existing interfaces. |
| **V. Test-Driven** | PASS | Each optimization includes correctness tests (byte-comparison vs C git) and performance benchmarks. Existing criterion benchmark suite is the validation tool. |
| **Error Handling** | PASS | thiserror for library errors, anyhow in CLI only. Commit-graph errors gracefully fall back to ODB. |
| **Performance** | PASS | This feature directly addresses constitution's performance standards: mmap I/O, lazy loading, zero-copy parsing, parallel operations. |
| **Byte String Handling** | PASS | No changes to string handling. All paths remain `BString`/`bstr`. |
| **Serialization** | PASS | Commit-graph writer uses manual serialization (no serde), matching the on-disk binary format exactly. |

**Post-Phase 1 Re-Check**: All principles still pass. The parallel stat() optimization uses rayon (already a workspace dependency approved for parallel operations). The commit-graph writer follows the manual serialization standard.

## Project Structure

### Documentation (this feature)

```text
specs/022-perf-optimization/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0 research findings
├── data-model.md        # Data model changes
├── quickstart.md        # Verification guide
├── contracts/
│   ├── commit-graph-api.md     # Commit-graph read/write API
│   ├── parallel-status-api.md  # Parallel status contract
│   └── blame-optimization-api.md # Blame optimization contract
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Task breakdown
```

### Source Code (repository root)

```text
crates/
├── git-revwalk/src/
│   ├── commit_graph/
│   │   ├── mod.rs           # CommitGraph struct (add fanout_offset, contains(), verify())
│   │   ├── parse.rs         # Fanout-accelerated lookup (activate fanout table)
│   │   └── write.rs         # NEW: CommitGraphWriter for commit-graph generation
│   └── walk.rs              # Graph-accelerated traversal, generation pruning
├── git-diff/src/
│   └── worktree.rs          # Parallel stat() via rayon
├── git-cli/src/commands/
│   ├── blame.rs             # Diff-based attribution, cached ODB reads
│   └── commit_graph.rs      # NEW: `commit-graph write` / `commit-graph verify` subcommand
├── git-odb/src/
│   └── lib.rs               # Expose cache size configuration
└── git-index/src/
    └── lib.rs               # Optional: mmap-based index parsing
```

**Structure Decision**: All changes fit within the existing 16-crate workspace. No new crates are needed — commit-graph functionality already lives in git-revwalk, and the CLI subcommand goes in git-cli. The git-diff crate gains a rayon dependency for parallel status.

## Complexity Tracking

> No constitution violations. All optimizations use existing patterns and dependencies.

| Decision | Justification |
|----------|---------------|
| Commit-graph write in git-revwalk (not new crate) | Keeps read + write together. Graph is already parsed here. Constitution principle III allows co-location. |
| rayon in git-diff | Already a workspace dependency. Constitution performance principle explicitly endorses parallel operations. |
| Existing unsafe mmap retained | No new unsafe. Constitution principle I: existing mmap in commit-graph/packfile has documented justification. |

## Implementation Phases

### Phase 1: Commit-Graph Fanout + RevWalk Graph-Accelerated Traversal (P1 — log/rev-list)

**Goal**: Make `log` and `rev-list` on large repos 2x faster by reading commit metadata from commit-graph instead of ODB.

**Changes**:

1. **git-revwalk/src/commit_graph/mod.rs**:
   - Add `oid_fanout_offset: usize` field to `CommitGraph`
   - Add `pub fn contains(&self, oid: &ObjectId) -> bool` method
   - Add `pub fn verify(&self) -> Result<(), RevWalkError>` method

2. **git-revwalk/src/commit_graph/parse.rs**:
   - Store `oid_fanout_offset` during parsing
   - Rewrite `find_oid_position()` to use fanout table: read `fanout[first_byte-1]` and `fanout[first_byte]` to get (lo, hi) range, then binary search within that range
   - Remove the TODO comment at line 164

3. **git-revwalk/src/walk.rs**:
   - Add `read_commit_meta()` method that checks commit-graph first, falls back to ODB
   - Modify `next_date_order()` to use `read_commit_meta()` instead of `read_commit()` for parent enumeration
   - Modify `enqueue()` to store generation from `read_commit_meta()`
   - Remove `#[allow(dead_code)]` from `generation` and `author_date` fields in `WalkEntry`
   - Add generation-based pruning in `mark_hidden()`

4. **Tests**:
   - Unit test: fanout-accelerated lookup returns same results as full binary search
   - Integration test: `gitr log` output matches `git log` on repo with commit-graph
   - Integration test: `gitr rev-list --all` matches `git rev-list --all`

**Estimated impact**: log 42ms → ~24ms, rev-list 28.5ms → ~18ms

### Phase 2: Parallel stat() for Status (P2)

**Goal**: Make `status` on large repos ~1.6x faster by parallelizing filesystem stat calls.

**Changes**:

1. **git-diff/Cargo.toml**: Add `rayon = { workspace = true }` dependency
2. **git-diff/src/worktree.rs**: Split into parallel stat + sequential content diff
3. **Tests**: Integration test for byte-identical status output

**Estimated impact**: status 52.3ms → ~33ms

### Phase 3: Blame Optimization (P3)

**Goal**: Make `blame` on large repos ~1.5x faster by using proper diff and commit-graph acceleration.

**Changes**:

1. **git-cli/src/commands/blame.rs**: Replace set-based with diff-based attribution, use `read_cached()`
2. **Tests**: Integration test for byte-identical blame output

**Estimated impact**: blame 122.5ms → ~75ms

### Phase 4: Commit-Graph Write Support (P4)

**Goal**: Enable gitr to generate commit-graph files compatible with C git.

**Changes**:

1. **git-revwalk/src/commit_graph/write.rs** (new): CommitGraphWriter
2. **git-cli/src/commands/commit_graph.rs** (new): CLI subcommands
3. **Tests**: Round-trip and C git verification tests

### Phase 5: Index Parsing Optimization (Low Priority)

**Goal**: Minor improvement via mmap-based index read.

### Phase 6: Regression Validation

**Goal**: Confirm no regressions, update benchmark documentation.
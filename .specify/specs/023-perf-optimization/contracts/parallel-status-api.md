# Contract: Parallel Status API

**Feature**: 022-perf-optimization
**Crate**: git-diff

## API Changes

### diff_index_to_worktree (behavioral change, no signature change)

```rust
/// Diff the index against the working tree (unstaged changes).
/// Now parallelizes stat() calls across index entries using rayon.
pub fn diff_index_to_worktree(
    repo: &mut Repository,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError>;
```

**Behavioral change**: The internal iteration over index entries switches from sequential `iter()` to parallel `par_iter()` for the stat() phase. The collection of `FileDiff` results preserves deterministic ordering (sorted by path).

## Internal Changes

### Two-Phase Approach

```rust
// Phase 1: Parallel stat (embarrassingly parallel, no shared mutable state)
// Each entry independently calls symlink_metadata() and compares stat data.
// Returns: Vec<(index, StatResult)> where StatResult indicates:
//   - Clean (stat matches)
//   - StatMismatch (needs content comparison)
//   - Deleted (file missing)

// Phase 2: Sequential content comparison (for stat-mismatched entries only)
// Reads file content, computes diff hunks.
// This phase is sequential because it accesses ODB and allocates diff results.
```

## Behavioral Contract

1. **Determinism**: Output order MUST match sequential implementation (sorted by index path).
2. **Correctness**: Parallel stat MUST produce identical `DiffResult` to sequential version.
3. **Error handling**: If any stat() call fails, the error is collected and reported after all parallel work completes.
4. **Thread pool**: Uses rayon's global thread pool (no custom pool creation).
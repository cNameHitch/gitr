# Contract: Blame Optimization API

**Feature**: 022-perf-optimization
**Crate**: git-cli (commands/blame.rs)

## API Changes

### blame_file (internal, behavioral change)

The blame algorithm switches from set-based line detection to proper diff-based line tracking.

## Internal Changes

### find_changed_lines → diff-based attribution

```rust
// Before: HashSet<&str> presence check
// After: Myers diff producing edit operations that map old line numbers to new

/// Compute line-level blame attribution between parent and current file versions.
/// Returns a mapping of current line numbers to attribution status.
fn diff_blame_lines(
    parent_lines: &[&[u8]],
    current_lines: &[&[u8]],
) -> Vec<LineAttribution>;

enum LineAttribution {
    /// Line unchanged from parent (inherited blame).
    Unchanged { parent_line: usize },
    /// Line added or modified in this commit (blame to current commit).
    Changed,
}
```

### Commit-Graph Integration

```rust
// blame_file() uses RevWalk which already loads commit-graph.
// Additional optimization: use read_cached() for repeated ODB access.
fn blame_file(
    repo: &Repository,
    start_oid: &ObjectId,
    file_path: &str,
    // ... existing params
) -> Result<Vec<BlameEntry>, ...>;
```

## Behavioral Contract

1. **Output parity**: Blame output MUST be byte-identical to C git for all test cases.
2. **Performance**: Blame on files with 500+ commits MUST complete within 1.5x of C git.
3. **Correctness**: Diff-based attribution MUST correctly handle insertions, deletions, and unchanged lines across commits.
4. **Fallback**: If commit-graph is unavailable, blame MUST still produce correct results via ODB path.
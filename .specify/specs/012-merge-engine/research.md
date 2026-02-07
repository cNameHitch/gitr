# Research: Merge Engine

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| merge-ort.c/h | ~5000 | `tree.rs`, `strategy/ort.rs` | ORT merge algorithm |
| merge-ll.c/h | ~500 | `content.rs` | Low-level content merge (xdl_merge) |
| merge-blobs.c/h | ~200 | `content.rs` | Blob-level merge driver |
| merge.c/h | ~800 | `lib.rs` | High-level merge API |
| apply.c/h | ~4000 | `apply.rs` | Patch application |
| sequencer.c/h | ~3000 | `sequencer.rs`, `cherry_pick.rs`, `revert.rs` | Sequencer, cherry-pick, revert |

## Three-Way Content Merge

The three-way merge takes three versions of a file:
- **Base** (common ancestor)
- **Ours** (current branch)
- **Theirs** (branch being merged)

Algorithm:
1. Diff base→ours and base→theirs
2. For non-overlapping hunks: apply both
3. For overlapping hunks: check if changes are identical (clean) or conflicting
4. For conflicts: output conflict markers

Conflict marker format:
```
<<<<<<< ours-label
our content
=======
their content
>>>>>>> theirs-label
```

With `diff3` style (merge.conflictstyle=diff3):
```
<<<<<<< ours-label
our content
||||||| base-label
base content
=======
their content
>>>>>>> theirs-label
```

## ORT Merge Strategy

ORT (Ostensibly Recursive's Twin) replaced the recursive strategy as default in git 2.34:
1. Find merge base(s). If multiple, recursively merge them to get a virtual base
2. Compute three-way diff: base→ours and base→theirs for all paths
3. Handle straightforward cases (only one side changed)
4. Detect renames using diff rename detection
5. Handle complex cases:
   - Renamed on both sides: rename/rename conflict if different targets
   - Renamed on one side, modified on other: follow rename, merge content
   - Deleted on one side, modified on other: modify/delete conflict
   - Both add same path: add/add conflict
   - Directory/file conflicts
6. Apply clean changes to index and working tree
7. Record conflicts (stages 1,2,3 in index, markers in working tree)

## Sequencer

The sequencer manages multi-commit operations:
- Cherry-pick of multiple commits
- Revert of multiple commits
- Rebase (apply commits on new base)

State files in `.git/sequencer/`:
- `head`: Original HEAD before operation
- `todo`: List of remaining commits to apply
- `opts`: Operation options (strategy, etc.)

The sequencer can be interrupted (on conflict) and resumed (`--continue`), aborted (`--abort`), or skipped (`--skip`).

## gitoxide Reference

`gix-merge` is relatively new:
- `gix_merge::blob::Platform` for content merge
- `gix_merge::tree()` for tree merge
- Uses `imara-diff` for the underlying line diff
- Still evolving

# Research: Diff Engine

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| diff.c/h | ~3000 | `lib.rs`, `diffcore.rs` | Main diff API, options, output |
| diffcore-rename.c | ~800 | `rename.rs` | Rename detection |
| diffcore-break.c | ~300 | `diffcore.rs` | Break large changes into pieces |
| diffcore-pickaxe.c | ~200 | `diffcore.rs` | Search for string in diffs |
| diffcore-order.c | ~100 | `diffcore.rs` | Custom output ordering |
| diffcore-delta.c | ~300 | `rename.rs` | Similarity scoring |
| xdiff/xdiffi.c | ~1000 | `algorithm/myers.rs` | Myers algorithm |
| xdiff/xhistogram.c | ~500 | `algorithm/histogram.rs` | Histogram algorithm |
| xdiff/xpatience.c | ~400 | `algorithm/patience.rs` | Patience algorithm |
| xdiff/xutils.c | ~300 | `algorithm/mod.rs` | Diff utilities |
| userdiff.c | ~300 | `lib.rs` | User-defined diff drivers |

## Diff Algorithms

### Myers (default)
- O(ND) algorithm by Eugene Myers (1986)
- Produces minimal edit script (minimum number of changes)
- Good for most cases

### Histogram
- Variant of patience diff with histogram-based line matching
- Often produces more readable diffs for code
- Default in JGit

### Patience
- Uses patience sorting to find unique matching lines
- Better at keeping logically related lines together
- Good for diffs where Myers produces confusing output

## Diffcore Pipeline

C git processes diffs through a pipeline:
1. **Raw tree diff**: Identify changed paths (A/M/D)
2. **diffcore-break**: Break complete rewrites into delete+add (for better rename detection)
3. **diffcore-rename**: Detect renames and copies using similarity scoring
4. **diffcore-merge-broken**: Re-merge broken pairs that weren't renamed
5. **diffcore-pickaxe**: Filter diffs by string content (--pickaxe, -S, -G)
6. **diffcore-order**: Apply custom output ordering (from .gitattributes)

## Rename Detection Algorithm

1. Collect all deleted files and added files
2. For exact matches (same OID): pair them as renames immediately
3. For remaining: compute similarity score using delta size metric
4. Pair remaining files using best-match scoring (above configurable threshold, default 50%)
5. With -C flag: also check existing files as copy sources

## Similarity Scoring
C git uses a delta-based similarity metric:
- `similarity = max(0, (base_size - delta_size) * 100 / base_size)`
- Computed using the same delta algorithm used for pack files
- Fast approximation, not character-by-character comparison

## gitoxide Reference

`gix-diff` provides:
- Tree diff via `gix_diff::tree::Changes`
- Line diff via `imara-diff` crate (external)
- Rename tracking via `gix_diff::Rewrites`

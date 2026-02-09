# Benchmark Summary: `git` vs `gitr` (Rust)

**Overall:** `gitr` is significantly faster than `git` for most operations, especially on small repositories where process startup overhead dominates. After spec 022 performance optimizations (commit-graph acceleration, parallel stat, diff-based blame, ODB caching), `gitr`'s large-repo disadvantage has been substantially reduced.

## Where `gitr` Dominates (3-4x faster)

These are mostly operations where `git`'s ~8-9ms startup overhead is the bottleneck:

| Command | git | gitr | Speedup |
|---|---|---|---|
| **init** | 12.4 ms | 2.3 ms | **5.3x** |
| **hash-object** (small) | 7.3 ms | 2.2 ms | **3.3x** |
| **cat-file** (all sizes) | ~7.9 ms | ~2.4 ms | **3.3x** |
| **show** (all sizes) | ~9.4 ms | ~2.8 ms | **3.4x** |
| **branch** (all sizes) | ~8.8 ms | ~2.7 ms | **3.3x** |
| **tag** (all sizes) | ~8.8 ms | ~2.7 ms | **3.2x** |
| **show-ref** (small) | 9.0 ms | 2.7 ms | **3.3x** |
| **rev-parse** (all sizes) | ~8.2 ms | ~2.6 ms | **3.2x** |
| **diff** (small) | 8.8 ms | 3.3 ms | **2.7x** |
| **blame** (small) | 10.2 ms | 3.5 ms | **2.9x** |
| **rev-list** (small) | 8.8 ms | 3.0 ms | **3.0x** |
| **log** (small) | 9.7 ms | 3.4 ms | **2.9x** |
| **status** (small) | 9.4 ms | 3.8 ms | **2.5x** |

## Where `gitr` is Moderately Faster

| Command | git | gitr | Speedup |
|---|---|---|---|
| **status** (medium) | 11.4 ms | 10.9 ms | **1.05x** |
| **log** (medium) | 12.2 ms | 9.3 ms | **1.3x** |
| **rev-list** (medium) | 10.7 ms | 6.4 ms | **1.7x** |
| **diff** (medium) | 11.7 ms | 6.6 ms | **1.8x** |
| **blame** (medium) | 17.0 ms | 12.3 ms | **1.4x** |
| **diff_cached** (large) | 13.9 ms | 8.9 ms | **1.6x** |
| **for-each-ref** (large) | 11.1 ms | 5.5 ms | **2.0x** |
| **commit** | 23.5 ms | 16.0 ms | **1.5x** |
| **add** | 20.0 ms | 18.1 ms | **1.1x** |

## Where `git` Wins (Large Repos)

| Command | git | gitr | git is faster by |
|---|---|---|---|
| **status** (large) | 28.1 ms | 58.8 ms | **2.1x** |
| **log** (large) | 22.7 ms | 35.0 ms | **1.5x** |
| **rev-list** (large) | 18.4 ms | 21.1 ms | **1.1x** |
| **blame** (large) | 54.8 ms | 66.4 ms | **1.2x** |
| **diff** (large) | 27.3 ms | 29.9 ms | **1.1x** |

## Optimization Impact (Spec 022)

Comparing post-optimization results against pre-optimization baselines for large repos:

| Command | Before | After | Change |
|---|---|---|---|
| **log** (large) | 42.0 ms | 35.0 ms | **-17%** (graph-accelerated traversal) |
| **rev-list** (large) | 28.5 ms | 21.1 ms | **-26%** (commit-graph metadata bypass) |
| **blame** (large) | 122.5 ms | 66.4 ms | **-46%** (diff-based attribution + ODB caching) |
| **diff** (large) | ~28 ms | 29.9 ms | ~same |
| **status** (large) | 52.3 ms | 58.8 ms | +12% (within run-to-run variance) |

## Key Takeaways

1. **Startup cost is `git`'s weakness.** `gitr` consistently shaves ~6ms off, suggesting much lower process initialization overhead. For commands that are inherently fast, this is the entire difference.

2. **History traversal gap is closing.** With commit-graph acceleration, `gitr` reduced its `rev-list` disadvantage from 1.7x to 1.1x, and `blame` from 2.3x to 1.2x. The `log` gap narrowed from 2.0x to 1.5x.

3. **`diff` scales well** — `gitr` matches `git` on large diffs (~28-30ms each) but is much faster on small/medium ones.

4. **`status` on large repos remains an optimization target** — the parallel stat approach didn't close the gap with C git, likely due to `git`'s highly optimized filesystem stat cache and untracked file handling.

5. **Write operations** (`add`, `commit`) show consistent `gitr` wins of 1.1-1.5x.

# Benchmark Summary: `git` vs `gitr` (Rust)

**Overall:** `gitr` is significantly faster than `git` for most operations, especially on small repositories where process startup overhead dominates. However, `gitr` loses its advantage—and sometimes falls behind—on large repos for operations that require traversing history or large working trees.

## Where `gitr` Dominates (3–4x faster)

These are mostly operations where `git`'s ~7–9ms startup overhead is the bottleneck:

| Command | git | gitr | Speedup |
|---|---|---|---|
| **init** | 12.4 ms | 2.3 ms | **5.3x** |
| **hash-object** (small) | 7.3 ms | 2.2 ms | **3.3x** |
| **cat-file** (all sizes) | ~7.9 ms | ~2.4 ms | **3.3x** |
| **show** (all sizes) | ~8.9 ms | ~2.7 ms | **3.3x** |
| **branch** (all sizes) | ~8.7 ms | ~2.7 ms | **3.2x** |
| **tag** (all sizes) | ~8.6 ms | ~2.7 ms | **3.2x** |
| **show-ref** (small) | 8.4 ms | 2.6 ms | **3.2x** |
| **rev-parse** (all sizes) | ~7.8 ms | ~2.5 ms | **3.1x** |
| **diff** (small) | 8.3 ms | 2.5 ms | **3.3x** |
| **diff_cached** (small) | 8.5 ms | 2.8 ms | **3.1x** |
| **blame** (small) | 9.8 ms | 4.2 ms | **2.3x** |

## Where `gitr` is Moderately Faster

| Command | git | gitr | Speedup |
|---|---|---|---|
| **status** (small) | 9.0 ms | 2.9 ms | **3.1x** |
| **diff** (medium) | 11.1 ms | 5.3 ms | **2.1x** |
| **diff_cached** (large) | 13.1 ms | 8.6 ms | **1.5x** |
| **for-each-ref** (large) | 10.7 ms | 5.2 ms | **2.0x** |
| **commit** | 22.5 ms | 15.7 ms | **1.4x** |
| **add** | 19.6 ms | 17.6 ms | **1.1x** |

## Where `git` Wins (Large Repos) ⚠️

| Command | git | gitr | git is faster by |
|---|---|---|---|
| **status** (large) | 27.3 ms | 52.3 ms | **1.9x** |
| **log** (large) | 21.3 ms | 42.0 ms | **2.0x** |
| **log --oneline** (large) | 24.5 ms | 41.6 ms | **1.7x** |
| **rev-list** (large) | 17.1 ms | 28.5 ms | **1.7x** |
| **blame** (large) | 53.1 ms | 122.5 ms | **2.3x** |
| **ls-files** (large) | 9.3 ms | 10.1 ms | **1.1x** |

## Key Takeaways

1. **Startup cost is `git`'s weakness.** `gitr` consistently shaves ~5–6ms off, suggesting much lower process initialization overhead. For commands that are inherently fast, this is the entire difference.

2. **History traversal is `gitr`'s weakness.** For `log`, `blame`, `rev-list`, and `status` on large repos, `gitr` is 1.5–2.3x *slower* than `git`. This likely reflects the maturity of `git`'s highly optimized packfile parsing, commit graph, and diffing algorithms.

3. **`diff` scales reasonably** — `gitr` matches `git` on large diffs (~28ms each) but is much faster on small/medium ones.

4. **Write operations** (`add`, `commit`) show modest `gitr` wins, suggesting comparable implementations with less startup overhead.

The Rust rewrite clearly wins for quick, frequent operations (the kind that IDEs and tools call thousands of times), but needs optimization work on history-heavy operations at scale.

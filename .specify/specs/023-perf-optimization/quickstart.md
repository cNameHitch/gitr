# Quickstart: Large-Repo Performance Optimization

**Feature**: 022-perf-optimization
**Date**: 2026-02-08

## Prerequisites

- Rust 1.75+ toolchain
- Existing gitr workspace builds: `cargo build --release`
- C git installed (for comparison benchmarks)
- A large test repository (10,000+ commits, 5,000+ files) — or use the benchmark harness which generates one

## Verification Steps

### 1. Run baseline benchmarks (before changes)

```bash
cargo bench -p git-cli --bench perf_compare -- bench_log_large bench_status_large bench_revlist_large bench_blame_large
```

Save results for comparison.

### 2. After implementing commit-graph optimizations

```bash
# Verify commit-graph read works with C git's graph
cd /path/to/large-repo
git commit-graph write   # generate with C git
cargo run --release -- log | head -20
# Output must be byte-identical to: git log | head -20

# Verify gitr's commit-graph write
cargo run --release -- commit-graph write
git commit-graph verify  # C git validates gitr's output
```

### 3. After implementing parallel status

```bash
cd /path/to/large-repo
diff <(cargo run --release -- status) <(git status)
# Must produce no output (byte-identical)

cargo bench -p git-cli --bench perf_compare -- bench_status
```

### 4. After implementing blame optimization

```bash
cd /path/to/large-repo
diff <(cargo run --release -- blame src/main.rs) <(git blame src/main.rs)
# Must produce no output (byte-identical)
```

### 5. Regression check

```bash
# Run full benchmark suite across all repo sizes
cargo bench -p git-cli --bench perf_compare

# Verify small/medium repos don't regress > 5%
# Compare against baseline saved in step 1
```

### 6. Full test suite

```bash
cargo test --workspace
cargo clippy --workspace
```

## Key Files to Modify

| Crate | File | Change |
|-------|------|--------|
| git-revwalk | `src/commit_graph/mod.rs` | Add fanout offset field, `contains()`, `verify()` |
| git-revwalk | `src/commit_graph/parse.rs` | Fanout-accelerated lookup |
| git-revwalk | `src/commit_graph/write.rs` | **NEW** — Commit-graph writer |
| git-revwalk | `src/walk.rs` | Graph-accelerated traversal, generation pruning |
| git-diff | `src/worktree.rs` | Parallel stat() via rayon |
| git-diff | `Cargo.toml` | Add rayon dependency |
| git-cli | `src/commands/blame.rs` | Diff-based attribution, cached ODB reads |
| git-cli | `src/commands/commit_graph.rs` | **NEW** — `commit-graph write` subcommand |
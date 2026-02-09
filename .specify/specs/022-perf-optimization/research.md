# Research: Large-Repo Performance Optimization

**Feature**: 022-perf-optimization
**Date**: 2026-02-08

## R1: Commit-Graph Read Path — Fanout Optimization

**Decision**: Use the existing fanout table (already parsed and stored) to narrow binary search in `find_oid_position()` from O(log N) over all commits to O(log(N/256)) over a single fanout bucket.

**Rationale**: The commit-graph parser already reads and validates the OID Fanout chunk (`CHUNK_OID_FANOUT`), but `find_oid_position()` in `parse.rs:151-185` ignores the fanout data entirely — it does a full binary search over all N commits. There is even a TODO comment at line 164: "TODO: Use fanout for O(log n) binary search." The fanout offset is stored but the `CommitGraph` struct doesn't retain it. Adding the `oid_fanout_offset` field and using it in lookups is a minimal change for a significant speedup on large graphs.

**Alternatives Considered**:
- Hash table lookup: Higher memory, not standard for commit-graph format
- No change: Acceptable for small graphs but limits scalability

## R2: Generation Number Pruning in RevWalk

**Decision**: Use generation numbers stored in `WalkEntry.generation` to prune unreachable commits during date-ordered walks and hidden-ancestor marking.

**Rationale**: The `walk.rs` code already extracts generation numbers from the commit-graph (lines 218-223) and stores them in `WalkEntry.generation`, but the field is `#[allow(dead_code)]` and unused. C git uses generation numbers to skip commits that are provably unreachable: if the maximum generation of all included tips is less than a commit's generation, it cannot be an ancestor. This avoids reading commits from packfiles that will never appear in output. The `mark_hidden()` method (lines 262-277) eagerly walks all ancestors of hidden commits — generation numbers can short-circuit this.

**Alternatives Considered**:
- Corrected commit dates (generation v2): More complex, marginal benefit for this use case
- Bloom filters for changed paths: Useful for path-limited log but not general rev-list

## R3: Commit-Graph as Commit Source (Bypass ODB for Metadata)

**Decision**: When a commit-graph is available, read commit metadata (parents, tree, timestamp) directly from the graph instead of through the ODB. Only fall back to ODB for commits not in the graph or when full commit content is needed.

**Rationale**: Currently, `read_commit()` in `walk.rs:242-252` always calls `repo.odb().read(oid)` which decompresses a packfile entry, parses the commit object, and allocates strings for author/committer. The commit-graph already has the parent OIDs, tree OID, and timestamp in a zero-copy memory-mapped format. For pure traversal (rev-list, merge-base), the full commit object is never needed — only the graph structure. This is the single largest optimization for `log` and `rev-list`.

**Alternatives Considered**:
- Caching parsed commits in ODB: Already exists (LRU with 1024 entries) but still requires decompression on first access
- Prefetching commits: Helps sequential access but doesn't eliminate decompression

## R4: Object Cache Tuning for Blame

**Decision**: Increase the ODB object cache size during blame operations and ensure the `read_cached()` path is used for repeated object access during blame's history walk.

**Rationale**: The ODB already has an LRU cache (`ObjectCache` with 1024 entries in `git-object/src/cache.rs`), but blame's `blame_file()` function in `git-cli/src/commands/blame.rs` calls `repo.odb().read()` (not `read_cached()`) for each commit. Blame accesses the same tree and blob objects repeatedly across adjacent commits. Using the cached path and potentially increasing the cache size during blame can reduce redundant packfile reads.

**Alternatives Considered**:
- Dedicated blame-specific cache: Over-engineered, the existing LRU is sufficient
- Unlimited cache: Memory-unsafe for very large repos

## R5: Parallel stat() for Status

**Decision**: Use `rayon::par_iter()` to parallelize `std::fs::symlink_metadata()` calls during `diff_index_to_worktree()` in `git-diff/src/worktree.rs`.

**Rationale**: The current implementation iterates index entries sequentially (lines 61-131), calling `symlink_metadata()` once per file. On repos with 5,000+ files, the stat() syscalls dominate wall-clock time. C git parallelizes this with threading. The `rayon` crate is already a workspace dependency but unused in git-diff. The fast path (stat matches → skip) makes this embarrassingly parallel since entries are independent.

**Alternatives Considered**:
- Filesystem monitor (fsmonitor): More complex, requires external daemon, not portable
- Index mmap: Minor benefit vs parallelizing stat which is the actual bottleneck

## R6: Blame Algorithm — Use Diff Engine

**Decision**: Replace the simple set-based `find_changed_lines()` in blame with proper Myers diff to get accurate line-level attribution including moved lines.

**Rationale**: The current blame implementation (blame.rs:377-398) uses `HashSet<&str>` to detect changed lines — this tracks exact string presence but cannot detect line movement or insertion positions. C git's blame uses the actual diff algorithm to map old line numbers to new line numbers, which is essential for accurate blame attribution. The diff engine is already available in the git-diff crate with Myers, patience, and histogram algorithms.

**Alternatives Considered**:
- Keep set-based approach with tweaks: Faster but less accurate, won't match C git output
- Copy detection: Future enhancement, not needed for parity

## R7: Commit-Graph Write Support

**Decision**: Implement `commit-graph write` as a new subcommand that generates commit-graph files matching C git's format exactly.

**Rationale**: While read support enables consuming graphs generated by C git, write support allows gitr to be self-sufficient and enables the `gitr gc` / maintenance path. The format is well-specified and the existing parser can be mirrored for writing.

**Alternatives Considered**:
- Rely on C git for graph generation: Works but defeats the purpose of a standalone tool
- Incremental-only writes: More complex; start with full rewrites

## R8: Index Parsing — Memory-Mapped I/O

**Decision**: Replace `std::fs::read()` with `memmap2::Mmap` for index file parsing in `git-index/src/lib.rs:136-137`.

**Rationale**: The current implementation reads the entire index file into a `Vec<u8>`. For large indexes (5,000+ entries, potentially several MB), memory mapping avoids the copy and lets the OS manage page faults efficiently. The packfile reader already uses mmap successfully. However, the actual bottleneck is stat() calls during status, not index parsing, so this is a lower-priority optimization.

**Alternatives Considered**:
- Lazy parsing: More complex, index is typically read fully anyway
- Streaming parser: Overkill, entire index fits comfortably in memory
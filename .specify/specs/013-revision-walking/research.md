# Research: Revision Walking

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| revision.c/h | ~3500 | `walk.rs`, `range.rs`, `sort.rs` | Core revision machinery |
| commit-graph.c/h | ~2000 | `commit_graph/` | Commit-graph reading/writing |
| commit-reach.c/h | ~800 | `merge_base.rs` | Merge base, reachability |
| reachable.c | ~200 | `objects.rs` | Reachable object listing |
| list-objects.c/h | ~500 | `objects.rs` | Object enumeration |
| list-objects-filter.c/h | ~500 | `filter.rs` | Partial clone filters |
| graph.c/h | ~800 | `graph_draw.rs` | ASCII graph drawing |
| log-tree.c/h | ~1000 | `pretty.rs` | Log tree output |
| pretty.c/h | ~1500 | `pretty.rs` | Format specifier handling |

## Commit-Graph Format

Binary file at `.git/objects/info/commit-graph`:
```
Header: CGPH (magic) | version (1) | hash_version | chunks | base_graphs
Chunk table: [(chunk_id, offset), ...] terminated by zero chunk
Chunks:
  - OID Fanout (256 × 4 bytes)
  - OID Lookup (N × hash_len sorted OIDs)
  - Commit Data (N × (tree_oid + parent1 + parent2 + date + generation))
  - Extra Edge List (for octopus merges with >2 parents)
  - (optional) Generation Data (v2 generation numbers)
  - (optional) Bloom Filter Index + Data
Trailer: checksum
```

Key fields per commit:
- Tree OID (hash_len bytes)
- Parent 1 index (4 bytes, 0x70000000 for none)
- Parent 2 index (4 bytes, special values for >2 parents)
- Generation number (bits 31-2) + commit date offset (bits 1-0 ... complex)

## Sorting Algorithms

- **Chronological (default)**: Sort by committer date, newest first. Use max-heap priority queue.
- **Topological**: Kahn's algorithm — emit commit only when all children have been emitted.
- **Author date**: Like chronological but uses author date instead of committer date.
- **Reverse**: Walk in the opposite direction (oldest first).

## Merge Base Algorithm

C git uses a "paint" algorithm:
1. Start BFS from both commits simultaneously
2. Color commits reachable from A as PARENT1, from B as PARENT2
3. Commits reachable from both get color STALE
4. The merge base(s) are commits that are PARENT1|PARENT2 but whose parents are all STALE

With generation numbers from commit-graph:
- Can quickly prune commits below the minimum generation of either branch
- Dramatically speeds up merge-base for distant branches

## Pretty-Print Format Specifiers

| Specifier | Meaning |
|-----------|---------|
| %H | Full commit hash |
| %h | Abbreviated commit hash |
| %T | Full tree hash |
| %P | Full parent hashes |
| %an | Author name |
| %ae | Author email |
| %ad | Author date (respects --date=) |
| %cn | Committer name |
| %ce | Committer email |
| %cd | Committer date |
| %s | Subject (first line) |
| %b | Body |
| %N | Notes |
| %Cred, %Cgreen, etc. | Color |
| %w(width,indent1,indent2) | Column wrapping |

## gitoxide Reference

`gix-traverse` provides:
- `gix_traverse::commit::Simple` for basic walk
- `gix_traverse::commit::Topo` for topological
- `gix_commitgraph` for commit-graph parsing
- Separate `gix-revwalk` for higher-level revision walking

# Data Model: Large-Repo Performance Optimization

**Feature**: 022-perf-optimization
**Date**: 2026-02-08

## Entities

### CommitGraph (enhanced — existing in git-revwalk)

The existing `CommitGraph` struct gains the fanout offset field to enable fanout-accelerated lookups.

**Fields**:
- `data: Mmap` — Memory-mapped commit-graph file data
- `num_commits: u32` — Total commits in the graph
- `oid_fanout_offset: usize` — **(NEW)** Offset to the OID Fanout chunk (256 × 4-byte cumulative counts)
- `oid_lookup_offset: usize` — Offset to the OID Lookup chunk
- `commit_data_offset: usize` — Offset to the Commit Data chunk
- `extra_edges_offset: Option<usize>` — Offset to the Extra Edge List chunk (octopus merges)
- `hash_len: usize` — Hash length (20 for SHA-1, 32 for SHA-256)

**Relationships**: Used by `RevWalk` for accelerated commit traversal. Replaces ODB reads when commit metadata (parents, tree, timestamp) is sufficient.

### CommitGraphEntry (unchanged — existing in git-revwalk)

**Fields**:
- `tree_oid: ObjectId` — Root tree OID
- `parent_oids: Vec<ObjectId>` — Parent commit OIDs
- `generation: u32` — Topological generation number (level + 1, 0 = unknown)
- `commit_time: i64` — Committer timestamp (seconds since epoch)

**Validation**: Generation numbers must satisfy: for all parents P of commit C, `C.generation > P.generation`. Generation 0 means the commit was not in the graph.

### CommitGraphWriter (new)

Responsible for serializing commit metadata into the commit-graph binary format.

**Fields**:
- `commits: Vec<CommitEntry>` — Sorted list of commits to include
- `hash_algo: HashAlgorithm` — SHA-1 or SHA-256

**CommitEntry** (internal):
- `oid: ObjectId`
- `tree_oid: ObjectId`
- `parent_oids: Vec<ObjectId>`
- `generation: u32` — Computed via topological traversal
- `commit_time: i64`

**Output format**: Binary file matching Git's `commit-graph-format.txt` specification:
1. Header: signature "CGPH", version 1, hash version, chunk count
2. Chunk TOC: chunk IDs + offsets
3. OID Fanout: 256 × 4-byte cumulative counts
4. OID Lookup: sorted OIDs
5. Commit Data: tree OID + parent indices + generation/date
6. Extra Edges: octopus merge overflow parents
7. Checksum: trailing hash

### ObjectCache (existing in git-object)

**Fields**:
- `cache: LruCache<ObjectId, Object>` — Bounded LRU cache

**Changes**: No structural changes. Behavioral change: blame and revwalk code paths will use `read_cached()` instead of `read()` where beneficial.

### WalkEntry (existing in git-revwalk — activation of unused fields)

**Fields** (no structural change):
- `oid: ObjectId`
- `commit_date: i64` — Sort key
- `author_date: i64` — **(ACTIVATED)** Used for generation-based pruning
- `generation: u32` — **(ACTIVATED)** Used for generation-based pruning
- `insertion_ctr: u64` — Stable tie-breaker

**State transitions**: The `#[allow(dead_code)]` annotations on `author_date` and `generation` are removed once pruning logic is implemented.

## Data Flow Changes

### Before (current)

```
RevWalk.next() → read_commit(oid) → ODB.read(oid) → PackFile.read_object()
    → decompress zlib → parse commit object → return Commit struct
    → extract parents, date → enqueue parents
```

### After (optimized)

```
RevWalk.next() → commit_graph.lookup(oid)
    → if found: return parents, tree, date, generation from mmap (zero-copy)
    → if not found: fallback to ODB.read(oid) (unchanged path)
    → generation pruning: skip if generation > max_included_generation
    → enqueue parents (from graph data, no ODB hit)
```

### Status Data Flow

```
Before: entries.iter() → sequential stat() per file
After:  entries.par_iter() → parallel stat() via rayon thread pool
```
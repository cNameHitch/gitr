# Contract: Commit-Graph API

**Feature**: 022-perf-optimization
**Crate**: git-revwalk

## Public API Changes

### CommitGraph (enhanced)

```rust
impl CommitGraph {
    /// Open a commit-graph file from a path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RevWalkError>;

    /// Try to open from a repository (single file or chain).
    pub fn open_from_repo(repo: &Repository) -> Result<Self, RevWalkError>;

    /// Look up a commit by OID. Returns None if not in graph.
    pub fn lookup(&self, oid: &ObjectId) -> Option<CommitGraphEntry>;

    /// Fast existence check without full entry parsing.
    pub fn contains(&self, oid: &ObjectId) -> bool;  // NEW

    /// Number of commits in the graph.
    pub fn num_commits(&self) -> u32;

    /// Validate checksum integrity.
    pub fn verify(&self) -> Result<(), RevWalkError>;  // NEW
}
```

### CommitGraphWriter (new)

```rust
pub struct CommitGraphWriter { /* ... */ }

impl CommitGraphWriter {
    /// Create a writer for the given hash algorithm.
    pub fn new(hash_algo: HashAlgorithm) -> Self;

    /// Add a commit to be included in the graph.
    pub fn add_commit(
        &mut self,
        oid: ObjectId,
        tree_oid: ObjectId,
        parents: Vec<ObjectId>,
        commit_time: i64,
    );

    /// Compute generation numbers and write the graph file.
    /// Returns the checksum of the written file.
    pub fn write(self, path: impl AsRef<Path>) -> Result<ObjectId, RevWalkError>;
}
```

## Internal API Changes

### RevWalk — Graph-Accelerated Traversal

```rust
impl RevWalk<'_> {
    /// Read commit metadata, preferring commit-graph over ODB.
    /// Returns (parents, tree_oid, commit_time, generation).
    fn read_commit_meta(&self, oid: &ObjectId)
        -> Result<CommitMeta, RevWalkError>;  // NEW internal method

    /// Check if a commit can be pruned based on generation number.
    fn is_prunable(&self, generation: u32) -> bool;  // NEW internal method
}

/// Lightweight commit metadata (no author/committer strings).
struct CommitMeta {
    parents: Vec<ObjectId>,
    tree_oid: ObjectId,
    commit_time: i64,
    generation: u32,
}
```

## Behavioral Contract

1. **Correctness**: All operations MUST produce byte-identical output whether commit-graph is present or not.
2. **Fallback**: If commit-graph lookup fails for any OID, fall back to ODB transparently.
3. **Checksums**: Corrupt commit-graph files MUST be detected and silently ignored (fallback to ODB).
4. **Thread safety**: CommitGraph is read-only after construction and safe to share via `&CommitGraph`.
# Data Model: Revision Walking

## Core Types

```rust
use git_hash::ObjectId;
use git_object::Commit;

/// Revision walk iterator.
pub struct RevWalk {
    repo: Repository,
    queue: BinaryHeap<WalkEntry>,
    seen: OidSet,
    sort: SortOrder,
    options: WalkOptions,
    commit_graph: Option<CommitGraph>,
}

struct WalkEntry {
    oid: ObjectId,
    generation: u32,
    date: i64,
}

/// Sort order for commit traversal.
#[derive(Debug, Clone, Copy)]
pub enum SortOrder {
    /// By committer date, newest first (default)
    Chronological,
    /// Topological: parents after children
    Topological,
    /// By author date
    AuthorDate,
    /// Reverse chronological
    Reverse,
}

/// Options for revision walking.
#[derive(Debug, Clone)]
pub struct WalkOptions {
    pub sort: SortOrder,
    pub first_parent_only: bool,
    pub ancestry_path: bool,
    pub max_count: Option<usize>,
    pub skip: Option<usize>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub author_pattern: Option<String>,
    pub committer_pattern: Option<String>,
    pub grep_pattern: Option<String>,
}

impl RevWalk {
    /// Create a new revision walker
    pub fn new(repo: &Repository) -> Result<Self, RevWalkError>;

    /// Add a starting commit (positive reference)
    pub fn push(&mut self, oid: ObjectId) -> Result<(), RevWalkError>;

    /// Add an exclusion commit (negative reference, like ^A)
    pub fn hide(&mut self, oid: ObjectId) -> Result<(), RevWalkError>;

    /// Push all refs as starting points (--all)
    pub fn push_all(&mut self) -> Result<(), RevWalkError>;

    /// Push all branches
    pub fn push_branches(&mut self) -> Result<(), RevWalkError>;

    /// Push all tags
    pub fn push_tags(&mut self) -> Result<(), RevWalkError>;

    /// Set the sort order
    pub fn set_sort(&mut self, sort: SortOrder);

    /// Set walk options
    pub fn set_options(&mut self, options: WalkOptions);

    /// Parse a revision range ("A..B", "A...B", "^A B")
    pub fn push_range(&mut self, range: &str) -> Result<(), RevWalkError>;
}

impl Iterator for RevWalk {
    type Item = Result<ObjectId, RevWalkError>;
}

/// Parsed revision range.
pub struct RevisionRange {
    pub include: Vec<ObjectId>,
    pub exclude: Vec<ObjectId>,
    pub symmetric: bool,
}

impl RevisionRange {
    pub fn parse(repo: &Repository, spec: &str) -> Result<Self, RevWalkError>;
}

/// Find the merge base(s) of two commits.
pub fn merge_base(
    repo: &Repository,
    a: &ObjectId,
    b: &ObjectId,
) -> Result<Vec<ObjectId>, RevWalkError>;

/// Find the single best merge base.
pub fn merge_base_one(
    repo: &Repository,
    a: &ObjectId,
    b: &ObjectId,
) -> Result<Option<ObjectId>, RevWalkError>;

/// Check if commit A is an ancestor of commit B.
pub fn is_ancestor(
    repo: &Repository,
    ancestor: &ObjectId,
    descendant: &ObjectId,
) -> Result<bool, RevWalkError>;

/// Commit-graph file reader.
pub struct CommitGraph {
    data: Mmap,
    num_commits: u32,
    oid_lookup_offset: usize,
    commit_data_offset: usize,
    extra_edges_offset: Option<usize>,
}

impl CommitGraph {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RevWalkError>;
    pub fn lookup(&self, oid: &ObjectId) -> Option<CommitGraphEntry>;
    pub fn num_commits(&self) -> u32;
}

pub struct CommitGraphEntry {
    pub tree_oid: ObjectId,
    pub parents: Vec<u32>,  // indices into commit-graph
    pub generation: u32,
    pub commit_time: i64,
}

/// Pretty-print a commit with the given format.
pub fn format_commit(
    commit: &Commit,
    oid: &ObjectId,
    format: &str,
    options: &FormatOptions,
) -> String;

pub struct FormatOptions {
    pub date_format: DateFormat,
    pub color: bool,
    pub abbrev_len: usize,
}

/// Draw ASCII graph for commit history.
pub struct GraphDrawer {
    // Internal state for tracking active branches
}

impl GraphDrawer {
    pub fn new() -> Self;
    pub fn draw_commit(&mut self, oid: &ObjectId, parents: &[ObjectId]) -> Vec<String>;
}

/// List all objects reachable from a set of commits.
pub fn list_objects(
    repo: &Repository,
    include: &[ObjectId],
    exclude: &[ObjectId],
    filter: Option<&ObjectFilter>,
) -> Result<Vec<ObjectId>, RevWalkError>;

/// Object filter for partial clone.
pub enum ObjectFilter {
    BlobNone,
    BlobLimit(u64),
    TreeDepth(u32),
}

#[derive(Debug, thiserror::Error)]
pub enum RevWalkError {
    #[error("invalid revision: {0}")]
    InvalidRevision(String),
    #[error("commit not found: {0}")]
    CommitNotFound(ObjectId),
    #[error("invalid commit-graph: {0}")]
    InvalidCommitGraph(String),
    #[error(transparent)]
    Odb(#[from] git_odb::OdbError),
    #[error(transparent)]
    Ref(#[from] git_ref::RefError),
    #[error(transparent)]
    Repo(#[from] git_repository::RepoError),
}
```

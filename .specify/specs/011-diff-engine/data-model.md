# Data Model: Diff Engine

## Core Types

```rust
use git_hash::ObjectId;
use git_object::FileMode;
use bstr::{BStr, BString};

/// Options controlling diff behavior.
#[derive(Debug, Clone)]
pub struct DiffOptions {
    pub algorithm: DiffAlgorithm,
    pub context_lines: u32,
    pub detect_renames: bool,
    pub rename_threshold: u8,  // 0-100, default 50
    pub detect_copies: bool,
    pub copy_threshold: u8,
    pub color: bool,
    pub stat_width: Option<usize>,
    pub output_format: DiffOutputFormat,
    pub pathspec: Option<Pathspec>,
}

/// Available diff algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffAlgorithm {
    Myers,
    Histogram,
    Patience,
    Minimal,  // Myers with minimal=true
}

/// Diff output format
#[derive(Debug, Clone, Copy)]
pub enum DiffOutputFormat {
    Unified,
    Stat,
    ShortStat,
    NumStat,
    Raw,
    NameOnly,
    NameStatus,
    Summary,
}

/// Result of diffing two trees or a working tree.
pub struct DiffResult {
    pub files: Vec<FileDiff>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool;
    pub fn num_files_changed(&self) -> usize;
    pub fn insertions(&self) -> usize;
    pub fn deletions(&self) -> usize;
    /// Format the diff result for output
    pub fn format(&self, options: &DiffOptions) -> String;
}

/// Diff for a single file.
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub status: FileStatus,
    pub old_path: Option<BString>,
    pub new_path: Option<BString>,
    pub old_mode: Option<FileMode>,
    pub new_mode: Option<FileMode>,
    pub old_oid: Option<ObjectId>,
    pub new_oid: Option<ObjectId>,
    pub hunks: Vec<Hunk>,
    pub is_binary: bool,
    pub similarity: Option<u8>,  // For renames/copies
}

/// File-level change status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    TypeChanged,
    Unmerged,
}

impl FileStatus {
    pub fn as_char(&self) -> char;  // A, D, M, R, C, T, U
}

/// A contiguous region of changes.
#[derive(Debug, Clone)]
pub struct Hunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub header: Option<BString>,  // Function/class name from context
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff hunk.
#[derive(Debug, Clone)]
pub enum DiffLine {
    Context(BString),
    Addition(BString),
    Deletion(BString),
}

/// Line-level diff between two byte sequences.
pub fn diff_lines(
    old: &[u8],
    new: &[u8],
    algorithm: DiffAlgorithm,
) -> Vec<Hunk>;

/// Tree-to-tree diff.
pub fn diff_trees(
    repo: &Repository,
    old_tree: Option<&ObjectId>,
    new_tree: Option<&ObjectId>,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError>;

/// Index-to-working-tree diff.
pub fn diff_index_to_worktree(
    repo: &Repository,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError>;

/// HEAD-to-index diff (staged changes).
pub fn diff_head_to_index(
    repo: &Repository,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError>;

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("failed to read object {oid}: {source}")]
    ObjectRead {
        oid: ObjectId,
        #[source]
        source: git_odb::OdbError,
    },

    #[error("failed to diff binary file: {0}")]
    BinaryFile(BString),

    #[error(transparent)]
    Repo(#[from] git_repository::RepoError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

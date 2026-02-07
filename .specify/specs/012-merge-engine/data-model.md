# Data Model: Merge Engine

## Core Types

```rust
use git_hash::ObjectId;
use git_diff::DiffAlgorithm;

/// Options for merge operations.
#[derive(Debug, Clone)]
pub struct MergeOptions {
    pub strategy: MergeStrategyType,
    pub strategy_options: Vec<String>,
    pub diff_algorithm: DiffAlgorithm,
    pub rename_threshold: u8,
    pub conflict_style: ConflictStyle,
    pub allow_unrelated_histories: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum MergeStrategyType {
    Ort,
    Recursive,
    Ours,
    Subtree,
}

#[derive(Debug, Clone, Copy)]
pub enum ConflictStyle {
    Merge,    // Default: ours/theirs markers
    Diff3,    // Include base in markers
    ZDiff3,   // Zealous diff3 (reduce conflict size)
}

/// Result of a merge operation.
pub struct MergeResult {
    /// The resulting tree OID (if merge was clean)
    pub tree: Option<ObjectId>,
    /// Whether the merge was clean (no conflicts)
    pub is_clean: bool,
    /// List of conflicts (empty if clean)
    pub conflicts: Vec<ConflictEntry>,
    /// Commit message for the merge
    pub message: Option<String>,
}

/// A file-level conflict.
#[derive(Debug, Clone)]
pub struct ConflictEntry {
    pub path: BString,
    pub conflict_type: ConflictType,
    pub base: Option<ConflictSide>,
    pub ours: Option<ConflictSide>,
    pub theirs: Option<ConflictSide>,
}

#[derive(Debug, Clone, Copy)]
pub enum ConflictType {
    Content,          // Both modified same region
    ModifyDelete,     // One modified, other deleted
    AddAdd,           // Both added same path
    RenameRename,     // Both renamed differently
    RenameDelete,     // One renamed, other deleted
    DirectoryFile,    // One added dir, other added file
}

#[derive(Debug, Clone)]
pub struct ConflictSide {
    pub oid: ObjectId,
    pub mode: FileMode,
    pub path: BString,
}

/// Result of a three-way content merge.
pub enum ContentMergeResult {
    /// Clean merge, no conflicts
    Clean(Vec<u8>),
    /// Conflict with markers in the content
    Conflict {
        content: Vec<u8>,
        conflict_count: usize,
    },
}

/// Perform a three-way content merge.
pub fn merge_content(
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    options: &MergeOptions,
    labels: (&str, &str, &str),  // base, ours, theirs labels
) -> ContentMergeResult;

/// Trait for merge strategies.
pub trait MergeStrategy {
    fn merge(
        &self,
        repo: &Repository,
        ours: &ObjectId,    // Our commit
        theirs: &ObjectId,  // Their commit
        base: &ObjectId,    // Merge base
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError>;
}

/// Cherry-pick a commit onto the current branch.
pub fn cherry_pick(
    repo: &Repository,
    commit: &ObjectId,
    options: &MergeOptions,
) -> Result<MergeResult, MergeError>;

/// Revert a commit on the current branch.
pub fn revert(
    repo: &Repository,
    commit: &ObjectId,
    options: &MergeOptions,
) -> Result<MergeResult, MergeError>;

/// Multi-commit operation sequencer.
pub struct Sequencer {
    repo_path: PathBuf,
    original_head: ObjectId,
    todo: Vec<SequencerEntry>,
    current: usize,
    operation: SequencerOperation,
}

pub enum SequencerOperation {
    CherryPick,
    Revert,
    Rebase,
}

pub struct SequencerEntry {
    pub commit: ObjectId,
    pub action: SequencerAction,
}

pub enum SequencerAction {
    Pick,
    Revert,
    Edit,
    Squash,
    Fixup,
    Exec(String),
    Break,
}

impl Sequencer {
    pub fn new(repo: &Repository, operation: SequencerOperation) -> Self;
    pub fn add(&mut self, commit: ObjectId, action: SequencerAction);
    pub fn execute(&mut self, repo: &Repository) -> Result<SequencerResult, MergeError>;
    pub fn continue_operation(&mut self, repo: &Repository) -> Result<SequencerResult, MergeError>;
    pub fn abort(&self, repo: &Repository) -> Result<(), MergeError>;
    pub fn skip(&mut self, repo: &Repository) -> Result<SequencerResult, MergeError>;
    /// Save state to .git/sequencer/
    pub fn save(&self) -> Result<(), MergeError>;
    /// Load state from .git/sequencer/
    pub fn load(repo: &Repository) -> Result<Option<Self>, MergeError>;
}

pub enum SequencerResult {
    Complete,
    Paused { conflict: ConflictEntry },
}

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("merge conflict in {path}")]
    Conflict { path: BString },
    #[error("no merge base found")]
    NoMergeBase,
    #[error("cannot merge unrelated histories (use --allow-unrelated-histories)")]
    UnrelatedHistories,
    #[error("sequencer already in progress (use --continue, --abort, or --skip)")]
    SequencerInProgress,
    #[error(transparent)]
    Diff(#[from] git_diff::DiffError),
    #[error(transparent)]
    Repo(#[from] git_repository::RepoError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

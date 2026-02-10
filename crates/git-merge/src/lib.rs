//! Merge engine: three-way content merge, ORT tree merge, conflict handling,
//! cherry-pick, revert, and sequencer.
//!
//! Provides the core merge machinery used by `git merge`, `git cherry-pick`,
//! `git revert`, and `git apply`. Supports ORT (default), ours, and subtree
//! strategies with pluggable strategy options.

pub mod apply;
pub mod cherry_pick;
pub mod conflict;
pub mod content;
pub mod rerere;
pub mod revert;
pub mod sequencer;
pub mod strategy;

use bstr::BString;
use git_diff::DiffAlgorithm;
use git_hash::ObjectId;
use git_object::FileMode;

/// Options for merge operations.
#[derive(Debug, Clone)]
pub struct MergeOptions {
    /// Which merge strategy to use.
    pub strategy: MergeStrategyType,
    /// Strategy-specific options (e.g. "theirs", "patience").
    pub strategy_options: Vec<String>,
    /// Diff algorithm for content merge.
    pub diff_algorithm: DiffAlgorithm,
    /// Similarity threshold for rename detection (0-100, default 50).
    pub rename_threshold: u8,
    /// Conflict marker style.
    pub conflict_style: ConflictStyle,
    /// Allow merging unrelated histories.
    pub allow_unrelated_histories: bool,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            strategy: MergeStrategyType::Ort,
            strategy_options: Vec::new(),
            diff_algorithm: DiffAlgorithm::Myers,
            rename_threshold: 50,
            conflict_style: ConflictStyle::Merge,
            allow_unrelated_histories: false,
        }
    }
}

/// Available merge strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategyType {
    /// ORT strategy (default since git 2.34).
    Ort,
    /// Legacy recursive strategy.
    Recursive,
    /// Always take our side.
    Ours,
    /// Subtree merge.
    Subtree,
    /// Octopus merge (3+ branches).
    Octopus,
}

impl MergeStrategyType {
    /// Parse a strategy name string (as used by `git merge -s <strategy>`).
    ///
    /// Accepted values: "ort", "recursive", "ours", "subtree", "octopus".
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ort" => Some(Self::Ort),
            "recursive" => Some(Self::Recursive),
            "ours" => Some(Self::Ours),
            "subtree" => Some(Self::Subtree),
            "octopus" => Some(Self::Octopus),
            _ => None,
        }
    }

    /// Return the canonical name for this strategy.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ort => "ort",
            Self::Recursive => "recursive",
            Self::Ours => "ours",
            Self::Subtree => "subtree",
            Self::Octopus => "octopus",
        }
    }
}

/// Conflict marker style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStyle {
    /// Default: show ours and theirs only.
    Merge,
    /// Include base content between `|||||||` markers.
    Diff3,
    /// Zealous diff3: reduce conflict size by pulling out common prefix/suffix.
    ZDiff3,
}

impl ConflictStyle {
    /// Parse a conflict style name (as used by `merge.conflictStyle` config).
    ///
    /// Accepted values: "merge", "diff3", "zdiff3".
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "merge" => Some(Self::Merge),
            "diff3" => Some(Self::Diff3),
            "zdiff3" => Some(Self::ZDiff3),
            _ => None,
        }
    }

    /// Return the canonical config name for this style.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Diff3 => "diff3",
            Self::ZDiff3 => "zdiff3",
        }
    }
}

/// Result of a merge operation.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// The resulting tree OID (if merge was clean).
    pub tree: Option<ObjectId>,
    /// Whether the merge was clean (no conflicts).
    pub is_clean: bool,
    /// List of conflicts (empty if clean).
    pub conflicts: Vec<ConflictEntry>,
    /// Commit message for the merge.
    pub message: Option<String>,
}

impl MergeResult {
    /// Create a clean merge result.
    pub fn clean(tree: ObjectId) -> Self {
        Self {
            tree: Some(tree),
            is_clean: true,
            conflicts: Vec::new(),
            message: None,
        }
    }

    /// Create a conflicted merge result.
    pub fn conflicted(conflicts: Vec<ConflictEntry>) -> Self {
        Self {
            tree: None,
            is_clean: false,
            conflicts,
            message: None,
        }
    }
}

/// A file-level conflict.
#[derive(Debug, Clone)]
pub struct ConflictEntry {
    /// Path of the conflicted file.
    pub path: BString,
    /// Type of conflict.
    pub conflict_type: ConflictType,
    /// Base (common ancestor) side.
    pub base: Option<ConflictSide>,
    /// Our side (current branch).
    pub ours: Option<ConflictSide>,
    /// Their side (branch being merged).
    pub theirs: Option<ConflictSide>,
}

/// Types of merge conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictType {
    /// Both sides modified the same region.
    Content,
    /// One side modified, the other deleted.
    ModifyDelete,
    /// Both sides added the same path with different content.
    AddAdd,
    /// Both sides renamed the same file differently.
    RenameRename,
    /// One side renamed, the other deleted.
    RenameDelete,
    /// One side added a directory, the other a file at the same path.
    DirectoryFile,
}

/// One side of a conflict.
#[derive(Debug, Clone)]
pub struct ConflictSide {
    /// Object ID of this side's content.
    pub oid: ObjectId,
    /// File mode.
    pub mode: FileMode,
    /// Path (may differ from ConflictEntry path if renamed).
    pub path: BString,
}

/// Result of a three-way content merge.
#[derive(Debug, Clone)]
pub enum ContentMergeResult {
    /// Clean merge, no conflicts.
    Clean(Vec<u8>),
    /// Conflict with markers in the content.
    Conflict {
        /// Merged content including conflict markers.
        content: Vec<u8>,
        /// Number of conflict regions.
        conflict_count: usize,
    },
}

impl ContentMergeResult {
    /// Whether the merge was clean.
    pub fn is_clean(&self) -> bool {
        matches!(self, Self::Clean(_))
    }

    /// Get the merged content (with or without conflict markers).
    pub fn content(&self) -> &[u8] {
        match self {
            Self::Clean(data) => data,
            Self::Conflict { content, .. } => content,
        }
    }
}

/// Error types for merge operations.
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

    #[error("nothing to commit")]
    NothingToCommit,

    #[error("object not found: {0}")]
    ObjectNotFound(ObjectId),

    #[error("expected {expected} object, got {actual} for {oid}")]
    UnexpectedObjectType {
        oid: ObjectId,
        expected: &'static str,
        actual: String,
    },

    #[error("invalid patch: {0}")]
    InvalidPatch(String),

    #[error("patch does not apply: {0}")]
    PatchDoesNotApply(String),

    #[error(transparent)]
    Diff(#[from] git_diff::DiffError),

    #[error(transparent)]
    Odb(#[from] git_odb::OdbError),

    #[error(transparent)]
    Repo(#[from] git_repository::RepoError),

    #[error(transparent)]
    Index(#[from] git_index::IndexError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options() {
        let opts = MergeOptions::default();
        assert_eq!(opts.strategy, MergeStrategyType::Ort);
        assert_eq!(opts.diff_algorithm, DiffAlgorithm::Myers);
        assert_eq!(opts.rename_threshold, 50);
        assert_eq!(opts.conflict_style, ConflictStyle::Merge);
        assert!(!opts.allow_unrelated_histories);
        assert!(opts.strategy_options.is_empty());
    }

    #[test]
    fn clean_merge_result() {
        let oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let result = MergeResult::clean(oid);
        assert!(result.is_clean);
        assert!(result.conflicts.is_empty());
        assert_eq!(result.tree, Some(oid));
    }

    #[test]
    fn conflicted_merge_result() {
        let conflicts = vec![ConflictEntry {
            path: BString::from("file.txt"),
            conflict_type: ConflictType::Content,
            base: None,
            ours: None,
            theirs: None,
        }];
        let result = MergeResult::conflicted(conflicts);
        assert!(!result.is_clean);
        assert_eq!(result.conflicts.len(), 1);
        assert!(result.tree.is_none());
    }

    #[test]
    fn content_merge_result_clean() {
        let result = ContentMergeResult::Clean(b"hello world\n".to_vec());
        assert!(result.is_clean());
        assert_eq!(result.content(), b"hello world\n");
    }

    #[test]
    fn content_merge_result_conflict() {
        let result = ContentMergeResult::Conflict {
            content: b"<<<<<<< ours\nfoo\n=======\nbar\n>>>>>>> theirs\n".to_vec(),
            conflict_count: 1,
        };
        assert!(!result.is_clean());
        assert!(!result.content().is_empty());
    }

    #[test]
    fn conflict_types() {
        assert_eq!(ConflictType::Content, ConflictType::Content);
        assert_ne!(ConflictType::Content, ConflictType::AddAdd);
    }

    #[test]
    fn merge_strategy_types() {
        assert_eq!(MergeStrategyType::Ort, MergeStrategyType::Ort);
        assert_ne!(MergeStrategyType::Ort, MergeStrategyType::Ours);
    }
}

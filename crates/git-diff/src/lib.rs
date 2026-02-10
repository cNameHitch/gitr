//! Diff engine: line-level algorithms, tree diff, rename detection, and output formatting.
//!
//! Provides Myers, histogram, and patience diff algorithms, tree-to-tree diffing,
//! working tree comparison, rename/copy detection, the diffcore transformation
//! pipeline, and multiple output formats (unified, stat, raw, name-only).

pub mod algorithm;
pub mod binary;
pub mod color;
pub mod diffcore;
pub mod driver;
pub mod format;
pub mod pickaxe;
pub mod rename;
pub mod tree;
pub mod worktree;

use bstr::BString;
use git_hash::ObjectId;
use git_object::FileMode;

/// Options controlling diff behavior.
#[derive(Debug, Clone)]
pub struct DiffOptions {
    /// Which diff algorithm to use.
    pub algorithm: DiffAlgorithm,
    /// Number of context lines around each hunk (default 3).
    pub context_lines: u32,
    /// Enable rename detection.
    pub detect_renames: bool,
    /// Similarity threshold for rename detection (0-100, default 50).
    pub rename_threshold: u8,
    /// Enable copy detection.
    pub detect_copies: bool,
    /// Similarity threshold for copy detection (0-100, default 50).
    pub copy_threshold: u8,
    /// Enable color output.
    pub color: bool,
    /// Width for --stat output (None = auto-detect terminal width).
    pub stat_width: Option<usize>,
    /// Output format to produce.
    pub output_format: DiffOutputFormat,
    /// Pathspec filter (None = all paths).
    pub pathspec: Option<Vec<BString>>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            algorithm: DiffAlgorithm::Myers,
            context_lines: 3,
            detect_renames: false,
            rename_threshold: 50,
            detect_copies: false,
            copy_threshold: 50,
            color: false,
            stat_width: None,
            output_format: DiffOutputFormat::Unified,
            pathspec: None,
        }
    }
}

/// Available diff algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffAlgorithm {
    /// Myers O(ND) algorithm (default, produces minimal edit scripts).
    Myers,
    /// Histogram diff (variant of patience with histogram-based matching).
    Histogram,
    /// Patience diff (uses patience sorting on unique lines).
    Patience,
    /// Myers with minimal=true (always find the absolute minimum edit script).
    Minimal,
}

/// Diff output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOutputFormat {
    /// Standard unified diff with context.
    Unified,
    /// File change statistics (insertions/deletions per file).
    Stat,
    /// Short stat (summary line only).
    ShortStat,
    /// Numeric stat (machine-readable insertions/deletions).
    NumStat,
    /// Raw format with modes and OIDs.
    Raw,
    /// Only changed file paths.
    NameOnly,
    /// File paths with status letter (M/A/D/R/C/T).
    NameStatus,
    /// Summary of changes (new file, deleted file, rename).
    Summary,
    /// Word-level diff using `[-removed-]{+added+}` markers.
    WordDiff,
}

/// Result of diffing two trees or a working tree.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// Per-file diff entries.
    pub files: Vec<FileDiff>,
}

impl DiffResult {
    /// True if no files changed.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Number of files changed.
    pub fn num_files_changed(&self) -> usize {
        self.files.len()
    }

    /// Total number of lines inserted across all files.
    pub fn insertions(&self) -> usize {
        self.files.iter().map(|f| f.insertions()).sum()
    }

    /// Total number of lines deleted across all files.
    pub fn deletions(&self) -> usize {
        self.files.iter().map(|f| f.deletions()).sum()
    }
}

/// Diff for a single file.
#[derive(Debug, Clone)]
pub struct FileDiff {
    /// Type of change.
    pub status: FileStatus,
    /// Old path (None for added files).
    pub old_path: Option<BString>,
    /// New path (None for deleted files).
    pub new_path: Option<BString>,
    /// Old file mode (None for added files).
    pub old_mode: Option<FileMode>,
    /// New file mode (None for deleted files).
    pub new_mode: Option<FileMode>,
    /// Old object ID (None for added files).
    pub old_oid: Option<ObjectId>,
    /// New object ID (None for deleted files).
    pub new_oid: Option<ObjectId>,
    /// Diff hunks (empty for binary files or mode-only changes).
    pub hunks: Vec<Hunk>,
    /// Whether the file is binary.
    pub is_binary: bool,
    /// Similarity percentage for renames/copies (0-100).
    pub similarity: Option<u8>,
}

impl FileDiff {
    /// Number of lines inserted in this file.
    pub fn insertions(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| matches!(l, DiffLine::Addition(_)))
            .count()
    }

    /// Number of lines deleted in this file.
    pub fn deletions(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| matches!(l, DiffLine::Deletion(_)))
            .count()
    }

    /// The effective path for display (prefers new_path, falls back to old_path).
    pub fn path(&self) -> &BString {
        self.new_path
            .as_ref()
            .or(self.old_path.as_ref())
            .expect("FileDiff must have at least one path")
    }
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
    /// Single-character status code matching C git output.
    pub fn as_char(&self) -> char {
        match self {
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Modified => 'M',
            Self::Renamed => 'R',
            Self::Copied => 'C',
            Self::TypeChanged => 'T',
            Self::Unmerged => 'U',
        }
    }
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Modified => "M",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::TypeChanged => "T",
            Self::Unmerged => "U",
        })
    }
}

/// A contiguous region of changes.
#[derive(Debug, Clone)]
pub struct Hunk {
    /// Start line in the old file (1-based).
    pub old_start: u32,
    /// Number of lines from the old file.
    pub old_count: u32,
    /// Start line in the new file (1-based).
    pub new_start: u32,
    /// Number of lines from the new file.
    pub new_count: u32,
    /// Optional function/section header (from hunk context).
    pub header: Option<BString>,
    /// Lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff hunk.
#[derive(Debug, Clone)]
pub enum DiffLine {
    /// Unchanged context line.
    Context(BString),
    /// Added line.
    Addition(BString),
    /// Deleted line.
    Deletion(BString),
}

/// Error types for diff operations.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("failed to read object {oid}: {source}")]
    ObjectRead {
        oid: ObjectId,
        #[source]
        source: git_odb::OdbError,
    },

    #[error("object not found: {0}")]
    ObjectNotFound(ObjectId),

    #[error("expected {expected} object, got {actual} for {oid}")]
    UnexpectedObjectType {
        oid: ObjectId,
        expected: &'static str,
        actual: String,
    },

    #[error("binary file: {0}")]
    BinaryFile(BString),

    #[error(transparent)]
    Repo(#[from] git_repository::RepoError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options() {
        let opts = DiffOptions::default();
        assert_eq!(opts.algorithm, DiffAlgorithm::Myers);
        assert_eq!(opts.context_lines, 3);
        assert!(!opts.detect_renames);
        assert_eq!(opts.rename_threshold, 50);
        assert!(!opts.color);
        assert_eq!(opts.output_format, DiffOutputFormat::Unified);
    }

    #[test]
    fn file_status_char() {
        assert_eq!(FileStatus::Added.as_char(), 'A');
        assert_eq!(FileStatus::Deleted.as_char(), 'D');
        assert_eq!(FileStatus::Modified.as_char(), 'M');
        assert_eq!(FileStatus::Renamed.as_char(), 'R');
        assert_eq!(FileStatus::Copied.as_char(), 'C');
        assert_eq!(FileStatus::TypeChanged.as_char(), 'T');
        assert_eq!(FileStatus::Unmerged.as_char(), 'U');
    }

    #[test]
    fn file_status_display() {
        assert_eq!(FileStatus::Added.to_string(), "A");
        assert_eq!(FileStatus::Modified.to_string(), "M");
    }

    #[test]
    fn empty_diff_result() {
        let result = DiffResult { files: vec![] };
        assert!(result.is_empty());
        assert_eq!(result.num_files_changed(), 0);
        assert_eq!(result.insertions(), 0);
        assert_eq!(result.deletions(), 0);
    }

    #[test]
    fn diff_result_counts() {
        let result = DiffResult {
            files: vec![FileDiff {
                status: FileStatus::Modified,
                old_path: Some(BString::from("file.txt")),
                new_path: Some(BString::from("file.txt")),
                old_mode: Some(FileMode::Regular),
                new_mode: Some(FileMode::Regular),
                old_oid: None,
                new_oid: None,
                hunks: vec![Hunk {
                    old_start: 1,
                    old_count: 3,
                    new_start: 1,
                    new_count: 4,
                    header: None,
                    lines: vec![
                        DiffLine::Context(BString::from("a")),
                        DiffLine::Deletion(BString::from("b")),
                        DiffLine::Addition(BString::from("c")),
                        DiffLine::Addition(BString::from("d")),
                        DiffLine::Context(BString::from("e")),
                    ],
                }],
                is_binary: false,
                similarity: None,
            }],
        };
        assert_eq!(result.num_files_changed(), 1);
        assert_eq!(result.insertions(), 2);
        assert_eq!(result.deletions(), 1);
    }
}

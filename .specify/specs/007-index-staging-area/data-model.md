# Data Model: Index / Staging Area

## Core Types

```rust
use git_hash::ObjectId;
use git_object::FileMode;
use bstr::{BStr, BString};

/// The git index (staging area).
pub struct Index {
    /// Index format version (2, 3, or 4)
    version: u32,
    /// Cache entries sorted by path
    entries: Vec<IndexEntry>,
    /// Cache tree extension
    cache_tree: Option<CacheTree>,
    /// Resolve-undo extension
    resolve_undo: Option<ResolveUndo>,
    /// Unknown extensions (preserved for round-trip)
    unknown_extensions: Vec<RawExtension>,
    /// Checksum of the index file
    checksum: ObjectId,
}

impl Index {
    /// Read the index from a file
    pub fn read_from(path: impl AsRef<Path>) -> Result<Self, IndexError>;
    /// Write the index to a file (atomic, using lock file)
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), IndexError>;

    /// Number of entries
    pub fn len(&self) -> usize;
    /// Is the index empty?
    pub fn is_empty(&self) -> bool;

    /// Get an entry by path and stage
    pub fn get(&self, path: &BStr, stage: Stage) -> Option<&IndexEntry>;
    /// Get all entries for a path (all stages)
    pub fn get_all(&self, path: &BStr) -> Vec<&IndexEntry>;

    /// Add or update an entry
    pub fn add(&mut self, entry: IndexEntry);
    /// Remove entries matching a path
    pub fn remove(&mut self, path: &BStr, stage: Stage) -> bool;

    /// Check if the path has conflicts (stages 1, 2, or 3)
    pub fn has_conflicts(&self, path: &BStr) -> bool;
    /// Get all conflicted paths
    pub fn conflicts(&self) -> Vec<&BStr>;

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = &IndexEntry>;
    /// Iterate over entries matching a pathspec
    pub fn iter_matching(&self, pathspec: &Pathspec) -> impl Iterator<Item = &IndexEntry>;

    /// Get the cache tree (if available)
    pub fn cache_tree(&self) -> Option<&CacheTree>;
    /// Invalidate cache tree for a path
    pub fn invalidate_cache_tree(&mut self, path: &BStr);

    /// Create a tree hierarchy from the current index state
    pub fn write_tree(&self, odb: &ObjectDatabase) -> Result<ObjectId, IndexError>;
}

/// A single entry in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// File path (relative to repo root)
    pub path: BString,
    /// Object ID of the blob
    pub oid: ObjectId,
    /// File mode
    pub mode: FileMode,
    /// Merge stage (0 = normal, 1 = base, 2 = ours, 3 = theirs)
    pub stage: Stage,
    /// Stat data from the file system
    pub stat: StatData,
    /// Entry flags
    pub flags: EntryFlags,
}

/// Merge stage for index entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Normal entry (stage 0)
    Normal,
    /// Base version in merge conflict (stage 1)
    Base,
    /// Ours version in merge conflict (stage 2)
    Ours,
    /// Theirs version in merge conflict (stage 3)
    Theirs,
}

impl Stage {
    pub fn as_u8(&self) -> u8;
    pub fn from_u8(n: u8) -> Result<Self, IndexError>;
}

/// File system stat data cached in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatData {
    pub ctime_secs: u32,
    pub ctime_nsecs: u32,
    pub mtime_secs: u32,
    pub mtime_nsecs: u32,
    pub dev: u32,
    pub ino: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u32,
}

impl StatData {
    /// Create from file system metadata
    pub fn from_metadata(meta: &std::fs::Metadata) -> Self;
    /// Check if stat data matches file metadata (for change detection)
    pub fn matches(&self, meta: &std::fs::Metadata) -> bool;
}

/// Entry flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryFlags {
    pub assume_valid: bool,
    pub intent_to_add: bool,
    pub skip_worktree: bool,
}

/// Cache tree extension — cached tree OIDs for fast commit.
pub struct CacheTree {
    /// Root tree node
    root: CacheTreeNode,
}

pub struct CacheTreeNode {
    /// Number of entries covered by this tree (-1 = invalid)
    pub entry_count: i32,
    /// Number of subtrees
    pub subtree_count: usize,
    /// Tree OID (valid only if entry_count >= 0)
    pub oid: Option<ObjectId>,
    /// Subtrees
    pub children: Vec<(BString, CacheTreeNode)>,
}

impl CacheTree {
    pub fn parse(data: &[u8]) -> Result<Self, IndexError>;
    pub fn serialize(&self) -> Vec<u8>;
    /// Invalidate the entry for the given path and all ancestors
    pub fn invalidate(&mut self, path: &BStr);
    /// Get the tree OID for a path prefix
    pub fn get_oid(&self, path: &BStr) -> Option<&ObjectId>;
}

/// Resolve-undo extension (REUC).
pub struct ResolveUndo {
    pub entries: Vec<ResolveUndoEntry>,
}

pub struct ResolveUndoEntry {
    pub path: BString,
    pub modes: [Option<FileMode>; 3],  // base, ours, theirs
    pub oids: [Option<ObjectId>; 3],
}

/// Raw unknown extension (preserved for round-trip).
pub struct RawExtension {
    pub signature: [u8; 4],
    pub data: Vec<u8>,
}

/// Gitignore pattern matching.
pub struct IgnoreStack {
    patterns: Vec<IgnorePattern>,
}

pub struct IgnorePattern {
    pub pattern: WildmatchPattern,
    pub negated: bool,
    pub directory_only: bool,
    pub anchored: bool,
    pub source: PathBuf,
}

impl IgnoreStack {
    /// Load from .gitignore, info/exclude, and core.excludesFile
    pub fn load(repo_root: &Path, config: &ConfigSet) -> Result<Self, IndexError>;
    /// Check if a path is ignored
    pub fn is_ignored(&self, path: &BStr, is_dir: bool) -> bool;
}

/// Pathspec for file filtering.
pub struct Pathspec {
    patterns: Vec<PathspecPattern>,
}

pub struct PathspecPattern {
    pub raw: BString,
    pub pattern: BString,
    pub magic: PathspecMagic,
}

#[derive(Debug, Default)]
pub struct PathspecMagic {
    pub top: bool,      // :(top) — relative to repo root
    pub exclude: bool,  // :(exclude) or :!
    pub icase: bool,    // :(icase) — case-insensitive
    pub glob: bool,     // :(glob) — glob matching
    pub literal: bool,  // :(literal) — no wildcards
}

impl Pathspec {
    pub fn parse(patterns: &[&str]) -> Result<Self, IndexError>;
    pub fn matches(&self, path: &BStr, is_dir: bool) -> bool;
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("invalid index header: {0}")]
    InvalidHeader(String),

    #[error("unsupported index version: {0}")]
    UnsupportedVersion(u32),

    #[error("index checksum mismatch")]
    ChecksumMismatch,

    #[error("invalid index entry at offset {offset}: {reason}")]
    InvalidEntry { offset: usize, reason: String },

    #[error("invalid extension '{sig}': {reason}")]
    InvalidExtension { sig: String, reason: String },

    #[error("invalid pathspec: {0}")]
    InvalidPathspec(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Lock(#[from] git_utils::LockError),

    #[error(transparent)]
    Odb(#[from] git_odb::OdbError),
}
```

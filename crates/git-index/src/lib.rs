//! Index (staging area) for git.
//!
//! Provides reading, writing, and manipulation of the git index file (`.git/index`).
//! The index sits between the working tree and the object database, tracking which
//! files are staged for the next commit.

pub mod attributes;
pub mod entry;
pub mod extensions;
pub mod ignore;
pub mod pathspec;
mod read;
mod write;

use std::path::Path;

use bstr::BStr;
use git_hash::ObjectId;
use git_odb::ObjectDatabase;

pub use entry::{EntryFlags, IndexEntry, StatData};
pub use error::IndexError;
pub use extensions::tree::CacheTree;
pub use extensions::{RawExtension, ResolveUndo};
pub use ignore::IgnoreStack;
pub use pathspec::Pathspec;

mod error {
    use std::path::PathBuf;

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

        #[error("invalid ignore pattern: {0}")]
        InvalidIgnorePattern(String),

        #[error("invalid attribute: {0}")]
        InvalidAttribute(String),

        #[error("lock failed: {path}")]
        LockFailed { path: PathBuf },

        #[error(transparent)]
        Io(#[from] std::io::Error),

        #[error(transparent)]
        Odb(#[from] git_odb::OdbError),
    }
}

/// Merge stage for index entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Normal entry (stage 0).
    Normal,
    /// Base version in merge conflict (stage 1).
    Base,
    /// Ours version in merge conflict (stage 2).
    Ours,
    /// Theirs version in merge conflict (stage 3).
    Theirs,
}

impl Stage {
    pub fn as_u8(&self) -> u8 {
        match self {
            Stage::Normal => 0,
            Stage::Base => 1,
            Stage::Ours => 2,
            Stage::Theirs => 3,
        }
    }

    pub fn from_u8(n: u8) -> Result<Self, IndexError> {
        match n {
            0 => Ok(Stage::Normal),
            1 => Ok(Stage::Base),
            2 => Ok(Stage::Ours),
            3 => Ok(Stage::Theirs),
            _ => Err(IndexError::InvalidEntry {
                offset: 0,
                reason: format!("invalid stage: {n}"),
            }),
        }
    }
}

/// The git index (staging area).
pub struct Index {
    /// Index format version (2, 3, or 4).
    version: u32,
    /// Cache entries sorted by (path, stage).
    entries: Vec<IndexEntry>,
    /// Cache tree extension.
    cache_tree: Option<CacheTree>,
    /// Resolve-undo extension.
    resolve_undo: Option<ResolveUndo>,
    /// Unknown extensions (preserved for round-trip).
    unknown_extensions: Vec<RawExtension>,
    /// Checksum of the index file.
    _checksum: ObjectId,
}

impl Index {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self {
            version: 2,
            entries: Vec::new(),
            cache_tree: None,
            resolve_undo: None,
            unknown_extensions: Vec::new(),
            _checksum: ObjectId::NULL_SHA1,
        }
    }

    /// Read the index from a file (memory-mapped for large indices).
    pub fn read_from(path: impl AsRef<Path>) -> Result<Self, IndexError> {
        let file = std::fs::File::open(path.as_ref())?;
        let data = unsafe { memmap2::Mmap::map(&file) }?;
        read::parse_index(&data)
    }

    /// Write the index to a file (atomic, using lock file).
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), IndexError> {
        write::write_index(self, path.as_ref())
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is the index empty?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by path and stage.
    pub fn get(&self, path: &BStr, stage: Stage) -> Option<&IndexEntry> {
        self.entries
            .iter()
            .find(|e| e.path[..] == path[..] && e.stage == stage)
    }

    /// Get all entries for a path (all stages).
    pub fn get_all(&self, path: &BStr) -> Vec<&IndexEntry> {
        self.entries
            .iter()
            .filter(|e| e.path[..] == path[..])
            .collect()
    }

    /// Add or update an entry. Maintains sorted order.
    pub fn add(&mut self, entry: IndexEntry) {
        // Remove existing entry with same path and stage
        self.entries
            .retain(|e| !(e.path == entry.path && e.stage == entry.stage));

        // Invalidate cache tree for this path
        if let Some(ref mut tree) = self.cache_tree {
            tree.invalidate(BStr::new(&entry.path));
        }

        // Insert in sorted position
        let pos = self
            .entries
            .binary_search_by(|e| cmp_entries(e, &entry))
            .unwrap_or_else(|pos| pos);
        self.entries.insert(pos, entry);
    }

    /// Remove entries matching a path and stage. Returns true if any were removed.
    pub fn remove(&mut self, path: &BStr, stage: Stage) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !(e.path[..] == path[..] && e.stage == stage));
        let removed = self.entries.len() < before;

        if removed {
            if let Some(ref mut tree) = self.cache_tree {
                tree.invalidate(path);
            }
        }

        removed
    }

    /// Check if the path has conflicts (stages 1, 2, or 3).
    pub fn has_conflicts(&self, path: &BStr) -> bool {
        self.entries
            .iter()
            .any(|e| e.path[..] == path[..] && e.stage != Stage::Normal)
    }

    /// Get all conflicted paths.
    pub fn conflicts(&self) -> Vec<&BStr> {
        let mut paths: Vec<&BStr> = self
            .entries
            .iter()
            .filter(|e| e.stage != Stage::Normal)
            .map(|e| e.path.as_ref())
            .collect();
        paths.dedup();
        paths
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &IndexEntry> {
        self.entries.iter()
    }

    /// Iterate over entries matching a pathspec.
    pub fn iter_matching<'a>(
        &'a self,
        pathspec: &'a Pathspec,
    ) -> impl Iterator<Item = &'a IndexEntry> {
        self.entries
            .iter()
            .filter(move |e| pathspec.matches(BStr::new(&e.path), false))
    }

    /// Get the index version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Get the cache tree (if available).
    pub fn cache_tree(&self) -> Option<&CacheTree> {
        self.cache_tree.as_ref()
    }

    /// Get the cache tree mutably.
    pub fn cache_tree_mut(&mut self) -> Option<&mut CacheTree> {
        self.cache_tree.as_mut()
    }

    /// Set the cache tree.
    pub fn set_cache_tree(&mut self, tree: Option<CacheTree>) {
        self.cache_tree = tree;
    }

    /// Get the resolve-undo extension.
    pub fn resolve_undo(&self) -> Option<&ResolveUndo> {
        self.resolve_undo.as_ref()
    }

    /// Create a tree hierarchy from the current index state.
    pub fn write_tree(&self, odb: &ObjectDatabase) -> Result<ObjectId, IndexError> {
        write::write_tree_from_index(self, odb)
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

/// Compare two index entries for sort order: by path, then by stage.
fn cmp_entries(a: &IndexEntry, b: &IndexEntry) -> std::cmp::Ordering {
    a.path.cmp(&b.path).then(a.stage.as_u8().cmp(&b.stage.as_u8()))
}


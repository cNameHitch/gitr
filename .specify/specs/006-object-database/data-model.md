# Data Model: Object Database

## Core Types

```rust
use git_hash::ObjectId;
use git_object::{Object, ObjectType, ObjectCache};
use git_loose::LooseObjectStore;
use git_pack::PackFile;

/// Trait for pluggable object storage backends.
pub trait OdbBackend: Send + Sync {
    /// Read an object by OID
    fn read(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError>;

    /// Read just the header (type + size)
    fn read_header(&self, oid: &ObjectId) -> Result<Option<(ObjectType, usize)>, OdbError>;

    /// Check if an object exists
    fn contains(&self, oid: &ObjectId) -> bool;

    /// Write an object, returning its OID
    fn write(&self, obj: &Object) -> Result<ObjectId, OdbError>;

    /// Find all OIDs matching the given prefix
    fn lookup_prefix(&self, prefix: &str) -> Result<Vec<ObjectId>, OdbError>;
}

/// Unified object database providing access across all storage backends.
pub struct ObjectDatabase {
    /// Loose object store
    loose: LooseObjectStore,
    /// Pack files (protected by RwLock for refresh)
    packs: std::sync::RwLock<Vec<PackFile>>,
    /// Alternate object databases
    alternates: Vec<ObjectDatabase>,
    /// Object cache
    cache: std::sync::Mutex<ObjectCache>,
    /// Path to the objects directory
    objects_dir: std::path::PathBuf,
}

impl ObjectDatabase {
    /// Open the object database at the given objects directory
    pub fn open(objects_dir: impl AsRef<std::path::Path>) -> Result<Self, OdbError>;

    /// Read an object by OID (searches loose → packs → alternates)
    pub fn read(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError>;

    /// Read with caching
    pub fn read_cached(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError>;

    /// Read just the header
    pub fn read_header(&self, oid: &ObjectId) -> Result<Option<(ObjectType, usize)>, OdbError>;

    /// Check if an object exists (fast, no decompression)
    pub fn contains(&self, oid: &ObjectId) -> bool;

    /// Write a new object (always to loose store)
    pub fn write(&self, obj: &Object) -> Result<ObjectId, OdbError>;

    /// Write raw content with type
    pub fn write_raw(&self, obj_type: ObjectType, content: &[u8]) -> Result<ObjectId, OdbError>;

    /// Resolve an OID prefix to a full OID
    /// Returns error if prefix is ambiguous
    pub fn resolve_prefix(&self, prefix: &str) -> Result<ObjectId, OdbError>;

    /// Refresh the list of pack files (call after gc/repack)
    pub fn refresh(&self) -> Result<(), OdbError>;

    /// Iterate over all known object OIDs (for fsck/gc)
    pub fn iter_all_oids(&self) -> Result<Box<dyn Iterator<Item = Result<ObjectId, OdbError>>>, OdbError>;
}

/// Lightweight object info (header only, no content)
pub struct ObjectInfo {
    pub obj_type: ObjectType,
    pub size: usize,
}

/// Error types for ODB operations
#[derive(Debug, thiserror::Error)]
pub enum OdbError {
    #[error("object not found: {0}")]
    NotFound(ObjectId),

    #[error("ambiguous object name: {prefix} matches {count} objects")]
    Ambiguous { prefix: String, count: usize },

    #[error("corrupt object {oid}: {reason}")]
    Corrupt { oid: ObjectId, reason: String },

    #[error("alternates error: {0}")]
    Alternates(String),

    #[error("circular alternates chain detected at {0}")]
    CircularAlternates(std::path::PathBuf),

    #[error(transparent)]
    Loose(#[from] git_loose::LooseError),

    #[error(transparent)]
    Pack(#[from] git_pack::PackError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

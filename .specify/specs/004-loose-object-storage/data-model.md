# Data Model: Loose Object Storage

## Core Types

```rust
use git_hash::{ObjectId, HashAlgorithm, Hasher};
use git_object::{Object, ObjectType};
use std::path::{Path, PathBuf};

/// Interface to the loose object directory (.git/objects/).
pub struct LooseObjectStore {
    /// Path to the objects directory
    objects_dir: PathBuf,
    /// Hash algorithm in use
    hash_algo: HashAlgorithm,
    /// Compression level (0-9, or -1 for default)
    compression_level: i32,
}

impl LooseObjectStore {
    /// Open the loose object store at the given path
    pub fn open(objects_dir: impl AsRef<Path>, hash_algo: HashAlgorithm) -> Result<Self, LooseError>;

    /// Set the compression level
    pub fn set_compression_level(&mut self, level: i32);

    // --- Read operations ---

    /// Read a loose object by OID
    pub fn read(&self, oid: &ObjectId) -> Result<Option<Object>, LooseError>;

    /// Read just the header (type + size) without full decompression
    pub fn read_header(&self, oid: &ObjectId) -> Result<Option<(ObjectType, usize)>, LooseError>;

    /// Open a streaming reader for a loose object
    pub fn stream(&self, oid: &ObjectId) -> Result<Option<LooseObjectStream>, LooseError>;

    /// Check if a loose object exists
    pub fn contains(&self, oid: &ObjectId) -> bool;

    // --- Write operations ---

    /// Write an object to the loose store. Returns the OID.
    /// No-op if the object already exists.
    pub fn write(&self, obj: &Object) -> Result<ObjectId, LooseError>;

    /// Write raw bytes with a known type. Returns the OID.
    pub fn write_raw(&self, obj_type: ObjectType, content: &[u8]) -> Result<ObjectId, LooseError>;

    /// Write from a stream with known type and size. Returns the OID.
    pub fn write_stream(
        &self,
        obj_type: ObjectType,
        size: usize,
        reader: &mut dyn std::io::Read,
    ) -> Result<ObjectId, LooseError>;

    // --- Enumeration ---

    /// Iterate over all loose object OIDs
    pub fn iter(&self) -> Result<LooseObjectIter, LooseError>;

    // --- Path helpers ---

    /// Get the file path for a given OID
    pub fn object_path(&self, oid: &ObjectId) -> PathBuf;
}

/// Streaming reader for a loose object.
/// Decompresses data on demand as `Read` is called.
pub struct LooseObjectStream {
    obj_type: ObjectType,
    size: usize,
    decoder: flate2::read::ZlibDecoder<std::fs::File>,
    bytes_read: usize,
}

impl LooseObjectStream {
    pub fn object_type(&self) -> ObjectType;
    pub fn size(&self) -> usize;
    pub fn bytes_remaining(&self) -> usize;
}

impl std::io::Read for LooseObjectStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
}

/// Iterator over loose object OIDs.
pub struct LooseObjectIter {
    dir_iter: Box<dyn Iterator<Item = std::io::Result<std::fs::DirEntry>>>,
    current_subdir: Option<Box<dyn Iterator<Item = std::io::Result<std::fs::DirEntry>>>>,
    current_prefix: String,
}

impl Iterator for LooseObjectIter {
    type Item = Result<ObjectId, LooseError>;
}

/// Errors from loose object operations
#[derive(Debug, thiserror::Error)]
pub enum LooseError {
    #[error("corrupt loose object {oid}: {reason}")]
    Corrupt { oid: String, reason: String },

    #[error("decompression error for {oid}: {source}")]
    Decompress {
        oid: String,
        #[source]
        source: std::io::Error,
    },

    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: ObjectId,
        actual: ObjectId,
    },

    #[error("object not found: {0}")]
    NotFound(ObjectId),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Object(#[from] git_object::ObjectError),
}
```

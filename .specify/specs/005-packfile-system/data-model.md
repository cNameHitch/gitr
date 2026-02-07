# Data Model: Packfile System

## Core Types

### Pack File

```rust
use git_hash::ObjectId;
use git_object::ObjectType;
use memmap2::Mmap;

/// A memory-mapped packfile with its index.
pub struct PackFile {
    /// Memory-mapped pack data
    data: Mmap,
    /// The pack index for OID → offset lookup
    index: PackIndex,
    /// Path to the .pack file
    pack_path: PathBuf,
    /// Pack checksum (from trailer)
    checksum: ObjectId,
}

impl PackFile {
    /// Open a pack file and its index
    pub fn open(pack_path: impl AsRef<Path>) -> Result<Self, PackError>;

    /// Read an object by OID
    pub fn read_object(&self, oid: &ObjectId) -> Result<Option<PackedObject>, PackError>;

    /// Read an object at a known offset
    pub fn read_at_offset(&self, offset: u64) -> Result<PackedObject, PackError>;

    /// Check if this pack contains the given OID
    pub fn contains(&self, oid: &ObjectId) -> bool;

    /// Get the number of objects in this pack
    pub fn num_objects(&self) -> u32;

    /// Iterate over all objects in the pack
    pub fn iter(&self) -> PackIter<'_>;

    /// Verify the pack checksum
    pub fn verify_checksum(&self) -> Result<(), PackError>;
}
```

### Pack Index

```rust
/// Pack index (v2) providing OID → offset mapping.
pub struct PackIndex {
    /// Memory-mapped index data
    data: Mmap,
    /// Version (currently always 2)
    version: u32,
    /// Number of objects
    num_objects: u32,
    /// Fan-out table (256 entries)
    fanout: [u32; 256],
}

impl PackIndex {
    /// Open a pack index file
    pub fn open(idx_path: impl AsRef<Path>) -> Result<Self, PackError>;

    /// Look up an OID, returning the offset in the pack file
    pub fn lookup(&self, oid: &ObjectId) -> Option<u64>;

    /// Look up by OID prefix, returning all matches
    pub fn lookup_prefix(&self, prefix: &[u8]) -> Vec<(ObjectId, u64)>;

    /// Get the OID at the given sorted index position
    pub fn oid_at_index(&self, index: u32) -> ObjectId;

    /// Get the offset at the given sorted index position
    pub fn offset_at_index(&self, index: u32) -> u64;

    /// Get the CRC32 at the given sorted index position
    pub fn crc32_at_index(&self, index: u32) -> u32;

    /// Total number of objects
    pub fn num_objects(&self) -> u32;

    /// Iterate over all (OID, offset) pairs in sorted order
    pub fn iter(&self) -> PackIndexIter<'_>;

    /// Pack checksum stored in the index trailer
    pub fn pack_checksum(&self) -> ObjectId;
}
```

### Pack Entry

```rust
/// Type of packed object entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackEntryType {
    Commit,
    Tree,
    Blob,
    Tag,
    /// Delta with offset to base in same pack
    OfsDelta { base_offset: u64 },
    /// Delta referencing base by OID
    RefDelta { base_oid: ObjectId },
}

/// A raw entry read from a packfile (before delta resolution)
pub struct PackEntry {
    pub entry_type: PackEntryType,
    pub uncompressed_size: usize,
    pub data_offset: u64,  // offset to the compressed data
    pub header_size: usize,
}

/// A fully resolved object from a packfile
pub struct PackedObject {
    pub obj_type: ObjectType,
    pub data: Vec<u8>,
}
```

### Delta Operations

```rust
/// A single delta instruction
#[derive(Debug, Clone)]
pub enum DeltaInstruction {
    /// Copy bytes from the base object
    Copy {
        offset: u64,
        size: usize,
    },
    /// Insert literal bytes
    Insert(Vec<u8>),
}

/// Apply a delta instruction stream to a base object
pub fn apply_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>, PackError>;

/// Parse delta instructions from raw bytes
pub fn parse_delta_instructions(delta: &[u8]) -> Result<(usize, usize, Vec<DeltaInstruction>), PackError>;

/// Compute a delta between two objects
pub fn compute_delta(source: &[u8], target: &[u8]) -> Vec<u8>;
```

### Multi-Pack Index

```rust
/// Multi-pack index spanning multiple packfiles
pub struct MultiPackIndex {
    data: Mmap,
    num_packs: u32,
    num_objects: u32,
    pack_names: Vec<String>,
}

impl MultiPackIndex {
    pub fn open(midx_path: impl AsRef<Path>) -> Result<Self, PackError>;
    pub fn lookup(&self, oid: &ObjectId) -> Option<(u32, u64)>; // (pack_index, offset)
    pub fn num_objects(&self) -> u32;
    pub fn pack_names(&self) -> &[String];
}
```

### Bitmap Index

```rust
/// Bitmap index for fast reachability queries
pub struct BitmapIndex {
    data: Mmap,
    num_commits: u32,
}

impl BitmapIndex {
    pub fn open(bitmap_path: impl AsRef<Path>) -> Result<Self, PackError>;
    pub fn reachable_from(&self, commit_oid: &ObjectId) -> Result<OidSet, PackError>;
    pub fn has_bitmap_for(&self, commit_oid: &ObjectId) -> bool;
}
```

### Reverse Index

```rust
/// Reverse index: offset → OID mapping
pub struct ReverseIndex {
    // Sorted array of (offset, index_position) pairs
    entries: Vec<(u64, u32)>,
}

impl ReverseIndex {
    /// Build from a pack index
    pub fn build(index: &PackIndex) -> Self;
    /// Load from .rev file
    pub fn open(rev_path: impl AsRef<Path>) -> Result<Self, PackError>;
    /// Look up OID by offset
    pub fn lookup_offset(&self, offset: u64, index: &PackIndex) -> Option<ObjectId>;
}
```

### Pack Writer

```rust
/// Builder for creating new packfiles
pub struct PackWriter {
    file: std::fs::File,
    hasher: Hasher,
    num_objects: u32,
    entries: Vec<PackWriterEntry>,
    /// When true, allow delta bases that reference objects not in this pack
    thin: bool,
}

impl PackWriter {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, PackError>;

    /// Enable thin pack mode. Delta bases may reference objects not included
    /// in the pack. The receiver is expected to already have those objects.
    /// Used by push (remote has the bases) and fetch (client has the bases).
    pub fn set_thin(&mut self, thin: bool);

    pub fn add_object(&mut self, obj_type: ObjectType, data: &[u8]) -> Result<(), PackError>;
    pub fn add_delta(&mut self, base_oid: ObjectId, delta: &[u8]) -> Result<(), PackError>;
    pub fn finish(self) -> Result<(PathBuf, ObjectId), PackError>; // Returns (pack_path, checksum)
}

/// Build a pack index from a written pack
pub fn build_pack_index(pack_path: &Path) -> Result<PathBuf, PackError>;

/// Generate a pack containing objects reachable from `wants` but not from `haves`.
/// If `thin` is true, deltas may reference base objects in `haves` without
/// including them in the pack. This is the core routine used by push and fetch.
pub fn generate_pack(
    odb: &ObjectDatabase,
    wants: &[ObjectId],
    haves: &[ObjectId],
    thin: bool,
    output: impl std::io::Write,
) -> Result<PackGenerationResult, PackError>;

pub struct PackGenerationResult {
    pub num_objects: u32,
    pub bytes_written: u64,
    pub checksum: ObjectId,
}
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("invalid pack header: {0}")]
    InvalidHeader(String),

    #[error("invalid pack index: {0}")]
    InvalidIndex(String),

    #[error("invalid delta at offset {offset}: {reason}")]
    InvalidDelta { offset: u64, reason: String },

    #[error("delta base not found: {0}")]
    MissingBase(ObjectId),

    #[error("delta chain too deep (>{max_depth} levels) at offset {offset}")]
    DeltaChainTooDeep { offset: u64, max_depth: usize },

    #[error("pack checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: ObjectId, actual: ObjectId },

    #[error("corrupt pack entry at offset {0}")]
    CorruptEntry(u64),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Object(#[from] git_object::ObjectError),
}
```

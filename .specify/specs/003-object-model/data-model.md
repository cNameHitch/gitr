# Data Model: Object Model

## Core Types

### Object Type Enum

```rust
/// The four types of git objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl ObjectType {
    /// Parse from the type string in object headers
    pub fn from_bytes(s: &[u8]) -> Result<Self, ObjectError> {
        match s {
            b"blob" => Ok(Self::Blob),
            b"tree" => Ok(Self::Tree),
            b"commit" => Ok(Self::Commit),
            b"tag" => Ok(Self::Tag),
            _ => Err(ObjectError::InvalidType(BString::from(s))),
        }
    }

    /// The canonical byte representation
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Blob => b"blob",
            Self::Tree => b"tree",
            Self::Commit => b"commit",
            Self::Tag => b"tag",
        }
    }
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(std::str::from_utf8(self.as_bytes()).unwrap())
    }
}
```

### Object Enum

```rust
/// A parsed git object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
    Tag(Tag),
}

impl Object {
    /// Parse from raw bytes (header + content)
    pub fn parse(data: &[u8]) -> Result<Self, ObjectError>;

    /// Parse from content bytes with known type (no header)
    pub fn parse_content(obj_type: ObjectType, content: &[u8]) -> Result<Self, ObjectError>;

    /// Serialize to canonical git format (header + content)
    pub fn serialize(&self) -> Vec<u8>;

    /// Serialize just the content (no header)
    pub fn serialize_content(&self) -> Vec<u8>;

    /// Get the object type
    pub fn object_type(&self) -> ObjectType;

    /// Compute the OID by hashing the serialized form
    pub fn compute_oid(&self, algo: HashAlgorithm) -> ObjectId;

    /// Get the size of the content (excluding header)
    pub fn content_size(&self) -> usize;
}
```

### Blob

```rust
/// A git blob — raw file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub data: Vec<u8>,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Self;
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError>;
    pub fn serialize_content(&self) -> &[u8];
}
```

### Tree

```rust
/// File mode for tree entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileMode {
    /// Regular file (100644)
    Regular,
    /// Executable file (100755)
    Executable,
    /// Symbolic link (120000)
    Symlink,
    /// Git submodule link (160000)
    Gitlink,
    /// Subdirectory (040000)
    Tree,
    /// Unknown mode (preserved for round-trip)
    Unknown(u32),
}

impl FileMode {
    /// Parse from octal ASCII bytes (e.g., b"100644")
    pub fn from_bytes(s: &[u8]) -> Result<Self, ObjectError>;

    /// Serialize to octal ASCII bytes
    pub fn as_bytes(&self) -> BString;

    /// Get the raw numeric value
    pub fn raw(&self) -> u32;

    /// Is this a tree (directory) entry?
    pub fn is_tree(&self) -> bool;

    /// Is this a blob (file) entry?
    pub fn is_blob(&self) -> bool;
}

/// A single entry in a git tree object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: FileMode,
    pub name: BString,
    pub oid: ObjectId,
}

impl TreeEntry {
    /// Compare entries using git's tree sorting rules.
    /// Directories sort as if they have a trailing '/'.
    pub fn cmp_entries(a: &TreeEntry, b: &TreeEntry) -> std::cmp::Ordering;
}

impl PartialOrd for TreeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Self::cmp_entries(self, other))
    }
}

impl Ord for TreeEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Self::cmp_entries(self, other)
    }
}

/// A git tree object — a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn new() -> Self;
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError>;
    pub fn serialize_content(&self) -> Vec<u8>;
    /// Sort entries in git canonical order
    pub fn sort(&mut self);
    /// Lookup an entry by name
    pub fn find(&self, name: &BStr) -> Option<&TreeEntry>;
    /// Iterate entries
    pub fn iter(&self) -> impl Iterator<Item = &TreeEntry>;
}
```

### Commit

```rust
/// A git commit object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// OID of the root tree
    pub tree: ObjectId,
    /// Parent commit OIDs (empty for root commit)
    pub parents: Vec<ObjectId>,
    /// Author identity and timestamp
    pub author: Signature,
    /// Committer identity and timestamp
    pub committer: Signature,
    /// Optional encoding header (e.g., "UTF-8", "ISO-8859-1")
    pub encoding: Option<BString>,
    /// Optional GPG signature
    pub gpgsig: Option<BString>,
    /// Extra headers (mergetag, etc.) preserved for round-trip
    pub extra_headers: Vec<(BString, BString)>,
    /// Commit message (including any leading blank line convention)
    pub message: BString,
}

impl Commit {
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError>;
    pub fn serialize_content(&self) -> Vec<u8>;

    /// Get the first parent (or None for root commits)
    pub fn first_parent(&self) -> Option<&ObjectId>;
    /// Is this a merge commit? (more than one parent)
    pub fn is_merge(&self) -> bool;
    /// Is this a root commit? (no parents)
    pub fn is_root(&self) -> bool;
    /// Get just the summary (first line) of the message
    pub fn summary(&self) -> &BStr;
    /// Get the message body (everything after the first paragraph)
    pub fn body(&self) -> Option<&BStr>;
}
```

### Tag

```rust
/// A git annotated tag object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// OID of the tagged object
    pub target: ObjectId,
    /// Type of the tagged object
    pub target_type: ObjectType,
    /// Tag name
    pub tag_name: BString,
    /// Tagger identity and timestamp (optional for some old tags)
    pub tagger: Option<Signature>,
    /// Tag message
    pub message: BString,
    /// Optional GPG signature
    pub gpgsig: Option<BString>,
}

impl Tag {
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError>;
    pub fn serialize_content(&self) -> Vec<u8>;
}
```

### Object Header

```rust
/// Parse an object header from raw bytes.
/// Returns (type, content_size, header_length).
pub fn parse_header(data: &[u8]) -> Result<(ObjectType, usize, usize), ObjectError>;

/// Write an object header: "<type> <size>\0"
pub fn write_header(obj_type: ObjectType, content_size: usize) -> Vec<u8>;
```

### Object Cache

```rust
/// LRU cache for parsed objects.
pub struct ObjectCache {
    cache: lru::LruCache<ObjectId, Object>,
}

impl ObjectCache {
    /// Create with the given capacity (number of objects)
    pub fn new(capacity: usize) -> Self;
    /// Get a cached object
    pub fn get(&mut self, oid: &ObjectId) -> Option<&Object>;
    /// Insert an object into the cache
    pub fn insert(&mut self, oid: ObjectId, obj: Object);
    /// Clear all cached objects
    pub fn clear(&mut self);
    /// Current number of cached objects
    pub fn len(&self) -> usize;
}
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("invalid object type: {0}")]
    InvalidType(BString),

    #[error("invalid object header: {0}")]
    InvalidHeader(String),

    #[error("truncated object: expected {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },

    #[error("invalid tree entry at offset {offset}: {reason}")]
    InvalidTreeEntry { offset: usize, reason: String },

    #[error("invalid commit: missing '{field}' header")]
    MissingCommitField { field: &'static str },

    #[error("invalid tag: missing '{field}' header")]
    MissingTagField { field: &'static str },

    #[error("invalid file mode: {0}")]
    InvalidFileMode(String),

    #[error("invalid signature: {0}")]
    InvalidSignature(String),

    #[error(transparent)]
    Hash(#[from] git_hash::HashError),
}
```

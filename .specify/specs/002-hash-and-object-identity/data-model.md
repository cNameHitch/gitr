# Data Model: Hash & Object Identity

## Core Types

### Hash Algorithm

```rust
/// Supported git hash algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// SHA-1 (default, 20 bytes / 160 bits)
    Sha1,
    /// SHA-256 (experimental, 32 bytes / 256 bits)
    Sha256,
}

impl HashAlgorithm {
    /// Length of the hash digest in bytes
    pub const fn digest_len(&self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    /// Length of the hex representation
    pub const fn hex_len(&self) -> usize {
        self.digest_len() * 2
    }

    /// The null (all-zeros) OID for this algorithm
    pub fn null_oid(&self) -> ObjectId {
        match self {
            Self::Sha1 => ObjectId::Sha1([0u8; 20]),
            Self::Sha256 => ObjectId::Sha256([0u8; 32]),
        }
    }
}

impl Default for HashAlgorithm {
    fn default() -> Self { Self::Sha1 }
}
```

### Object ID

```rust
/// A git object identifier — the hash of an object's content.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectId {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

impl ObjectId {
    /// The SHA-1 null OID (all zeros)
    pub const NULL_SHA1: Self = Self::Sha1([0u8; 20]);
    /// The SHA-256 null OID (all zeros)
    pub const NULL_SHA256: Self = Self::Sha256([0u8; 32]);

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8], algo: HashAlgorithm) -> Result<Self, HashError>;

    /// Create from hex string
    pub fn from_hex(hex: &str) -> Result<Self, HashError>;

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8];

    /// Get the hash algorithm
    pub fn algorithm(&self) -> HashAlgorithm;

    /// Check if this is the null OID
    pub fn is_null(&self) -> bool;

    /// Get hex string representation
    pub fn to_hex(&self) -> String;

    /// Get the first byte (for fan-out table indexing)
    pub fn first_byte(&self) -> u8;

    /// Check if this OID starts with the given hex prefix
    pub fn starts_with_hex(&self, prefix: &str) -> bool;

    /// Get the loose object path component: "xx/xxxx..."
    pub fn loose_path(&self) -> String;
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl std::fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ObjectId({})", &self.to_hex()[..8])
    }
}

impl std::str::FromStr for ObjectId {
    type Err = HashError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}
```

### Streaming Hasher

```rust
/// Streaming hash computation.
pub struct Hasher {
    inner: HasherInner,
}

enum HasherInner {
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
}

impl Hasher {
    /// Create a new hasher for the given algorithm
    pub fn new(algo: HashAlgorithm) -> Self;

    /// Feed data into the hasher
    pub fn update(&mut self, data: &[u8]);

    /// Finalize and return the ObjectId
    pub fn finalize(self) -> ObjectId;

    /// Convenience: hash data in one call
    pub fn digest(algo: HashAlgorithm, data: &[u8]) -> ObjectId;

    /// Hash a git object: "type len\0content"
    pub fn hash_object(algo: HashAlgorithm, obj_type: &str, data: &[u8]) -> ObjectId;
}

impl std::io::Write for Hasher {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>;
    fn flush(&mut self) -> std::io::Result<()>;
}
```

### Hex Encoding

```rust
/// Fast hex encoding of bytes to a pre-allocated buffer
pub fn hex_encode(bytes: &[u8], buf: &mut [u8]);

/// Hex encode to a new String
pub fn hex_to_string(bytes: &[u8]) -> String;

/// Decode hex string to bytes
pub fn hex_decode(hex: &str, buf: &mut [u8]) -> Result<(), HashError>;

/// Decode hex string to a new Vec
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, HashError>;

/// Check if a string is valid hex
pub fn is_valid_hex(s: &str) -> bool;
```

### OID Collections

```rust
/// Sorted array of ObjectIds with binary search.
/// Equivalent to C git's `oid_array`.
pub struct OidArray {
    oids: Vec<ObjectId>,
    sorted: bool,
}

impl OidArray {
    pub fn new() -> Self;
    pub fn push(&mut self, oid: ObjectId);
    pub fn sort(&mut self);
    pub fn contains(&mut self, oid: &ObjectId) -> bool;
    pub fn lookup(&mut self, oid: &ObjectId) -> Option<usize>;
    pub fn for_each_unique<F: FnMut(&ObjectId) -> Result<()>>(&mut self, f: F) -> Result<()>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = &ObjectId>;
}

/// Hash map keyed by ObjectId.
pub struct OidMap<V> {
    inner: std::collections::HashMap<ObjectId, V>,
}

impl<V> OidMap<V> {
    pub fn new() -> Self;
    pub fn insert(&mut self, oid: ObjectId, value: V) -> Option<V>;
    pub fn get(&self, oid: &ObjectId) -> Option<&V>;
    pub fn get_mut(&mut self, oid: &ObjectId) -> Option<&mut V>;
    pub fn contains_key(&self, oid: &ObjectId) -> bool;
    pub fn remove(&mut self, oid: &ObjectId) -> Option<V>;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (&ObjectId, &V)>;
}

/// Hash set of ObjectIds.
pub struct OidSet {
    inner: std::collections::HashSet<ObjectId>,
}

impl OidSet {
    pub fn new() -> Self;
    pub fn insert(&mut self, oid: ObjectId) -> bool;
    pub fn contains(&self, oid: &ObjectId) -> bool;
    pub fn remove(&mut self, oid: &ObjectId) -> bool;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = &ObjectId>;
}
```

### Fan-out Table

```rust
/// Fan-out table mapping first byte to count/offset.
/// Used in pack index files for fast object lookup.
pub struct FanoutTable {
    /// 256 entries, each containing cumulative count
    table: [u32; 256],
}

impl FanoutTable {
    /// Build from a sorted slice of OIDs
    pub fn build(oids: &[ObjectId]) -> Self;
    /// Get the range of indices for OIDs starting with the given byte
    pub fn range(&self, first_byte: u8) -> std::ops::Range<usize>;
    /// Total number of objects
    pub fn total(&self) -> u32;
    /// Read from binary format (pack index)
    pub fn from_bytes(data: &[u8]) -> Result<Self, HashError>;
    /// Write to binary format
    pub fn to_bytes(&self) -> [u8; 1024]; // 256 * 4 bytes
}
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum HashError {
    #[error("invalid hex character at position {position}: '{character}'")]
    InvalidHex { position: usize, character: char },

    #[error("invalid hex length: expected {expected}, got {actual}")]
    InvalidHexLength { expected: usize, actual: usize },

    #[error("invalid hash length: expected {expected} bytes, got {actual}")]
    InvalidHashLength { expected: usize, actual: usize },

    #[error("ambiguous object name: prefix '{prefix}' matches multiple objects")]
    AmbiguousPrefix { prefix: String },

    #[error("SHA-1 collision detected")]
    Sha1Collision,
}
```

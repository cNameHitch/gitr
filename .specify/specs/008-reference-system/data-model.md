# Data Model: Reference System

## Core Types

```rust
use git_hash::ObjectId;
use bstr::{BStr, BString};

/// A git reference — either direct (points to OID) or symbolic (points to another ref).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// Direct reference to an object
    Direct {
        name: RefName,
        target: ObjectId,
    },
    /// Symbolic reference to another ref
    Symbolic {
        name: RefName,
        target: RefName,
    },
}

impl Reference {
    /// Get the ref name
    pub fn name(&self) -> &RefName;
    /// Resolve to a direct reference (follows symref chain)
    /// Requires a ref store to look up intermediate refs
    pub fn peel_to_direct(&self, store: &dyn RefStore) -> Result<ObjectId, RefError>;
    /// Is this a symbolic ref?
    pub fn is_symbolic(&self) -> bool;
}

/// A validated reference name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefName(BString);

impl RefName {
    /// Create and validate a ref name
    pub fn new(name: impl Into<BString>) -> Result<Self, RefError>;
    /// Create without validation (for internal use)
    pub(crate) fn new_unchecked(name: BString) -> Self;
    /// Get the short name (e.g., "main" from "refs/heads/main")
    pub fn short_name(&self) -> &BStr;
    /// Is this under refs/heads/?
    pub fn is_branch(&self) -> bool;
    /// Is this under refs/tags/?
    pub fn is_tag(&self) -> bool;
    /// Is this under refs/remotes/?
    pub fn is_remote(&self) -> bool;
    /// As raw bytes
    pub fn as_bstr(&self) -> &BStr;
}

impl std::fmt::Display for RefName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

/// Trait for pluggable reference storage backends.
pub trait RefStore: Send + Sync {
    /// Resolve a ref name to a Reference
    fn resolve(&self, name: &RefName) -> Result<Option<Reference>, RefError>;

    /// Resolve a ref name to its final OID (following symrefs)
    fn resolve_to_oid(&self, name: &RefName) -> Result<Option<ObjectId>, RefError>;

    /// Create a new ref transaction
    fn transaction(&self) -> RefTransaction;

    /// Iterate refs with an optional prefix
    fn iter(&self, prefix: Option<&str>) -> Result<Box<dyn Iterator<Item = Result<Reference, RefError>> + '_>, RefError>;

    /// Read the reflog for a ref
    fn reflog(&self, name: &RefName) -> Result<Vec<ReflogEntry>, RefError>;

    /// Append a reflog entry
    fn append_reflog(&self, name: &RefName, entry: &ReflogEntry) -> Result<(), RefError>;
}

/// Atomic batch of ref updates.
pub struct RefTransaction {
    updates: Vec<RefUpdate>,
}

/// A single update within a transaction.
pub struct RefUpdate {
    pub name: RefName,
    pub action: RefUpdateAction,
    pub reflog_message: Option<String>,
}

pub enum RefUpdateAction {
    /// Create a new ref (fails if exists)
    Create { new_target: ObjectId },
    /// Update existing ref with CAS check
    Update {
        old_target: ObjectId,
        new_target: ObjectId,
    },
    /// Delete ref with CAS check
    Delete { old_target: ObjectId },
    /// Set symbolic ref
    SetSymbolic { target: RefName },
}

impl RefTransaction {
    pub fn new() -> Self;
    pub fn update(&mut self, name: RefName, old: ObjectId, new: ObjectId, message: impl Into<String>);
    pub fn create(&mut self, name: RefName, target: ObjectId, message: impl Into<String>);
    pub fn delete(&mut self, name: RefName, old: ObjectId, message: impl Into<String>);
    pub fn set_symbolic(&mut self, name: RefName, target: RefName, message: impl Into<String>);
    /// Commit the transaction atomically
    pub fn commit(self, store: &dyn RefStore) -> Result<(), RefError>;
}

/// A single reflog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
    pub old_oid: ObjectId,
    pub new_oid: ObjectId,
    pub identity: Signature,
    pub message: BString,
}

impl ReflogEntry {
    /// Parse from a reflog line
    pub fn parse(line: &BStr) -> Result<Self, RefError>;
    /// Serialize to a reflog line
    pub fn to_bytes(&self) -> BString;
}

/// Files-backend ref store (loose refs + packed-refs).
pub struct FilesRefStore {
    git_dir: PathBuf,
    packed_refs: Option<PackedRefs>,
}

/// Parsed packed-refs file.
pub struct PackedRefs {
    refs: Vec<PackedRef>,
    sorted: bool,
}

pub struct PackedRef {
    pub name: RefName,
    pub oid: ObjectId,
    pub peeled: Option<ObjectId>,
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum RefError {
    #[error("invalid ref name: {0}")]
    InvalidName(String),

    #[error("ref not found: {0}")]
    NotFound(String),

    #[error("ref update rejected: {name}: expected {expected}, found {actual}")]
    CasFailed {
        name: String,
        expected: ObjectId,
        actual: ObjectId,
    },

    #[error("symbolic ref loop detected: {0}")]
    SymrefLoop(String),

    #[error("transaction failed: {0}")]
    TransactionFailed(String),

    #[error(transparent)]
    Lock(#[from] git_utils::LockError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

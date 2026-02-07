use std::path::PathBuf;

use git_hash::ObjectId;

/// Error types for reference operations.
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

    #[error("lock file error: {0}")]
    Lock(#[from] git_utils::LockError),

    #[error("{0}")]
    Util(#[from] git_utils::UtilError),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("ref already exists: {0}")]
    AlreadyExists(String),

    #[error("directory-file conflict: cannot create ref '{name}' because '{conflict}' exists")]
    DirectoryConflict { name: String, conflict: String },

    #[error("packed-refs error: {0}")]
    PackedRefs(String),

    #[error("reflog error: {0}")]
    Reflog(String),

    #[error("I/O error on {path}: {source}")]
    IoPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Hash(#[from] git_hash::HashError),
}

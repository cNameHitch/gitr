use std::path::PathBuf;

/// Base error type for git-utils operations.
#[derive(Debug, thiserror::Error)]
pub enum UtilError {
    #[error("lock file error: {0}")]
    Lock(#[from] LockError),

    #[error("date parse error: {0}")]
    DateParse(String),

    #[error("path error: {0}")]
    Path(String),

    #[error("wildmatch error: {0}")]
    Wildmatch(String),

    #[error("subprocess failed: {command}: {source}")]
    Subprocess {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("subprocess timed out: {command}")]
    SubprocessTimeout { command: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Lock file specific errors.
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("unable to create lock file '{path}': already locked")]
    AlreadyLocked { path: PathBuf },

    #[error("unable to create lock file '{path}': {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unable to commit lock file '{path}': {source}")]
    Commit {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
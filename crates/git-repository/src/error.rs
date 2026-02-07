use std::path::PathBuf;

/// Errors from repository operations.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("not a git repository (or any of the parent directories): {0}")]
    NotFound(PathBuf),

    #[error("invalid git directory: {path}: {reason}")]
    InvalidGitDir { path: PathBuf, reason: String },

    #[error("repository already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("bare repository has no working tree")]
    BareNoWorkTree,

    #[error("unable to read HEAD: {0}")]
    InvalidHead(String),

    #[error(transparent)]
    Config(#[from] git_config::ConfigError),

    #[error(transparent)]
    Odb(#[from] git_odb::OdbError),

    #[error(transparent)]
    Ref(#[from] git_ref::RefError),

    #[error(transparent)]
    Index(#[from] git_index::IndexError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

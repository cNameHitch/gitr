use std::path::PathBuf;

/// Errors that can occur during config operations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid config key: {0}")]
    InvalidKey(String),

    #[error("parse error in {file}:{line}: {message}")]
    Parse {
        file: String,
        line: usize,
        message: String,
    },

    #[error("invalid boolean value: {0}")]
    InvalidBool(String),

    #[error("invalid integer value: {0}")]
    InvalidInt(String),

    #[error("invalid color value: {0}")]
    InvalidColor(String),

    #[error("circular include detected: {0}")]
    CircularInclude(String),

    #[error("include depth limit exceeded (max {0})")]
    IncludeDepthExceeded(usize),

    #[error("config file not found: {0}")]
    FileNotFound(PathBuf),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("lock error: {0}")]
    Lock(#[from] git_utils::UtilError),
}

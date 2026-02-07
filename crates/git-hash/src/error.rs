/// Errors produced by hash and OID operations.
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

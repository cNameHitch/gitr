//! Loose object storage: read, write, and enumerate zlib-compressed objects.
//!
//! Each loose object lives at `.git/objects/XX/YYYY...` where `XX` is the first
//! byte of the OID in hex and `YYYY...` is the rest. The file content is
//! zlib-compressed `"<type> <size>\0<content>"`.

mod enumerate;
mod read;
mod stream;
mod write;

pub use enumerate::LooseObjectIter;
pub use stream::LooseObjectStream;

use git_hash::{HashAlgorithm, ObjectId};
use std::path::{Path, PathBuf};

/// Interface to the loose object directory (`.git/objects/`).
pub struct LooseObjectStore {
    /// Path to the objects directory.
    objects_dir: PathBuf,
    /// Hash algorithm in use.
    hash_algo: HashAlgorithm,
    /// Zlib compression level.
    compression_level: flate2::Compression,
}

impl LooseObjectStore {
    /// Open the loose object store at the given path.
    pub fn open(objects_dir: impl AsRef<Path>, hash_algo: HashAlgorithm) -> Self {
        Self {
            objects_dir: objects_dir.as_ref().to_path_buf(),
            hash_algo,
            compression_level: flate2::Compression::default(),
        }
    }

    /// Set the zlib compression level (0–9).
    pub fn set_compression_level(&mut self, level: u32) {
        self.compression_level = flate2::Compression::new(level);
    }

    /// Get the hash algorithm in use.
    pub fn hash_algo(&self) -> HashAlgorithm {
        self.hash_algo
    }

    /// Get the file path for a given OID.
    pub fn object_path(&self, oid: &ObjectId) -> PathBuf {
        self.objects_dir.join(oid.loose_path())
    }
}

/// Errors from loose object operations.
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
        expected: String,
        actual: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("object parse error: {0}")]
    Object(#[from] git_object::ObjectError),

    #[error("hash error: {0}")]
    Hash(#[from] git_hash::HashError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_path_sha1() {
        let store = LooseObjectStore::open("/tmp/objects", HashAlgorithm::Sha1);
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        let path = store.object_path(&oid);
        assert_eq!(
            path,
            PathBuf::from("/tmp/objects/da/39a3ee5e6b4b0d3255bfef95601890afd80709")
        );
    }

    #[test]
    fn set_compression_level() {
        let mut store = LooseObjectStore::open("/tmp/objects", HashAlgorithm::Sha1);
        store.set_compression_level(9);
    }
}

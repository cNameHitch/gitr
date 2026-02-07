//! Git object model: blob, tree, commit, tag parsing and serialization.
//!
//! This crate provides Rust types for git's four object types, their parsing
//! from raw bytes, serialization to canonical format, and supporting types
//! like `ObjectType`, `FileMode`, and `ObjectCache`.

mod blob;
mod commit;
pub mod header;
pub mod name;
mod tag;
mod tree;
pub mod cache;

pub use blob::Blob;
pub use commit::Commit;
pub use tag::Tag;
pub use tree::{FileMode, Tree, TreeEntry};

use bstr::BString;
use git_hash::{HashAlgorithm, HashError, ObjectId};

/// Errors produced by object operations.
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
    Hash(#[from] HashError),
}

/// The four types of git objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl ObjectType {
    /// Parse from the type string in object headers.
    pub fn from_bytes(s: &[u8]) -> Result<Self, ObjectError> {
        match s {
            b"blob" => Ok(Self::Blob),
            b"tree" => Ok(Self::Tree),
            b"commit" => Ok(Self::Commit),
            b"tag" => Ok(Self::Tag),
            _ => Err(ObjectError::InvalidType(BString::from(s))),
        }
    }

    /// The canonical byte representation.
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
        f.write_str(match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
            Self::Tag => "tag",
        })
    }
}

impl std::str::FromStr for ObjectType {
    type Err = ObjectError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bytes(s.as_bytes())
    }
}

/// A parsed git object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
    Tag(Tag),
}

impl Object {
    /// Parse from raw bytes (header + content).
    pub fn parse(data: &[u8]) -> Result<Self, ObjectError> {
        let (obj_type, content_size, header_len) = header::parse_header(data)?;
        let content = &data[header_len..];
        if content.len() < content_size {
            return Err(ObjectError::Truncated {
                expected: content_size,
                actual: content.len(),
            });
        }
        Self::parse_content(obj_type, &content[..content_size])
    }

    /// Parse from content bytes with known type (no header).
    pub fn parse_content(obj_type: ObjectType, content: &[u8]) -> Result<Self, ObjectError> {
        match obj_type {
            ObjectType::Blob => Ok(Self::Blob(Blob::parse(content)?)),
            ObjectType::Tree => Ok(Self::Tree(Tree::parse(content)?)),
            ObjectType::Commit => Ok(Self::Commit(Commit::parse(content)?)),
            ObjectType::Tag => Ok(Self::Tag(Tag::parse(content)?)),
        }
    }

    /// Serialize to canonical git format (header + content).
    pub fn serialize(&self) -> Vec<u8> {
        let content = self.serialize_content();
        let hdr = header::write_header(self.object_type(), content.len());
        let mut out = Vec::with_capacity(hdr.len() + content.len());
        out.extend_from_slice(&hdr);
        out.extend_from_slice(&content);
        out
    }

    /// Serialize just the content (no header).
    pub fn serialize_content(&self) -> Vec<u8> {
        match self {
            Self::Blob(b) => b.serialize_content().to_vec(),
            Self::Tree(t) => t.serialize_content(),
            Self::Commit(c) => c.serialize_content(),
            Self::Tag(t) => t.serialize_content(),
        }
    }

    /// Get the object type.
    pub fn object_type(&self) -> ObjectType {
        match self {
            Self::Blob(_) => ObjectType::Blob,
            Self::Tree(_) => ObjectType::Tree,
            Self::Commit(_) => ObjectType::Commit,
            Self::Tag(_) => ObjectType::Tag,
        }
    }

    /// Compute the OID by hashing the serialized form.
    pub fn compute_oid(&self, algo: HashAlgorithm) -> Result<ObjectId, HashError> {
        let content = self.serialize_content();
        git_hash::hasher::Hasher::hash_object(
            algo,
            std::str::from_utf8(self.object_type().as_bytes()).unwrap(),
            &content,
        )
    }

    /// Get the size of the content (excluding header).
    pub fn content_size(&self) -> usize {
        match self {
            Self::Blob(b) => b.data.len(),
            Self::Tree(t) => t.serialize_content().len(),
            Self::Commit(c) => c.serialize_content().len(),
            Self::Tag(t) => t.serialize_content().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_type_from_bytes() {
        assert_eq!(ObjectType::from_bytes(b"blob").unwrap(), ObjectType::Blob);
        assert_eq!(ObjectType::from_bytes(b"tree").unwrap(), ObjectType::Tree);
        assert_eq!(
            ObjectType::from_bytes(b"commit").unwrap(),
            ObjectType::Commit
        );
        assert_eq!(ObjectType::from_bytes(b"tag").unwrap(), ObjectType::Tag);
        assert!(ObjectType::from_bytes(b"unknown").is_err());
    }

    #[test]
    fn object_type_display() {
        assert_eq!(ObjectType::Blob.to_string(), "blob");
        assert_eq!(ObjectType::Commit.to_string(), "commit");
    }

    #[test]
    fn object_type_from_str() {
        assert_eq!("tree".parse::<ObjectType>().unwrap(), ObjectType::Tree);
        assert!("invalid".parse::<ObjectType>().is_err());
    }

    #[test]
    fn object_type_as_bytes() {
        assert_eq!(ObjectType::Blob.as_bytes(), b"blob");
        assert_eq!(ObjectType::Tag.as_bytes(), b"tag");
    }
}

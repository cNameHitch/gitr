//! Packfile reading, writing, delta encoding, and index support.
//!
//! This crate implements git's packfile format — the primary storage
//! optimization that stores objects using delta compression. Packfiles
//! are also the wire format for network transfer (push/fetch).

pub mod bitmap;
pub mod delta;
pub mod entry;
pub mod generate;
pub mod index;
pub mod midx;
pub mod pack;
pub mod revindex;
pub mod verify;
pub mod write;

use git_hash::ObjectId;
use git_object::ObjectType;

/// Errors that can occur during pack operations.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("invalid pack header: {0}")]
    InvalidHeader(String),

    #[error("invalid pack index: {0}")]
    InvalidIndex(String),

    #[error("invalid delta at offset {offset}: {reason}")]
    InvalidDelta { offset: u64, reason: String },

    #[error("delta base not found: {0}")]
    MissingBase(ObjectId),

    #[error("delta chain too deep (>{max_depth} levels) at offset {offset}")]
    DeltaChainTooDeep { offset: u64, max_depth: usize },

    #[error("pack checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: ObjectId, actual: ObjectId },

    #[error("corrupt pack entry at offset {0}")]
    CorruptEntry(u64),

    #[error("unsupported pack version: {0}")]
    UnsupportedVersion(u32),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Object(#[from] git_object::ObjectError),

    #[error(transparent)]
    Hash(#[from] git_hash::HashError),
}

/// Type of a packed object entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackEntryType {
    Commit,
    Tree,
    Blob,
    Tag,
    /// Delta with offset to base in same pack.
    OfsDelta { base_offset: u64 },
    /// Delta referencing base by OID.
    RefDelta { base_oid: ObjectId },
}

impl PackEntryType {
    /// Convert a non-delta pack entry type to an ObjectType.
    pub fn to_object_type(self) -> Option<ObjectType> {
        match self {
            Self::Commit => Some(ObjectType::Commit),
            Self::Tree => Some(ObjectType::Tree),
            Self::Blob => Some(ObjectType::Blob),
            Self::Tag => Some(ObjectType::Tag),
            Self::OfsDelta { .. } | Self::RefDelta { .. } => None,
        }
    }

    /// Type number as used in pack entry headers.
    pub fn type_number(&self) -> u8 {
        match self {
            Self::Commit => 1,
            Self::Tree => 2,
            Self::Blob => 3,
            Self::Tag => 4,
            Self::OfsDelta { .. } => 6,
            Self::RefDelta { .. } => 7,
        }
    }
}

/// A fully resolved object read from a packfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedObject {
    pub obj_type: ObjectType,
    pub data: Vec<u8>,
}

/// Pack format constants.
pub const PACK_SIGNATURE: &[u8; 4] = b"PACK";
pub const PACK_VERSION: u32 = 2;
pub const PACK_HEADER_SIZE: usize = 12;

/// Pack index v2 constants.
pub const IDX_SIGNATURE: [u8; 4] = [0xff, 0x74, 0x4f, 0x63]; // "\377tOc"
pub const IDX_VERSION: u32 = 2;

/// Maximum delta chain depth before we bail out.
pub const MAX_DELTA_CHAIN_DEPTH: usize = 512;

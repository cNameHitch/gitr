//! Reference system for the gitr git implementation.
//!
//! This crate provides the core reference types and operations for git refs:
//! resolving, creating, updating, deleting, enumerating, and maintaining reflogs.
//!
//! The primary backend is the files backend (`FilesRefStore`) which stores
//! loose refs as individual files under `.git/refs/` and packed refs in
//! `.git/packed-refs`. A pluggable `RefStore` trait allows alternative backends.

mod error;
pub mod files;
mod name;
pub mod reflog;
mod store;

pub use error::RefError;
pub use files::packed::{PackedRef, PackedRefs};
pub use files::FilesRefStore;
pub use name::RefName;
pub use reflog::ReflogEntry;
pub use store::{RefStore, RefTransaction, RefUpdate, RefUpdateAction};

/// A git reference — either direct (points to an OID) or symbolic (points to another ref).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// Direct reference to an object.
    Direct {
        name: RefName,
        target: git_hash::ObjectId,
    },
    /// Symbolic reference to another ref.
    Symbolic { name: RefName, target: RefName },
}

impl Reference {
    /// Get the ref name.
    pub fn name(&self) -> &RefName {
        match self {
            Reference::Direct { name, .. } => name,
            Reference::Symbolic { name, .. } => name,
        }
    }

    /// Is this a symbolic ref?
    pub fn is_symbolic(&self) -> bool {
        matches!(self, Reference::Symbolic { .. })
    }

    /// Is this a direct ref?
    pub fn is_direct(&self) -> bool {
        matches!(self, Reference::Direct { .. })
    }

    /// Get the target OID if this is a direct ref.
    pub fn target_oid(&self) -> Option<git_hash::ObjectId> {
        match self {
            Reference::Direct { target, .. } => Some(*target),
            Reference::Symbolic { .. } => None,
        }
    }

    /// Get the symbolic target if this is a symbolic ref.
    pub fn symbolic_target(&self) -> Option<&RefName> {
        match self {
            Reference::Symbolic { target, .. } => Some(target),
            Reference::Direct { .. } => None,
        }
    }

    /// Resolve to a direct OID by following symbolic ref chains.
    /// Requires a ref store to look up intermediate refs.
    pub fn peel_to_oid(&self, store: &dyn RefStore) -> Result<git_hash::ObjectId, RefError> {
        match self {
            Reference::Direct { target, .. } => Ok(*target),
            Reference::Symbolic { target, .. } => store
                .resolve_to_oid(target)?
                .ok_or_else(|| RefError::NotFound(target.to_string())),
        }
    }
}

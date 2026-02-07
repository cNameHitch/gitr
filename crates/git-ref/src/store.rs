use git_hash::ObjectId;

use crate::error::RefError;
use crate::name::RefName;
use crate::reflog::ReflogEntry;
use crate::Reference;

/// Trait for pluggable reference storage backends.
///
/// Provides the core operations: resolve, update, iterate, and reflog access.
/// The default implementation is `FilesRefStore` (loose refs + packed-refs).
pub trait RefStore: Send + Sync {
    /// Resolve a ref name to a Reference (may be Direct or Symbolic).
    fn resolve(&self, name: &RefName) -> Result<Option<Reference>, RefError>;

    /// Resolve a ref name to its final OID, following symbolic ref chains.
    fn resolve_to_oid(&self, name: &RefName) -> Result<Option<ObjectId>, RefError>;

    /// Iterate refs with an optional prefix filter.
    /// Results are sorted lexicographically by full ref name.
    fn iter(
        &self,
        prefix: Option<&str>,
    ) -> Result<Box<dyn Iterator<Item = Result<Reference, RefError>> + '_>, RefError>;

    /// Read the reflog for a ref.
    fn reflog(&self, name: &RefName) -> Result<Vec<ReflogEntry>, RefError>;

    /// Append a reflog entry for a ref.
    fn append_reflog(&self, name: &RefName, entry: &ReflogEntry) -> Result<(), RefError>;
}

/// Atomic batch of ref updates.
///
/// Collects multiple ref updates and applies them atomically:
/// all succeed or all fail.
pub struct RefTransaction {
    pub(crate) updates: Vec<RefUpdate>,
}

/// A single update within a transaction.
pub struct RefUpdate {
    pub name: RefName,
    pub action: RefUpdateAction,
    pub reflog_message: Option<String>,
}

/// The action to perform on a ref within a transaction.
pub enum RefUpdateAction {
    /// Create a new ref (fails if it already exists).
    Create { new_target: ObjectId },
    /// Update an existing ref with compare-and-swap check.
    Update {
        old_target: ObjectId,
        new_target: ObjectId,
    },
    /// Delete a ref with compare-and-swap check.
    Delete { old_target: ObjectId },
    /// Set a symbolic ref to point to another ref.
    SetSymbolic { target: RefName },
}

impl RefTransaction {
    /// Create a new empty transaction.
    pub fn new() -> Self {
        Self {
            updates: Vec::new(),
        }
    }

    /// Add an update (CAS) operation to the transaction.
    pub fn update(
        &mut self,
        name: RefName,
        old: ObjectId,
        new: ObjectId,
        message: impl Into<String>,
    ) {
        self.updates.push(RefUpdate {
            name,
            action: RefUpdateAction::Update {
                old_target: old,
                new_target: new,
            },
            reflog_message: Some(message.into()),
        });
    }

    /// Add a create operation to the transaction.
    pub fn create(&mut self, name: RefName, target: ObjectId, message: impl Into<String>) {
        self.updates.push(RefUpdate {
            name,
            action: RefUpdateAction::Create { new_target: target },
            reflog_message: Some(message.into()),
        });
    }

    /// Add a delete operation to the transaction.
    pub fn delete(&mut self, name: RefName, old: ObjectId, message: impl Into<String>) {
        self.updates.push(RefUpdate {
            name,
            action: RefUpdateAction::Delete { old_target: old },
            reflog_message: Some(message.into()),
        });
    }

    /// Add a set-symbolic operation to the transaction.
    pub fn set_symbolic(
        &mut self,
        name: RefName,
        target: RefName,
        message: impl Into<String>,
    ) {
        self.updates.push(RefUpdate {
            name,
            action: RefUpdateAction::SetSymbolic { target },
            reflog_message: Some(message.into()),
        });
    }

    /// Get the list of updates in this transaction.
    pub fn updates(&self) -> &[RefUpdate] {
        &self.updates
    }

    /// Check if the transaction is empty.
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }
}

impl Default for RefTransaction {
    fn default() -> Self {
        Self::new()
    }
}

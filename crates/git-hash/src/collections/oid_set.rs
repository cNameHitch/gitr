use std::collections::HashSet;

use crate::ObjectId;

/// Hash set of ObjectIds.
pub struct OidSet {
    inner: HashSet<ObjectId>,
}

impl OidSet {
    pub fn new() -> Self {
        Self {
            inner: HashSet::new(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: HashSet::with_capacity(cap),
        }
    }

    /// Insert an OID. Returns `true` if the OID was newly inserted.
    pub fn insert(&mut self, oid: ObjectId) -> bool {
        self.inner.insert(oid)
    }

    pub fn contains(&self, oid: &ObjectId) -> bool {
        self.inner.contains(oid)
    }

    pub fn remove(&mut self, oid: &ObjectId) -> bool {
        self.inner.remove(oid)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ObjectId> {
        self.inner.iter()
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }
}

impl Default for OidSet {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<ObjectId> for OidSet {
    fn from_iter<I: IntoIterator<Item = ObjectId>>(iter: I) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

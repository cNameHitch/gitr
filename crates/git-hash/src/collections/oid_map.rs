use std::collections::HashMap;

use crate::ObjectId;

/// Hash map keyed by ObjectId.
pub struct OidMap<V> {
    inner: HashMap<ObjectId, V>,
}

impl<V> OidMap<V> {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(cap),
        }
    }

    pub fn insert(&mut self, oid: ObjectId, value: V) -> Option<V> {
        self.inner.insert(oid, value)
    }

    pub fn get(&self, oid: &ObjectId) -> Option<&V> {
        self.inner.get(oid)
    }

    pub fn get_mut(&mut self, oid: &ObjectId) -> Option<&mut V> {
        self.inner.get_mut(oid)
    }

    pub fn contains_key(&self, oid: &ObjectId) -> bool {
        self.inner.contains_key(oid)
    }

    pub fn remove(&mut self, oid: &ObjectId) -> Option<V> {
        self.inner.remove(oid)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ObjectId, &V)> {
        self.inner.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &ObjectId> {
        self.inner.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.inner.values()
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }
}

impl<V> Default for OidMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> FromIterator<(ObjectId, V)> for OidMap<V> {
    fn from_iter<I: IntoIterator<Item = (ObjectId, V)>>(iter: I) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

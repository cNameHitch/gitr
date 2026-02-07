//! LRU cache for parsed git objects.

use std::num::NonZeroUsize;

use git_hash::ObjectId;
use lru::LruCache;

use crate::Object;

/// LRU cache for parsed objects.
pub struct ObjectCache {
    cache: LruCache<ObjectId, Object>,
}

impl ObjectCache {
    /// Create with the given capacity (number of objects).
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap()),
            ),
        }
    }

    /// Get a cached object (promotes it to most-recently-used).
    pub fn get(&mut self, oid: &ObjectId) -> Option<&Object> {
        self.cache.get(oid)
    }

    /// Peek at a cached object without promoting it.
    pub fn peek(&self, oid: &ObjectId) -> Option<&Object> {
        self.cache.peek(oid)
    }

    /// Insert an object into the cache. Returns the evicted entry if the cache was full.
    pub fn insert(&mut self, oid: ObjectId, obj: Object) -> Option<(ObjectId, Object)> {
        self.cache.push(oid, obj)
    }

    /// Clear all cached objects.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Current number of cached objects.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Check if an OID is in the cache (without promoting).
    pub fn contains(&self, oid: &ObjectId) -> bool {
        self.cache.contains(oid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Blob;

    fn make_obj(n: u8) -> (ObjectId, Object) {
        let mut bytes = [0u8; 20];
        bytes[0] = n;
        let oid = ObjectId::from_bytes(&bytes, git_hash::HashAlgorithm::Sha1).unwrap();
        let obj = Object::Blob(Blob::new(vec![n]));
        (oid, obj)
    }

    #[test]
    fn insert_and_get() {
        let mut cache = ObjectCache::new(10);
        let (oid, obj) = make_obj(1);
        cache.insert(oid, obj.clone());
        assert_eq!(cache.get(&oid), Some(&obj));
    }

    #[test]
    fn cache_miss() {
        let mut cache = ObjectCache::new(10);
        let (oid, _) = make_obj(1);
        assert_eq!(cache.get(&oid), None);
    }

    #[test]
    fn lru_eviction() {
        let mut cache = ObjectCache::new(2);
        let (oid1, obj1) = make_obj(1);
        let (oid2, obj2) = make_obj(2);
        let (oid3, obj3) = make_obj(3);

        cache.insert(oid1, obj1);
        cache.insert(oid2, obj2);
        assert_eq!(cache.len(), 2);

        // Inserting a third should evict oid1 (least recently used).
        cache.insert(oid3, obj3);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&oid1).is_none());
        assert!(cache.get(&oid2).is_some());
        assert!(cache.get(&oid3).is_some());
    }

    #[test]
    fn clear() {
        let mut cache = ObjectCache::new(10);
        let (oid, obj) = make_obj(1);
        cache.insert(oid, obj);
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn access_promotes() {
        let mut cache = ObjectCache::new(2);
        let (oid1, obj1) = make_obj(1);
        let (oid2, obj2) = make_obj(2);
        let (oid3, obj3) = make_obj(3);

        cache.insert(oid1, obj1);
        cache.insert(oid2, obj2);

        // Access oid1 to make it most-recently-used.
        cache.get(&oid1);

        // Now inserting oid3 should evict oid2 (the LRU).
        cache.insert(oid3, obj3);
        assert!(cache.get(&oid1).is_some());
        assert!(cache.get(&oid2).is_none());
    }
}

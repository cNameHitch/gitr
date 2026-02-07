use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};

/// Extension trait providing git-specific convenience methods for HashMap.
pub trait GitHashMapExt<K, V> {
    /// Get or insert a default value, returning a mutable reference.
    fn get_or_insert_default(&mut self, key: K) -> &mut V
    where
        V: Default;

    /// Insert if not present, returning whether the insertion happened.
    fn insert_if_absent(&mut self, key: K, value: V) -> bool;
}

impl<K, V, S> GitHashMapExt<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn get_or_insert_default(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        self.entry(key).or_default()
    }

    fn insert_if_absent(&mut self, key: K, value: V) -> bool {
        use std::collections::hash_map::Entry;
        match self.entry(key) {
            Entry::Occupied(_) => false,
            Entry::Vacant(e) => {
                e.insert(value);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_or_insert_default_creates_entry() {
        let mut map: HashMap<&str, Vec<i32>> = HashMap::new();
        let v = map.get_or_insert_default("key");
        v.push(42);
        assert_eq!(map.get("key"), Some(&vec![42]));
    }

    #[test]
    fn insert_if_absent_works() {
        let mut map = HashMap::new();
        assert!(map.insert_if_absent("key", 1));
        assert!(!map.insert_if_absent("key", 2));
        assert_eq!(map.get("key"), Some(&1));
    }
}

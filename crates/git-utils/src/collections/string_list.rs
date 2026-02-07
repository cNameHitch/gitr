use bstr::{BString, ByteSlice};
use std::cmp::Ordering;

/// An item in a `StringList`, holding a byte string and optional associated data.
#[derive(Debug, Clone)]
pub struct StringListItem<T = ()> {
    pub string: BString,
    pub util: T,
}

/// A list of byte strings with optional associated data, supporting both
/// sorted (binary search) and unsorted (append) modes.
///
/// This mirrors C git's `string_list` data structure.
#[derive(Debug, Clone)]
pub struct StringList<T = ()> {
    items: Vec<StringListItem<T>>,
    sorted: bool,
    case_insensitive: bool,
}

impl<T> StringList<T> {
    /// Create a new sorted string list (uses binary search for lookups).
    pub fn new_sorted() -> Self {
        Self {
            items: Vec::new(),
            sorted: true,
            case_insensitive: false,
        }
    }

    /// Create a new unsorted string list (appends at end, linear search).
    pub fn new_unsorted() -> Self {
        Self {
            items: Vec::new(),
            sorted: false,
            case_insensitive: false,
        }
    }

    /// Set case-insensitive comparison mode.
    pub fn set_case_insensitive(&mut self, ci: bool) {
        self.case_insensitive = ci;
    }

    /// Compare two byte strings according to the list's comparison mode.
    fn compare(&self, a: &[u8], b: &[u8]) -> Ordering {
        if self.case_insensitive {
            let a_lower: Vec<u8> = a.iter().map(|c| c.to_ascii_lowercase()).collect();
            let b_lower: Vec<u8> = b.iter().map(|c| c.to_ascii_lowercase()).collect();
            a_lower.cmp(&b_lower)
        } else {
            a.cmp(b)
        }
    }

    /// Binary search for the insertion point. Returns `(index, exact_match)`.
    fn get_entry_index(&self, string: &[u8]) -> (usize, bool) {
        if self.items.is_empty() {
            return (0, false);
        }

        let mut left = 0usize;
        let mut right = self.items.len();

        while left < right {
            let middle = left + (right - left) / 2;
            match self.compare(string, self.items[middle].string.as_bytes()) {
                Ordering::Less => right = middle,
                Ordering::Greater => left = middle + 1,
                Ordering::Equal => return (middle, true),
            }
        }

        (right, false)
    }

    /// Insert a string into a sorted list. If the string already exists, returns
    /// a reference to the existing item. For sorted lists, maintains sort order.
    /// For unsorted lists, appends at the end.
    pub fn insert(&mut self, string: impl Into<BString>, util: T) -> &mut StringListItem<T> {
        let string = string.into();
        if self.sorted {
            let (index, exact) = self.get_entry_index(string.as_bytes());
            if exact {
                return &mut self.items[index];
            }
            self.items.insert(index, StringListItem { string, util });
            &mut self.items[index]
        } else {
            self.items.push(StringListItem { string, util });
            self.items.last_mut().unwrap()
        }
    }

    /// Look up a string, returning the item if found.
    pub fn lookup(&self, string: &[u8]) -> Option<&StringListItem<T>> {
        if self.sorted {
            let (index, exact) = self.get_entry_index(string);
            if exact {
                Some(&self.items[index])
            } else {
                None
            }
        } else {
            self.items
                .iter()
                .find(|item| self.compare(string, item.string.as_bytes()) == Ordering::Equal)
        }
    }

    /// Check if the list contains the given string.
    pub fn has_string(&self, string: &[u8]) -> bool {
        self.lookup(string).is_some()
    }

    /// Iterate over all items, calling `f` on each. Stops early if `f` returns `Err`.
    pub fn for_each<F>(&self, mut f: F) -> std::result::Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(&StringListItem<T>) -> std::result::Result<(), Box<dyn std::error::Error>>,
    {
        for item in &self.items {
            f(item)?;
        }
        Ok(())
    }

    /// Remove a string from a sorted list.
    pub fn remove(&mut self, string: &[u8]) -> bool {
        if self.sorted {
            let (index, exact) = self.get_entry_index(string);
            if exact {
                self.items.remove(index);
                return true;
            }
        } else if let Some(pos) = self
            .items
            .iter()
            .position(|item| self.compare(string, item.string.as_bytes()) == Ordering::Equal)
        {
            self.items.swap_remove(pos);
            return true;
        }
        false
    }

    /// Sort an unsorted list.
    pub fn sort(&mut self) {
        let ci = self.case_insensitive;
        self.items.sort_by(|a, b| {
            if ci {
                let a_lower: Vec<u8> =
                    a.string.as_bytes().iter().map(|c| c.to_ascii_lowercase()).collect();
                let b_lower: Vec<u8> =
                    b.string.as_bytes().iter().map(|c| c.to_ascii_lowercase()).collect();
                a_lower.cmp(&b_lower)
            } else {
                a.string.cmp(&b.string)
            }
        });
        self.sorted = true;
    }

    /// Remove consecutive duplicates from a sorted list.
    pub fn remove_duplicates(&mut self) {
        if self.items.len() <= 1 {
            return;
        }
        let ci = self.case_insensitive;
        self.items.dedup_by(|a, b| {
            if ci {
                a.string
                    .as_bytes()
                    .iter()
                    .map(|c| c.to_ascii_lowercase())
                    .eq(b.string.as_bytes().iter().map(|c| c.to_ascii_lowercase()))
            } else {
                a.string == b.string
            }
        });
    }

    /// Get the number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get an iterator over items.
    pub fn iter(&self) -> impl Iterator<Item = &StringListItem<T>> {
        self.items.iter()
    }

    /// Get a mutable iterator over items.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut StringListItem<T>> {
        self.items.iter_mut()
    }
}

impl StringList<()> {
    /// Convenience: insert a string with no associated data.
    pub fn insert_str(&mut self, string: impl Into<BString>) -> &mut StringListItem<()> {
        self.insert(string, ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_insert_and_lookup() {
        let mut list = StringList::new_sorted();
        list.insert_str("cherry");
        list.insert_str("apple");
        list.insert_str("banana");

        assert!(list.has_string(b"apple"));
        assert!(list.has_string(b"banana"));
        assert!(list.has_string(b"cherry"));
        assert!(!list.has_string(b"date"));

        // Items should be in sorted order
        let strings: Vec<&[u8]> = list.iter().map(|i| i.string.as_bytes()).collect();
        assert_eq!(
            strings,
            vec!["apple".as_bytes(), "banana".as_bytes(), "cherry".as_bytes()]
        );
    }

    #[test]
    fn sorted_no_duplicates() {
        let mut list = StringList::new_sorted();
        list.insert_str("apple");
        list.insert_str("apple");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn unsorted_append() {
        let mut list = StringList::new_unsorted();
        list.insert_str("cherry");
        list.insert_str("apple");
        list.insert_str("banana");

        let strings: Vec<&[u8]> = list.iter().map(|i| i.string.as_bytes()).collect();
        assert_eq!(
            strings,
            vec!["cherry".as_bytes(), "apple".as_bytes(), "banana".as_bytes()]
        );
    }

    #[test]
    fn unsorted_lookup() {
        let mut list = StringList::new_unsorted();
        list.insert_str("foo");
        list.insert_str("bar");
        assert!(list.has_string(b"foo"));
        assert!(!list.has_string(b"baz"));
    }

    #[test]
    fn case_insensitive() {
        let mut list = StringList::new_sorted();
        list.set_case_insensitive(true);
        list.insert_str("Apple");
        assert!(list.has_string(b"apple"));
        assert!(list.has_string(b"APPLE"));
    }

    #[test]
    fn remove_from_sorted() {
        let mut list = StringList::new_sorted();
        list.insert_str("a");
        list.insert_str("b");
        list.insert_str("c");
        assert!(list.remove(b"b"));
        assert_eq!(list.len(), 2);
        assert!(!list.has_string(b"b"));
    }

    #[test]
    fn with_util_data() {
        let mut list: StringList<i32> = StringList::new_sorted();
        list.insert(BString::from("key1"), 42);
        list.insert(BString::from("key2"), 99);

        let item = list.lookup(b"key1").unwrap();
        assert_eq!(item.util, 42);
    }

    #[test]
    fn sort_unsorted() {
        let mut list = StringList::new_unsorted();
        list.insert_str("c");
        list.insert_str("a");
        list.insert_str("b");
        list.sort();

        let strings: Vec<&[u8]> = list.iter().map(|i| i.string.as_bytes()).collect();
        assert_eq!(strings, vec![b"a", b"b", b"c"]);
    }
}

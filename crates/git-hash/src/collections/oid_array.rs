use crate::ObjectId;

/// Sorted array of ObjectIds with binary search.
///
/// Equivalent to C git's `oid_array`. OIDs are stored in a `Vec` and
/// lazily sorted on the first lookup or iteration that requires order.
pub struct OidArray {
    oids: Vec<ObjectId>,
    sorted: bool,
}

impl OidArray {
    pub fn new() -> Self {
        Self {
            oids: Vec::new(),
            sorted: true,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            oids: Vec::with_capacity(cap),
            sorted: true,
        }
    }

    /// Append an OID. Marks the array as unsorted.
    pub fn push(&mut self, oid: ObjectId) {
        self.oids.push(oid);
        if self.oids.len() > 1 {
            self.sorted = false;
        }
    }

    /// Sort the array if not already sorted.
    pub fn sort(&mut self) {
        if !self.sorted {
            self.oids.sort();
            self.sorted = true;
        }
    }

    /// Check if the array contains the given OID (sorts first if needed).
    pub fn contains(&mut self, oid: &ObjectId) -> bool {
        self.sort();
        self.oids.binary_search(oid).is_ok()
    }

    /// Binary search for an OID. Returns the index if found (sorts first if needed).
    pub fn lookup(&mut self, oid: &ObjectId) -> Option<usize> {
        self.sort();
        self.oids.binary_search(oid).ok()
    }

    /// Iterate over each unique OID (sorts and deduplicates).
    pub fn for_each_unique<F>(&mut self, mut f: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(&ObjectId) -> Result<(), Box<dyn std::error::Error>>,
    {
        self.sort();
        let mut prev: Option<&ObjectId> = None;
        for oid in &self.oids {
            if prev != Some(oid) {
                f(oid)?;
            }
            prev = Some(oid);
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.oids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.oids.is_empty()
    }

    /// Iterate over all OIDs (in current order — may be unsorted).
    pub fn iter(&self) -> impl Iterator<Item = &ObjectId> {
        self.oids.iter()
    }

    /// Iterate over all OIDs in sorted order.
    pub fn iter_sorted(&mut self) -> impl Iterator<Item = &ObjectId> {
        self.sort();
        self.oids.iter()
    }

    /// Clear the array.
    pub fn clear(&mut self) {
        self.oids.clear();
        self.sorted = true;
    }

    /// Find all OIDs whose hex representation starts with the given prefix.
    pub fn find_by_prefix(&mut self, prefix: &str) -> Vec<ObjectId> {
        self.sort();
        self.oids
            .iter()
            .filter(|oid| oid.starts_with_hex(prefix))
            .copied()
            .collect()
    }
}

impl Default for OidArray {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<ObjectId> for OidArray {
    fn from_iter<I: IntoIterator<Item = ObjectId>>(iter: I) -> Self {
        let oids: Vec<ObjectId> = iter.into_iter().collect();
        let sorted = oids.windows(2).all(|w| w[0] <= w[1]);
        Self { oids, sorted }
    }
}

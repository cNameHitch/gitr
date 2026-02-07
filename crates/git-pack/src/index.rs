//! Pack index (v2) reading and lookup.
//!
//! The pack index provides fast OID → offset mapping using a fan-out table
//! and binary search. Format:
//!
//! ```text
//! Header:  \xff tOc (4 bytes) | version (4 bytes = 2)
//! Fanout:  256 × 4-byte big-endian cumulative counts
//! OIDs:    N × 20-byte sorted OIDs
//! CRC32:   N × 4-byte CRC32 values
//! Offsets: N × 4-byte offsets (high bit = 1 → use 64-bit table)
//! 64-bit:  M × 8-byte offsets (for packs > 2GB)
//! Trailer: 20-byte pack checksum | 20-byte index checksum
//! ```

use std::path::{Path, PathBuf};

use git_hash::{HashAlgorithm, ObjectId};
use memmap2::Mmap;

use crate::{IDX_SIGNATURE, IDX_VERSION, PackError};

/// Pack index (v2) providing OID → offset mapping.
pub struct PackIndex {
    data: Mmap,
    version: u32,
    num_objects: u32,
    /// Byte offset where the fanout table starts (after 8-byte header).
    fanout_offset: usize,
    /// Byte offset where sorted OIDs start.
    oid_offset: usize,
    /// Byte offset where CRC32 values start.
    crc_offset: usize,
    /// Byte offset where 32-bit offsets start.
    offset32_offset: usize,
    /// Byte offset where 64-bit offsets start (if any).
    offset64_offset: usize,
    /// Path to the .idx file.
    idx_path: PathBuf,
    /// Hash algorithm (SHA-1 for now).
    hash_algo: HashAlgorithm,
}

impl PackIndex {
    /// Open a pack index file.
    pub fn open(idx_path: impl AsRef<Path>) -> Result<Self, PackError> {
        let idx_path = idx_path.as_ref().to_path_buf();
        let file = std::fs::File::open(&idx_path)?;
        let data = unsafe { Mmap::map(&file)? };

        // For now we only support SHA-1
        let hash_algo = HashAlgorithm::Sha1;
        let hash_len = hash_algo.digest_len(); // 20

        // Minimum size: header(8) + fanout(1024) + trailer(2 * hash_len)
        if data.len() < 8 + 1024 + 2 * hash_len {
            return Err(PackError::InvalidIndex("file too small".into()));
        }

        // Validate header
        if data[0..4] != IDX_SIGNATURE {
            return Err(PackError::InvalidIndex("bad signature".into()));
        }
        let version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if version != IDX_VERSION {
            return Err(PackError::InvalidIndex(format!(
                "unsupported version {version}, expected {IDX_VERSION}"
            )));
        }

        // Read number of objects from last fanout entry
        let fanout_offset = 8;
        let last_fanout_pos = fanout_offset + 255 * 4;
        let num_objects = u32::from_be_bytes([
            data[last_fanout_pos],
            data[last_fanout_pos + 1],
            data[last_fanout_pos + 2],
            data[last_fanout_pos + 3],
        ]);

        let n = num_objects as usize;
        let oid_offset = fanout_offset + 1024;
        let crc_offset = oid_offset + n * hash_len;
        let offset32_offset = crc_offset + n * 4;
        let offset64_offset = offset32_offset + n * 4;

        // Validate minimum expected file size
        // (offset64 table size is variable, trailer is 2 * hash_len)
        let min_size = offset64_offset + 2 * hash_len;
        if data.len() < min_size {
            return Err(PackError::InvalidIndex(format!(
                "file too small: {} < {min_size}",
                data.len()
            )));
        }

        Ok(Self {
            data,
            version,
            num_objects,
            fanout_offset,
            oid_offset,
            crc_offset,
            offset32_offset,
            offset64_offset,
            idx_path,
            hash_algo,
        })
    }

    /// Look up an OID, returning the offset in the pack file.
    pub fn lookup(&self, oid: &ObjectId) -> Option<u64> {
        let (lo, hi) = self.fanout_range(oid.first_byte());
        if lo >= hi {
            return None;
        }
        // Binary search within the range
        let target = oid.as_bytes();

        let mut low = lo;
        let mut high = hi;
        while low < high {
            let mid = low + (high - low) / 2;
            let mid_oid = self.oid_bytes_at(mid);
            match mid_oid.cmp(target) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => {
                    return Some(self.offset_at_index(mid as u32));
                }
            }
        }
        None
    }

    /// Look up by OID prefix, returning all matches as (OID, offset) pairs.
    pub fn lookup_prefix(&self, prefix: &[u8]) -> Vec<(ObjectId, u64)> {
        if prefix.is_empty() {
            return Vec::new();
        }

        let first_byte = prefix[0];
        let (lo, hi) = self.fanout_range(first_byte);

        let mut results = Vec::new();
        for i in lo..hi {
            let oid_bytes = self.oid_bytes_at(i);
            if oid_bytes.len() >= prefix.len() && oid_bytes[..prefix.len()] == *prefix {
                if let Ok(oid) = ObjectId::from_bytes(oid_bytes, self.hash_algo) {
                    results.push((oid, self.offset_at_index(i as u32)));
                }
            }
        }
        results
    }

    /// Get the OID at the given sorted index position.
    pub fn oid_at_index(&self, index: u32) -> ObjectId {
        let hash_len = self.hash_algo.digest_len();
        let start = self.oid_offset + index as usize * hash_len;
        ObjectId::from_bytes(&self.data[start..start + hash_len], self.hash_algo)
            .expect("valid OID in index")
    }

    /// Get the pack file offset at the given sorted index position.
    pub fn offset_at_index(&self, index: u32) -> u64 {
        let pos = self.offset32_offset + index as usize * 4;
        let val = u32::from_be_bytes([
            self.data[pos],
            self.data[pos + 1],
            self.data[pos + 2],
            self.data[pos + 3],
        ]);

        if val & 0x8000_0000 != 0 {
            // 64-bit offset: high bit is set, lower 31 bits index into 64-bit table
            let idx64 = (val & 0x7FFF_FFFF) as usize;
            let pos64 = self.offset64_offset + idx64 * 8;
            u64::from_be_bytes([
                self.data[pos64],
                self.data[pos64 + 1],
                self.data[pos64 + 2],
                self.data[pos64 + 3],
                self.data[pos64 + 4],
                self.data[pos64 + 5],
                self.data[pos64 + 6],
                self.data[pos64 + 7],
            ])
        } else {
            val as u64
        }
    }

    /// Get the CRC32 at the given sorted index position.
    pub fn crc32_at_index(&self, index: u32) -> u32 {
        let pos = self.crc_offset + index as usize * 4;
        u32::from_be_bytes([
            self.data[pos],
            self.data[pos + 1],
            self.data[pos + 2],
            self.data[pos + 3],
        ])
    }

    /// Total number of objects in this index.
    pub fn num_objects(&self) -> u32 {
        self.num_objects
    }

    /// Index version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Path to the .idx file.
    pub fn path(&self) -> &Path {
        &self.idx_path
    }

    /// Pack checksum stored in the index trailer.
    pub fn pack_checksum(&self) -> ObjectId {
        let hash_len = self.hash_algo.digest_len();
        let start = self.data.len() - 2 * hash_len;
        ObjectId::from_bytes(&self.data[start..start + hash_len], self.hash_algo)
            .expect("valid checksum in index trailer")
    }

    /// Index checksum (the trailing hash of the index file itself).
    pub fn index_checksum(&self) -> ObjectId {
        let hash_len = self.hash_algo.digest_len();
        let start = self.data.len() - hash_len;
        ObjectId::from_bytes(&self.data[start..start + hash_len], self.hash_algo)
            .expect("valid checksum in index trailer")
    }

    /// Iterate over all (OID, offset) pairs in sorted order.
    pub fn iter(&self) -> PackIndexIter<'_> {
        PackIndexIter {
            index: self,
            pos: 0,
        }
    }

    /// Get the fan-out range for a given first byte.
    /// Returns (start, end) indices into the sorted OID list.
    fn fanout_range(&self, first_byte: u8) -> (usize, usize) {
        let end = self.fanout_entry(first_byte) as usize;
        let start = if first_byte == 0 {
            0
        } else {
            self.fanout_entry(first_byte - 1) as usize
        };
        (start, end)
    }

    /// Read a single fanout table entry.
    fn fanout_entry(&self, index: u8) -> u32 {
        let pos = self.fanout_offset + index as usize * 4;
        u32::from_be_bytes([
            self.data[pos],
            self.data[pos + 1],
            self.data[pos + 2],
            self.data[pos + 3],
        ])
    }

    /// Raw OID bytes at the given sorted index position.
    fn oid_bytes_at(&self, index: usize) -> &[u8] {
        let hash_len = self.hash_algo.digest_len();
        let start = self.oid_offset + index * hash_len;
        &self.data[start..start + hash_len]
    }
}

/// Iterator over (OID, offset) pairs in a pack index.
pub struct PackIndexIter<'a> {
    index: &'a PackIndex,
    pos: u32,
}

impl<'a> Iterator for PackIndexIter<'a> {
    type Item = (ObjectId, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.index.num_objects {
            return None;
        }
        let oid = self.index.oid_at_index(self.pos);
        let offset = self.index.offset_at_index(self.pos);
        self.pos += 1;
        Some((oid, offset))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.index.num_objects - self.pos) as usize;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for PackIndexIter<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    use git_hash::hasher::Hasher;
    use std::io::Write;

    /// Build a synthetic v2 pack index in memory for testing.
    fn build_test_index(oids_and_offsets: &[(ObjectId, u64, u32)]) -> Vec<u8> {

        // Sort by OID
        let mut entries: Vec<_> = oids_and_offsets.to_vec();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(&IDX_SIGNATURE);
        buf.extend_from_slice(&IDX_VERSION.to_be_bytes());

        // Fanout table
        let mut fanout = [0u32; 256];
        for (oid, _, _) in &entries {
            let bucket = oid.first_byte() as usize;
            fanout[bucket] += 1;
        }
        for i in 1..256 {
            fanout[i] += fanout[i - 1];
        }
        for count in fanout {
            buf.extend_from_slice(&count.to_be_bytes());
        }

        // OIDs
        for (oid, _, _) in &entries {
            buf.extend_from_slice(oid.as_bytes());
        }

        // CRC32
        for (_, _, crc) in &entries {
            buf.extend_from_slice(&crc.to_be_bytes());
        }

        // 32-bit offsets (no 64-bit for this test helper)
        for (_, offset, _) in &entries {
            buf.extend_from_slice(&(*offset as u32).to_be_bytes());
        }

        // Trailer: pack checksum (fake) + index checksum
        let fake_pack_checksum = [0u8; 20];
        buf.extend_from_slice(&fake_pack_checksum);

        // Compute index checksum over everything so far
        let mut hasher = Hasher::new(HashAlgorithm::Sha1);
        hasher.update(&buf);
        let idx_checksum = hasher.finalize().unwrap();
        buf.extend_from_slice(idx_checksum.as_bytes());

        buf
    }

    fn write_test_index(dir: &Path, data: &[u8]) -> PathBuf {
        let path = dir.join("test.idx");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(data).unwrap();
        path
    }

    fn make_oid(first_byte: u8, suffix: u8) -> ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = first_byte;
        bytes[19] = suffix;
        ObjectId::from_bytes(&bytes, HashAlgorithm::Sha1).unwrap()
    }

    #[test]
    fn open_and_lookup_single_object() {
        let dir = tempfile::tempdir().unwrap();
        let oid = make_oid(0xab, 0x01);
        let data = build_test_index(&[(oid, 12, 0xdeadbeef)]);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        assert_eq!(idx.num_objects(), 1);
        assert_eq!(idx.version(), 2);

        // Successful lookup
        assert_eq!(idx.lookup(&oid), Some(12));

        // Missing lookup
        let missing = make_oid(0xab, 0x02);
        assert_eq!(idx.lookup(&missing), None);
    }

    #[test]
    fn lookup_multiple_objects() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (make_oid(0x00, 0x01), 100, 0x111),
            (make_oid(0x00, 0x02), 200, 0x222),
            (make_oid(0x0a, 0x01), 300, 0x333),
            (make_oid(0xff, 0x01), 400, 0x444),
        ];
        let data = build_test_index(&entries);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        assert_eq!(idx.num_objects(), 4);

        for (oid, offset, _) in &entries {
            assert_eq!(idx.lookup(oid), Some(*offset));
        }
    }

    #[test]
    fn oid_at_index_returns_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (make_oid(0xff, 0x01), 100, 0),
            (make_oid(0x00, 0x01), 200, 0),
            (make_oid(0x55, 0x01), 300, 0),
        ];
        let data = build_test_index(&entries);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        // Should be sorted: 0x00, 0x55, 0xff
        assert_eq!(idx.oid_at_index(0), make_oid(0x00, 0x01));
        assert_eq!(idx.oid_at_index(1), make_oid(0x55, 0x01));
        assert_eq!(idx.oid_at_index(2), make_oid(0xff, 0x01));
    }

    #[test]
    fn crc32_at_index() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (make_oid(0x10, 0x01), 100, 0xAAAA_BBBB),
            (make_oid(0x20, 0x01), 200, 0xCCCC_DDDD),
        ];
        let data = build_test_index(&entries);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        assert_eq!(idx.crc32_at_index(0), 0xAAAA_BBBB);
        assert_eq!(idx.crc32_at_index(1), 0xCCCC_DDDD);
    }

    #[test]
    fn iterator_yields_all_entries() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (make_oid(0x01, 0x01), 100, 0),
            (make_oid(0x02, 0x01), 200, 0),
            (make_oid(0x03, 0x01), 300, 0),
        ];
        let data = build_test_index(&entries);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        let items: Vec<_> = idx.iter().collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].0, make_oid(0x01, 0x01));
        assert_eq!(items[0].1, 100);
    }

    #[test]
    fn lookup_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (make_oid(0xab, 0x01), 100, 0),
            (make_oid(0xab, 0x02), 200, 0),
            (make_oid(0xac, 0x01), 300, 0),
        ];
        let data = build_test_index(&entries);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        let results = idx.lookup_prefix(&[0xab]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn empty_index() {
        let dir = tempfile::tempdir().unwrap();
        let data = build_test_index(&[]);
        let path = write_test_index(dir.path(), &data);

        let idx = PackIndex::open(&path).unwrap();
        assert_eq!(idx.num_objects(), 0);
        assert_eq!(idx.lookup(&make_oid(0x00, 0x00)), None);
        assert_eq!(idx.iter().count(), 0);
    }

    #[test]
    fn build_test_index_with_64bit_offsets() {
        // Manually construct an index with a 64-bit offset entry
        let oid = make_oid(0x42, 0x01);

        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(&IDX_SIGNATURE);
        buf.extend_from_slice(&IDX_VERSION.to_be_bytes());

        // Fanout: 1 object at bucket 0x42
        let mut fanout = [0u32; 256];
        for i in 0x42..256 {
            fanout[i] = 1;
        }
        for count in fanout {
            buf.extend_from_slice(&count.to_be_bytes());
        }

        // OIDs
        buf.extend_from_slice(oid.as_bytes());

        // CRC32
        buf.extend_from_slice(&0u32.to_be_bytes());

        // 32-bit offset with high bit set, pointing to 64-bit entry 0
        buf.extend_from_slice(&0x8000_0000u32.to_be_bytes());

        // 64-bit offset table: one entry at 5GB
        let large_offset: u64 = 5 * 1024 * 1024 * 1024; // 5GB
        buf.extend_from_slice(&large_offset.to_be_bytes());

        // Trailer
        let fake_pack_checksum = [0u8; 20];
        buf.extend_from_slice(&fake_pack_checksum);
        let mut hasher = Hasher::new(HashAlgorithm::Sha1);
        hasher.update(&buf);
        let idx_checksum = hasher.finalize().unwrap();
        buf.extend_from_slice(idx_checksum.as_bytes());

        let dir = tempfile::tempdir().unwrap();
        let path = write_test_index(dir.path(), &buf);

        let idx = PackIndex::open(&path).unwrap();
        assert_eq!(idx.num_objects(), 1);
        assert_eq!(idx.lookup(&oid), Some(large_offset));
    }
}

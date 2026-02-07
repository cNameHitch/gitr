//! Reverse index: offset → OID mapping.
//!
//! The reverse index provides the inverse of the pack index: given a byte
//! offset in a pack file, find the OID and index position of the object
//! at that offset. This can be built in-memory from a pack index, or
//! loaded from a `.rev` file on disk.
//!
//! `.rev` file format:
//! ```text
//! Header: RIDX (4) | version (4) | hash_version (4) | num_objects (4)
//! Body:   N × 4-byte index positions (sorted by pack offset)
//! Trailer: pack checksum (20) | rev checksum (20)
//! ```

use std::path::Path;

use git_hash::{HashAlgorithm, ObjectId};
use memmap2::Mmap;

use crate::index::PackIndex;
use crate::PackError;

/// Reverse index signature.
const RIDX_SIGNATURE: &[u8; 4] = b"RIDX";

/// Reverse index: offset → OID mapping.
pub struct ReverseIndex {
    /// Sorted array of (offset, index_position) pairs.
    entries: Vec<(u64, u32)>,
}

impl ReverseIndex {
    /// Build a reverse index from a pack index (in-memory).
    pub fn build(index: &PackIndex) -> Self {
        let n = index.num_objects();
        let mut entries: Vec<(u64, u32)> = (0..n)
            .map(|i| (index.offset_at_index(i), i))
            .collect();
        entries.sort_by_key(|&(offset, _)| offset);
        Self { entries }
    }

    /// Load a reverse index from a `.rev` file.
    pub fn open(rev_path: impl AsRef<Path>, index: &PackIndex) -> Result<Self, PackError> {
        let rev_path = rev_path.as_ref();
        let file = std::fs::File::open(rev_path)?;
        let data = unsafe { Mmap::map(&file)? };

        let hash_algo = HashAlgorithm::Sha1;
        let hash_len = hash_algo.digest_len();

        // Header: 16 bytes
        if data.len() < 16 {
            return Err(PackError::InvalidIndex("rev file too small".into()));
        }

        if &data[0..4] != RIDX_SIGNATURE {
            return Err(PackError::InvalidIndex("bad RIDX signature".into()));
        }

        let version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if version != 1 {
            return Err(PackError::InvalidIndex(format!(
                "unsupported rev index version {version}"
            )));
        }

        let num_objects = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

        if num_objects != index.num_objects() {
            return Err(PackError::InvalidIndex(format!(
                "rev index has {} objects but pack index has {}",
                num_objects,
                index.num_objects()
            )));
        }

        // Body: num_objects × 4-byte index positions
        let body_offset = 16;
        let body_size = num_objects as usize * 4;
        let expected_size = body_offset + body_size + 2 * hash_len;

        if data.len() < expected_size {
            return Err(PackError::InvalidIndex("rev file too small".into()));
        }

        // Read index positions and pair with offsets from pack index
        let mut entries: Vec<(u64, u32)> = Vec::with_capacity(num_objects as usize);
        for i in 0..num_objects as usize {
            let pos = body_offset + i * 4;
            let idx_pos = u32::from_be_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
            ]);
            let offset = index.offset_at_index(idx_pos);
            entries.push((offset, idx_pos));
        }

        // Already sorted by offset (that's the .rev file ordering)
        Ok(Self { entries })
    }

    /// Look up the OID of the object at the given pack offset.
    pub fn lookup_offset(&self, offset: u64, index: &PackIndex) -> Option<ObjectId> {
        self.entries
            .binary_search_by_key(&offset, |&(off, _)| off)
            .ok()
            .map(|pos| {
                let (_, idx_pos) = self.entries[pos];
                index.oid_at_index(idx_pos)
            })
    }

    /// Look up the index position of the object at the given pack offset.
    pub fn index_position_at_offset(&self, offset: u64) -> Option<u32> {
        self.entries
            .binary_search_by_key(&offset, |&(off, _)| off)
            .ok()
            .map(|pos| self.entries[pos].1)
    }

    /// Number of entries.
    pub fn num_entries(&self) -> usize {
        self.entries.len()
    }

    /// Iterate over (offset, index_position) pairs in offset order.
    pub fn iter(&self) -> impl Iterator<Item = &(u64, u32)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::PackIndex;
    use crate::{IDX_SIGNATURE, IDX_VERSION};
    use git_hash::hasher::Hasher;
    fn make_oid(first_byte: u8, suffix: u8) -> ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = first_byte;
        bytes[19] = suffix;
        ObjectId::from_bytes(&bytes, HashAlgorithm::Sha1).unwrap()
    }

    fn build_test_index(entries: &[(ObjectId, u64, u32)]) -> Vec<u8> {
        let mut sorted: Vec<_> = entries.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut buf = Vec::new();
        buf.extend_from_slice(&IDX_SIGNATURE);
        buf.extend_from_slice(&IDX_VERSION.to_be_bytes());

        let mut fanout = [0u32; 256];
        for (oid, _, _) in &sorted {
            fanout[oid.first_byte() as usize] += 1;
        }
        for i in 1..256 {
            fanout[i] += fanout[i - 1];
        }
        for count in fanout {
            buf.extend_from_slice(&count.to_be_bytes());
        }
        for (oid, _, _) in &sorted {
            buf.extend_from_slice(oid.as_bytes());
        }
        for (_, _, crc) in &sorted {
            buf.extend_from_slice(&crc.to_be_bytes());
        }
        for (_, offset, _) in &sorted {
            buf.extend_from_slice(&(*offset as u32).to_be_bytes());
        }
        let fake_checksum = [0u8; 20];
        buf.extend_from_slice(&fake_checksum);
        let mut hasher = Hasher::new(HashAlgorithm::Sha1);
        hasher.update(&buf);
        let idx_checksum = hasher.finalize().unwrap();
        buf.extend_from_slice(idx_checksum.as_bytes());
        buf
    }

    #[test]
    fn build_from_index() {
        let dir = tempfile::tempdir().unwrap();

        let entries = vec![
            (make_oid(0x10, 0x01), 300u64, 0u32),
            (make_oid(0x20, 0x02), 100, 0),
            (make_oid(0x30, 0x03), 200, 0),
        ];

        let idx_data = build_test_index(&entries);
        let idx_path = dir.path().join("test.idx");
        std::fs::write(&idx_path, &idx_data).unwrap();

        let index = PackIndex::open(&idx_path).unwrap();
        let revindex = ReverseIndex::build(&index);

        assert_eq!(revindex.num_entries(), 3);

        // Look up by offset
        let oid = revindex.lookup_offset(100, &index).unwrap();
        assert_eq!(oid, make_oid(0x20, 0x02));

        let oid = revindex.lookup_offset(200, &index).unwrap();
        assert_eq!(oid, make_oid(0x30, 0x03));

        let oid = revindex.lookup_offset(300, &index).unwrap();
        assert_eq!(oid, make_oid(0x10, 0x01));

        // Missing offset
        assert!(revindex.lookup_offset(999, &index).is_none());
    }

    #[test]
    fn entries_sorted_by_offset() {
        let dir = tempfile::tempdir().unwrap();

        let entries = vec![
            (make_oid(0xff, 0x01), 500u64, 0u32),
            (make_oid(0x01, 0x01), 100, 0),
            (make_oid(0x80, 0x01), 300, 0),
        ];

        let idx_data = build_test_index(&entries);
        let idx_path = dir.path().join("test.idx");
        std::fs::write(&idx_path, &idx_data).unwrap();

        let index = PackIndex::open(&idx_path).unwrap();
        let revindex = ReverseIndex::build(&index);

        let offsets: Vec<u64> = revindex.iter().map(|&(off, _)| off).collect();
        assert_eq!(offsets, vec![100, 300, 500]);
    }
}

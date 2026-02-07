//! Multi-pack index (MIDX) support.
//!
//! The MIDX format uses a chunk-based layout to index objects across
//! multiple pack files. Format:
//!
//! ```text
//! Header: MIDX (4) | version (1) | OID version (1) | num_chunks (1) | num_packs (4)
//! Chunk lookup table: [chunk_id (4) | offset (8)] × num_chunks + terminator
//! Chunks:
//!   - Pack names: null-terminated pack file names
//!   - OID fanout: 256 × 4-byte cumulative counts
//!   - OID lookup: N × (OID bytes) sorted
//!   - Object offsets: N × (pack_index: 4, offset: 4)
//!   - Optional: large offsets (8 bytes each)
//! ```

use std::path::{Path, PathBuf};

use git_hash::{HashAlgorithm, ObjectId};
use memmap2::Mmap;

use crate::PackError;

/// MIDX signature bytes.
const MIDX_SIGNATURE: &[u8; 4] = b"MIDX";

/// Chunk IDs used in MIDX files.
const CHUNK_PACK_NAMES: u32 = 0x504e_414d; // "PNAM"
const CHUNK_OID_FANOUT: u32 = 0x4f49_4446; // "OIDF"
const CHUNK_OID_LOOKUP: u32 = 0x4f49_444c; // "OIDL"
const CHUNK_OBJECT_OFFSETS: u32 = 0x4f4f_4646; // "OOFF"
const CHUNK_LARGE_OFFSETS: u32 = 0x4c4f_4646; // "LOFF"

/// Multi-pack index spanning multiple packfiles.
#[allow(dead_code)]
pub struct MultiPackIndex {
    data: Mmap,
    num_packs: u32,
    num_objects: u32,
    pack_names: Vec<String>,
    hash_algo: HashAlgorithm,
    /// Chunk offsets
    fanout_offset: usize,
    oid_offset: usize,
    offsets_offset: usize,
    large_offsets_offset: Option<usize>,
    midx_path: PathBuf,
}

impl MultiPackIndex {
    /// Open a multi-pack index file.
    pub fn open(midx_path: impl AsRef<Path>) -> Result<Self, PackError> {
        let midx_path = midx_path.as_ref().to_path_buf();
        let file = std::fs::File::open(&midx_path)?;
        let data = unsafe { Mmap::map(&file)? };

        // Minimum header: signature(4) + version(1) + oid_version(1) + num_chunks(1) + reserved(1) + num_packs(4) = 12
        if data.len() < 12 {
            return Err(PackError::InvalidIndex("MIDX file too small".into()));
        }

        // Validate signature
        if &data[0..4] != MIDX_SIGNATURE {
            return Err(PackError::InvalidIndex("bad MIDX signature".into()));
        }

        let version = data[4];
        if version != 1 {
            return Err(PackError::InvalidIndex(format!(
                "unsupported MIDX version {version}"
            )));
        }

        let oid_version = data[5];
        let hash_algo = match oid_version {
            1 => HashAlgorithm::Sha1,
            2 => HashAlgorithm::Sha256,
            _ => {
                return Err(PackError::InvalidIndex(format!(
                    "unsupported OID version {oid_version}"
                )))
            }
        };

        let num_chunks = data[6] as usize;
        let num_packs = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        // Parse chunk lookup table (starts at offset 12)
        let mut pos = 12;
        let mut pack_names_offset: Option<usize> = None;
        let mut fanout_offset: Option<usize> = None;
        let mut oid_offset: Option<usize> = None;
        let mut offsets_offset: Option<usize> = None;
        let mut large_offsets_offset: Option<usize> = None;

        for _ in 0..num_chunks {
            if pos + 12 > data.len() {
                return Err(PackError::InvalidIndex("truncated chunk table".into()));
            }
            let chunk_id = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let chunk_offset = u64::from_be_bytes([
                data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
                data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11],
            ]) as usize;
            pos += 12;

            match chunk_id {
                CHUNK_PACK_NAMES => pack_names_offset = Some(chunk_offset),
                CHUNK_OID_FANOUT => fanout_offset = Some(chunk_offset),
                CHUNK_OID_LOOKUP => oid_offset = Some(chunk_offset),
                CHUNK_OBJECT_OFFSETS => offsets_offset = Some(chunk_offset),
                CHUNK_LARGE_OFFSETS => large_offsets_offset = Some(chunk_offset),
                _ => {} // Unknown chunks are ignored
            }
        }

        // Skip terminator entry (zero chunk ID + end offset)
        // pos += 12;

        let fanout_offset = fanout_offset.ok_or_else(|| {
            PackError::InvalidIndex("missing OID fanout chunk".into())
        })?;
        let oid_offset = oid_offset.ok_or_else(|| {
            PackError::InvalidIndex("missing OID lookup chunk".into())
        })?;
        let offsets_offset = offsets_offset.ok_or_else(|| {
            PackError::InvalidIndex("missing object offsets chunk".into())
        })?;

        // Read number of objects from fanout[255]
        let fanout_last = fanout_offset + 255 * 4;
        if fanout_last + 4 > data.len() {
            return Err(PackError::InvalidIndex("truncated fanout table".into()));
        }
        let num_objects = u32::from_be_bytes([
            data[fanout_last],
            data[fanout_last + 1],
            data[fanout_last + 2],
            data[fanout_last + 3],
        ]);

        // Parse pack names
        let pack_names = if let Some(pn_offset) = pack_names_offset {
            parse_pack_names(&data[pn_offset..], fanout_offset.saturating_sub(pn_offset))
        } else {
            Vec::new()
        };

        Ok(Self {
            data,
            num_packs,
            num_objects,
            pack_names,
            hash_algo,
            fanout_offset,
            oid_offset,
            offsets_offset,
            large_offsets_offset,
            midx_path,
        })
    }

    /// Look up an OID, returning `(pack_index, offset)` if found.
    pub fn lookup(&self, oid: &ObjectId) -> Option<(u32, u64)> {
        let hash_len = self.hash_algo.digest_len();
        let (lo, hi) = self.fanout_range(oid.first_byte());
        if lo >= hi {
            return None;
        }

        let target = oid.as_bytes();
        let mut low = lo;
        let mut high = hi;

        while low < high {
            let mid = low + (high - low) / 2;
            let mid_oid = self.oid_bytes_at(mid, hash_len);
            match mid_oid.cmp(target) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => {
                    return Some(self.object_entry(mid as u32));
                }
            }
        }
        None
    }

    /// Number of objects in the MIDX.
    pub fn num_objects(&self) -> u32 {
        self.num_objects
    }

    /// Number of packs referenced by this MIDX.
    pub fn num_packs(&self) -> u32 {
        self.num_packs
    }

    /// Pack names referenced by this MIDX.
    pub fn pack_names(&self) -> &[String] {
        &self.pack_names
    }

    /// Iterate over all (OID, pack_index, offset) triples.
    pub fn iter(&self) -> MultiPackIndexIter<'_> {
        MultiPackIndexIter {
            midx: self,
            pos: 0,
        }
    }

    fn fanout_range(&self, first_byte: u8) -> (usize, usize) {
        let end = self.fanout_entry(first_byte) as usize;
        let start = if first_byte == 0 {
            0
        } else {
            self.fanout_entry(first_byte - 1) as usize
        };
        (start, end)
    }

    fn fanout_entry(&self, index: u8) -> u32 {
        let pos = self.fanout_offset + index as usize * 4;
        u32::from_be_bytes([
            self.data[pos],
            self.data[pos + 1],
            self.data[pos + 2],
            self.data[pos + 3],
        ])
    }

    fn oid_bytes_at(&self, index: usize, hash_len: usize) -> &[u8] {
        let start = self.oid_offset + index * hash_len;
        &self.data[start..start + hash_len]
    }

    fn object_entry(&self, index: u32) -> (u32, u64) {
        let pos = self.offsets_offset + index as usize * 8;
        let pack_index = u32::from_be_bytes([
            self.data[pos],
            self.data[pos + 1],
            self.data[pos + 2],
            self.data[pos + 3],
        ]);
        let offset_val = u32::from_be_bytes([
            self.data[pos + 4],
            self.data[pos + 5],
            self.data[pos + 6],
            self.data[pos + 7],
        ]);

        let offset = if offset_val & 0x8000_0000 != 0 {
            // Large offset
            if let Some(lo_offset) = self.large_offsets_offset {
                let idx = (offset_val & 0x7FFF_FFFF) as usize;
                let p = lo_offset + idx * 8;
                u64::from_be_bytes([
                    self.data[p], self.data[p + 1], self.data[p + 2], self.data[p + 3],
                    self.data[p + 4], self.data[p + 5], self.data[p + 6], self.data[p + 7],
                ])
            } else {
                offset_val as u64 // Shouldn't happen with well-formed MIDX
            }
        } else {
            offset_val as u64
        };

        (pack_index, offset)
    }
}

/// Iterator over MIDX entries.
pub struct MultiPackIndexIter<'a> {
    midx: &'a MultiPackIndex,
    pos: u32,
}

impl<'a> Iterator for MultiPackIndexIter<'a> {
    type Item = (ObjectId, u32, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.midx.num_objects {
            return None;
        }
        let hash_len = self.midx.hash_algo.digest_len();
        let oid_bytes = self.midx.oid_bytes_at(self.pos as usize, hash_len);
        let oid = ObjectId::from_bytes(oid_bytes, self.midx.hash_algo)
            .expect("valid OID in MIDX");
        let (pack_idx, offset) = self.midx.object_entry(self.pos);
        self.pos += 1;
        Some((oid, pack_idx, offset))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.midx.num_objects - self.pos) as usize;
        (remaining, Some(remaining))
    }
}

/// Parse null-terminated pack names from the PNAM chunk.
fn parse_pack_names(data: &[u8], max_len: usize) -> Vec<String> {
    let mut names = Vec::new();
    let mut pos = 0;
    let end = std::cmp::min(data.len(), max_len);

    while pos < end {
        // Find null terminator
        let name_end = data[pos..end].iter().position(|&b| b == 0);
        match name_end {
            Some(len) if len > 0 => {
                if let Ok(name) = std::str::from_utf8(&data[pos..pos + len]) {
                    names.push(name.to_string());
                }
                pos += len + 1;
            }
            _ => break,
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_hash::hasher::Hasher;

    /// Build a synthetic MIDX file for testing.
    fn build_test_midx(
        entries: &[(ObjectId, u32, u64)], // (oid, pack_index, offset)
        pack_names: &[&str],
    ) -> Vec<u8> {
        // Sort by OID
        let mut sorted: Vec<_> = entries.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let num_packs = pack_names.len();

        // Build pack names chunk
        let mut pnam_chunk = Vec::new();
        for name in pack_names {
            pnam_chunk.extend_from_slice(name.as_bytes());
            pnam_chunk.push(0); // null terminator
        }
        // Align to 4 bytes
        while pnam_chunk.len() % 4 != 0 {
            pnam_chunk.push(0);
        }

        // Build fanout
        let mut fanout_data = Vec::new();
        let mut fanout = [0u32; 256];
        for (oid, _, _) in &sorted {
            fanout[oid.first_byte() as usize] += 1;
        }
        for i in 1..256 {
            fanout[i] += fanout[i - 1];
        }
        for count in fanout {
            fanout_data.extend_from_slice(&count.to_be_bytes());
        }

        // Build OID lookup
        let mut oid_data = Vec::new();
        for (oid, _, _) in &sorted {
            oid_data.extend_from_slice(oid.as_bytes());
        }

        // Build object offsets (pack_index: u32, offset: u32)
        let mut offsets_data = Vec::new();
        for (_, pack_idx, offset) in &sorted {
            offsets_data.extend_from_slice(&pack_idx.to_be_bytes());
            offsets_data.extend_from_slice(&(*offset as u32).to_be_bytes());
        }

        // Calculate chunk offsets
        // Header: 12 bytes
        // Chunk table: 4 chunks × 12 bytes + 12 byte terminator = 60 bytes
        let num_chunks = 4u8;
        let header_size = 12;
        let chunk_table_size = (num_chunks as usize + 1) * 12;
        let chunks_start = header_size + chunk_table_size;

        let pnam_start = chunks_start;
        let fanout_start = pnam_start + pnam_chunk.len();
        let oid_start = fanout_start + fanout_data.len();
        let offsets_start = oid_start + oid_data.len();
        let end_offset = offsets_start + offsets_data.len();

        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(MIDX_SIGNATURE);
        buf.push(1); // version
        buf.push(1); // OID version (SHA-1)
        buf.push(num_chunks);
        buf.push(0); // reserved
        buf.extend_from_slice(&(num_packs as u32).to_be_bytes());

        // Chunk lookup table
        // PNAM
        buf.extend_from_slice(&CHUNK_PACK_NAMES.to_be_bytes());
        buf.extend_from_slice(&(pnam_start as u64).to_be_bytes());
        // OIDF
        buf.extend_from_slice(&CHUNK_OID_FANOUT.to_be_bytes());
        buf.extend_from_slice(&(fanout_start as u64).to_be_bytes());
        // OIDL
        buf.extend_from_slice(&CHUNK_OID_LOOKUP.to_be_bytes());
        buf.extend_from_slice(&(oid_start as u64).to_be_bytes());
        // OOFF
        buf.extend_from_slice(&CHUNK_OBJECT_OFFSETS.to_be_bytes());
        buf.extend_from_slice(&(offsets_start as u64).to_be_bytes());
        // Terminator
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&(end_offset as u64).to_be_bytes());

        // Chunk data
        buf.extend_from_slice(&pnam_chunk);
        buf.extend_from_slice(&fanout_data);
        buf.extend_from_slice(&oid_data);
        buf.extend_from_slice(&offsets_data);

        // Trailing checksum
        let mut hasher = Hasher::new(HashAlgorithm::Sha1);
        hasher.update(&buf);
        let checksum = hasher.finalize().unwrap();
        buf.extend_from_slice(checksum.as_bytes());

        buf
    }

    fn make_oid(first_byte: u8, suffix: u8) -> ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = first_byte;
        bytes[19] = suffix;
        ObjectId::from_bytes(&bytes, HashAlgorithm::Sha1).unwrap()
    }

    #[test]
    fn open_and_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let oid1 = make_oid(0x10, 0x01);
        let oid2 = make_oid(0x20, 0x02);

        let data = build_test_midx(
            &[(oid1, 0, 100), (oid2, 1, 200)],
            &["pack-aaa.pack", "pack-bbb.pack"],
        );

        let path = dir.path().join("multi-pack-index");
        std::fs::write(&path, &data).unwrap();

        let midx = MultiPackIndex::open(&path).unwrap();
        assert_eq!(midx.num_objects(), 2);
        assert_eq!(midx.num_packs(), 2);
        assert_eq!(midx.pack_names().len(), 2);

        // Lookup
        assert_eq!(midx.lookup(&oid1), Some((0, 100)));
        assert_eq!(midx.lookup(&oid2), Some((1, 200)));

        // Missing
        let missing = make_oid(0x99, 0x00);
        assert_eq!(midx.lookup(&missing), None);
    }

    #[test]
    fn iterate_all_entries() {
        let dir = tempfile::tempdir().unwrap();
        let entries = vec![
            (make_oid(0x01, 0x01), 0u32, 10u64),
            (make_oid(0x02, 0x01), 0, 20),
            (make_oid(0xff, 0x01), 1, 30),
        ];

        let data = build_test_midx(&entries, &["pack-a.pack", "pack-b.pack"]);
        let path = dir.path().join("multi-pack-index");
        std::fs::write(&path, &data).unwrap();

        let midx = MultiPackIndex::open(&path).unwrap();
        let items: Vec<_> = midx.iter().collect();
        assert_eq!(items.len(), 3);

        // Sorted order
        assert_eq!(items[0].0, make_oid(0x01, 0x01));
        assert_eq!(items[2].0, make_oid(0xff, 0x01));
    }
}

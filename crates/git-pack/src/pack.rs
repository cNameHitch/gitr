//! PackFile: reading .pack files.
//!
//! A pack file contains a header, a sequence of compressed objects
//! (possibly deltified), and a trailing checksum.

use std::path::{Path, PathBuf};

use flate2::bufread::ZlibDecoder;
use git_hash::{HashAlgorithm, ObjectId};
use git_object::ObjectType;
use memmap2::Mmap;
use std::io::Read;

use crate::entry::{parse_entry_header, PackEntry};
use crate::index::PackIndex;
use crate::{
    PackEntryType, PackError, PackedObject, PACK_HEADER_SIZE, PACK_SIGNATURE, PACK_VERSION,
    MAX_DELTA_CHAIN_DEPTH,
};

/// A memory-mapped packfile with its index.
pub struct PackFile {
    data: Mmap,
    index: PackIndex,
    pack_path: PathBuf,
    num_objects: u32,
    hash_algo: HashAlgorithm,
}

impl PackFile {
    /// Open a pack file and its associated index.
    ///
    /// Given a `.pack` file path, opens both the pack and its `.idx` file.
    pub fn open(pack_path: impl AsRef<Path>) -> Result<Self, PackError> {
        let pack_path = pack_path.as_ref().to_path_buf();

        // Derive .idx path from .pack path
        let idx_path = pack_path.with_extension("idx");

        let file = std::fs::File::open(&pack_path)?;
        let data = unsafe { Mmap::map(&file)? };

        // Validate pack header
        if data.len() < PACK_HEADER_SIZE {
            return Err(PackError::InvalidHeader("file too small".into()));
        }
        if &data[0..4] != PACK_SIGNATURE {
            return Err(PackError::InvalidHeader("bad PACK signature".into()));
        }
        let version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if version != PACK_VERSION {
            return Err(PackError::UnsupportedVersion(version));
        }
        let num_objects = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let index = PackIndex::open(&idx_path)?;

        // Validate object count matches between pack and index
        if index.num_objects() != num_objects {
            return Err(PackError::InvalidHeader(format!(
                "pack has {} objects but index has {}",
                num_objects,
                index.num_objects()
            )));
        }

        Ok(Self {
            data,
            index,
            pack_path,
            num_objects,
            hash_algo: HashAlgorithm::Sha1,
        })
    }

    /// Read an object by OID.
    ///
    /// Returns `None` if the OID is not in this pack.
    pub fn read_object(&self, oid: &ObjectId) -> Result<Option<PackedObject>, PackError> {
        match self.index.lookup(oid) {
            Some(offset) => self.read_at_offset(offset).map(Some),
            None => Ok(None),
        }
    }

    /// Read an object at a known offset in the pack.
    ///
    /// Resolves delta chains iteratively (not recursively) to handle
    /// arbitrary chain depths safely.
    pub fn read_at_offset(&self, offset: u64) -> Result<PackedObject, PackError> {
        self.read_at_offset_with_resolver(offset, |_| None)
    }

    /// Read an object by OID, with an external resolver for cross-pack REF_DELTA bases.
    ///
    /// The resolver is called when a REF_DELTA references a base OID not found in this pack.
    /// It should return the resolved base object's type and data if found externally.
    pub fn read_object_with_resolver(
        &self,
        oid: &ObjectId,
        resolver: impl Fn(&ObjectId) -> Option<(ObjectType, Vec<u8>)>,
    ) -> Result<Option<PackedObject>, PackError> {
        match self.index.lookup(oid) {
            Some(offset) => self.read_at_offset_with_resolver(offset, resolver).map(Some),
            None => Ok(None),
        }
    }

    /// Read an object at a known offset, with an external resolver for cross-pack REF_DELTA bases.
    fn read_at_offset_with_resolver(
        &self,
        offset: u64,
        resolver: impl Fn(&ObjectId) -> Option<(ObjectType, Vec<u8>)>,
    ) -> Result<PackedObject, PackError> {
        // Build the delta chain (innermost delta first, base last)
        let mut chain: Vec<(PackEntry, Vec<u8>)> = Vec::new();
        let mut current_offset = offset;

        for depth in 0..MAX_DELTA_CHAIN_DEPTH {
            let entry = parse_entry_header(
                &self.data[current_offset as usize..],
                current_offset,
            )?;

            // Decompress the data
            let compressed = &self.data[entry.data_offset as usize..];
            let decompressed = decompress(compressed, entry.uncompressed_size, current_offset)?;

            match entry.entry_type {
                PackEntryType::Commit
                | PackEntryType::Tree
                | PackEntryType::Blob
                | PackEntryType::Tag => {
                    // Base object — resolve chain
                    let obj_type = entry
                        .entry_type
                        .to_object_type()
                        .expect("non-delta type");

                    // Apply delta chain in reverse order
                    let mut data = decompressed;
                    for (_, delta_data) in chain.iter().rev() {
                        data = crate::delta::apply::apply_delta(&data, delta_data)?;
                    }

                    return Ok(PackedObject {
                        obj_type,
                        data,
                    });
                }
                PackEntryType::OfsDelta { base_offset } => {
                    chain.push((entry, decompressed));
                    current_offset = base_offset;
                }
                PackEntryType::RefDelta { base_oid } => {
                    chain.push((entry, decompressed));
                    // Try the index within this pack first
                    if let Some(base_offset) = self.index.lookup(&base_oid) {
                        current_offset = base_offset;
                    } else if let Some((obj_type, base_data)) = resolver(&base_oid) {
                        // External resolver found the base — apply delta chain
                        let mut data = base_data;
                        for (_, delta_data) in chain.iter().rev() {
                            data = crate::delta::apply::apply_delta(&data, delta_data)?;
                        }
                        return Ok(PackedObject { obj_type, data });
                    } else {
                        return Err(PackError::MissingBase(base_oid));
                    }
                }
            }

            if depth + 1 >= MAX_DELTA_CHAIN_DEPTH {
                return Err(PackError::DeltaChainTooDeep {
                    offset,
                    max_depth: MAX_DELTA_CHAIN_DEPTH,
                });
            }
        }

        Err(PackError::DeltaChainTooDeep {
            offset,
            max_depth: MAX_DELTA_CHAIN_DEPTH,
        })
    }

    /// Check if this pack contains the given OID.
    pub fn contains(&self, oid: &ObjectId) -> bool {
        self.index.lookup(oid).is_some()
    }

    /// Get the number of objects in this pack.
    pub fn num_objects(&self) -> u32 {
        self.num_objects
    }

    /// Get the pack index.
    pub fn index(&self) -> &PackIndex {
        &self.index
    }

    /// Get the path to the .pack file.
    pub fn path(&self) -> &Path {
        &self.pack_path
    }

    /// Get the raw memory-mapped pack data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the hash algorithm used by this pack.
    pub fn hash_algo(&self) -> HashAlgorithm {
        self.hash_algo
    }
}

/// Decompress zlib data with an expected uncompressed size.
fn decompress(compressed: &[u8], expected_size: usize, offset: u64) -> Result<Vec<u8>, PackError> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut buf = Vec::with_capacity(expected_size);
    decoder.read_to_end(&mut buf).map_err(|_| {
        PackError::CorruptEntry(offset)
    })?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::compute::compute_delta;
    use crate::entry::encode_entry_header;
    use git_object::ObjectType;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use git_hash::hasher::Hasher;
    use std::io::Write;

    /// Build a minimal valid .pack + .idx pair in a temp directory.
    /// Returns the path to the .pack file.
    fn build_test_pack(
        dir: &Path,
        objects: &[(ObjectType, &[u8])],
    ) -> (PathBuf, Vec<ObjectId>) {
        let pack_path = dir.join("test.pack");
        let idx_path = dir.join("test.idx");

        let mut pack_data = Vec::new();

        // Pack header
        pack_data.extend_from_slice(PACK_SIGNATURE);
        pack_data.extend_from_slice(&PACK_VERSION.to_be_bytes());
        pack_data.extend_from_slice(&(objects.len() as u32).to_be_bytes());

        // Track entries for index building: (oid, offset, crc32)
        let mut entries: Vec<(ObjectId, u64, u32)> = Vec::new();

        for (obj_type, content) in objects {
            let offset = pack_data.len() as u64;

            let type_num = match obj_type {
                ObjectType::Commit => 1,
                ObjectType::Tree => 2,
                ObjectType::Blob => 3,
                ObjectType::Tag => 4,
            };

            // Build the raw entry (header + compressed data)
            let header = encode_entry_header(type_num, content.len() as u64);
            let mut compressed = Vec::new();
            {
                let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
                encoder.write_all(content).unwrap();
                encoder.finish().unwrap();
            }

            // CRC32 of the raw entry bytes (header + compressed)
            let mut crc_hasher = crc32fast::Hasher::new();
            crc_hasher.update(&header);
            crc_hasher.update(&compressed);
            let crc = crc_hasher.finalize();

            // Compute OID
            let oid = git_hash::hasher::Hasher::hash_object(
                HashAlgorithm::Sha1,
                obj_type.as_bytes().iter().map(|&b| b as char).collect::<String>().as_str(),
                content,
            )
            .unwrap();

            pack_data.extend_from_slice(&header);
            pack_data.extend_from_slice(&compressed);

            entries.push((oid, offset, crc));
        }

        // Pack trailer: SHA-1 of all preceding content
        let pack_checksum = {
            let mut h = Hasher::new(HashAlgorithm::Sha1);
            h.update(&pack_data);
            h.finalize().unwrap()
        };
        pack_data.extend_from_slice(pack_checksum.as_bytes());

        // Write .pack
        std::fs::write(&pack_path, &pack_data).unwrap();

        // Build .idx (v2 format)
        let oids: Vec<ObjectId> = entries.iter().map(|(oid, _, _)| *oid).collect();
        let idx_data = build_test_idx(&entries, pack_checksum.as_bytes());
        std::fs::write(&idx_path, &idx_data).unwrap();

        (pack_path, oids)
    }

    /// Build a v2 .idx file from sorted entries.
    fn build_test_idx(entries: &[(ObjectId, u64, u32)], pack_checksum: &[u8]) -> Vec<u8> {
        use crate::{IDX_SIGNATURE, IDX_VERSION};

        // Sort by OID
        let mut sorted: Vec<_> = entries.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(&IDX_SIGNATURE);
        buf.extend_from_slice(&IDX_VERSION.to_be_bytes());

        // Fanout table
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

        // OIDs
        for (oid, _, _) in &sorted {
            buf.extend_from_slice(oid.as_bytes());
        }

        // CRC32
        for (_, _, crc) in &sorted {
            buf.extend_from_slice(&crc.to_be_bytes());
        }

        // 32-bit offsets
        for (_, offset, _) in &sorted {
            buf.extend_from_slice(&(*offset as u32).to_be_bytes());
        }

        // Pack checksum
        buf.extend_from_slice(pack_checksum);

        // Index checksum
        let idx_checksum = {
            let mut h = Hasher::new(HashAlgorithm::Sha1);
            h.update(&buf);
            h.finalize().unwrap()
        };
        buf.extend_from_slice(idx_checksum.as_bytes());

        buf
    }

    #[test]
    fn read_single_blob() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"Hello, packfile world!";
        let (pack_path, oids) = build_test_pack(dir.path(), &[(ObjectType::Blob, content)]);

        let pack = PackFile::open(&pack_path).unwrap();
        assert_eq!(pack.num_objects(), 1);

        let obj = pack.read_object(&oids[0]).unwrap().unwrap();
        assert_eq!(obj.obj_type, ObjectType::Blob);
        assert_eq!(obj.data, content);
    }

    #[test]
    fn read_multiple_objects() {
        let dir = tempfile::tempdir().unwrap();
        let objects = vec![
            (ObjectType::Blob, b"blob content".as_slice()),
            (ObjectType::Blob, b"another blob".as_slice()),
            (ObjectType::Commit, b"tree 0000000000000000000000000000000000000000\nauthor Test <test@test.com> 0 +0000\ncommitter Test <test@test.com> 0 +0000\n\ntest commit\n".as_slice()),
        ];
        let (pack_path, oids) = build_test_pack(dir.path(), &objects);

        let pack = PackFile::open(&pack_path).unwrap();
        assert_eq!(pack.num_objects(), 3);

        for (i, (obj_type, content)) in objects.iter().enumerate() {
            let obj = pack.read_object(&oids[i]).unwrap().unwrap();
            assert_eq!(obj.obj_type, *obj_type);
            assert_eq!(obj.data, *content);
        }
    }

    #[test]
    fn contains_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let (pack_path, oids) = build_test_pack(dir.path(), &[(ObjectType::Blob, b"test")]);

        let pack = PackFile::open(&pack_path).unwrap();
        assert!(pack.contains(&oids[0]));

        let missing = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();
        assert!(!pack.contains(&missing));
        assert_eq!(pack.read_object(&missing).unwrap(), None);
    }

    #[test]
    fn read_ofs_delta_object() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("test.pack");
        let idx_path = dir.path().join("test.idx");

        // Build a pack with a base blob and an OFS_DELTA
        let base_content = b"Hello, this is the base object content for delta testing!";

        // Base entry
        let base_header = encode_entry_header(3, base_content.len() as u64); // blob
        let mut base_compressed = Vec::new();
        {
            let mut enc = ZlibEncoder::new(&mut base_compressed, Compression::default());
            enc.write_all(base_content).unwrap();
            enc.finish().unwrap();
        }

        // Target content (modified version)
        let target_content = b"Hello, this is the modified object content for delta testing!";

        // Compute delta from base to target
        let delta_bytes = compute_delta(base_content, target_content);

        // OFS_DELTA entry
        let base_offset_in_pack = PACK_HEADER_SIZE; // base is right after header
        let delta_offset_in_pack = PACK_HEADER_SIZE + base_header.len() + base_compressed.len();
        let negative_offset = delta_offset_in_pack - base_offset_in_pack;

        let delta_header = encode_entry_header(6, delta_bytes.len() as u64); // OFS_DELTA
        let ofs_encoded = crate::entry::encode_ofs_delta_offset(negative_offset as u64);

        let mut delta_compressed = Vec::new();
        {
            let mut enc = ZlibEncoder::new(&mut delta_compressed, Compression::default());
            enc.write_all(&delta_bytes).unwrap();
            enc.finish().unwrap();
        }

        // Assemble pack
        let mut pack_data = Vec::new();
        pack_data.extend_from_slice(PACK_SIGNATURE);
        pack_data.extend_from_slice(&PACK_VERSION.to_be_bytes());
        pack_data.extend_from_slice(&2u32.to_be_bytes()); // 2 objects

        let base_entry_offset = pack_data.len() as u64;
        pack_data.extend_from_slice(&base_header);
        pack_data.extend_from_slice(&base_compressed);

        let delta_entry_offset = pack_data.len() as u64;
        pack_data.extend_from_slice(&delta_header);
        pack_data.extend_from_slice(&ofs_encoded);
        pack_data.extend_from_slice(&delta_compressed);

        // Pack checksum
        let pack_checksum = {
            let mut h = Hasher::new(HashAlgorithm::Sha1);
            h.update(&pack_data);
            h.finalize().unwrap()
        };
        pack_data.extend_from_slice(pack_checksum.as_bytes());

        std::fs::write(&pack_path, &pack_data).unwrap();

        // Compute OIDs
        let base_oid =
            Hasher::hash_object(HashAlgorithm::Sha1, "blob", base_content).unwrap();
        let target_oid =
            Hasher::hash_object(HashAlgorithm::Sha1, "blob", target_content).unwrap();

        // CRC32
        let base_crc = {
            let mut h = crc32fast::Hasher::new();
            h.update(&base_header);
            h.update(&base_compressed);
            h.finalize()
        };
        let delta_crc = {
            let mut h = crc32fast::Hasher::new();
            h.update(&delta_header);
            h.update(&ofs_encoded);
            h.update(&delta_compressed);
            h.finalize()
        };

        // Build and write index
        let idx_data = build_test_idx(
            &[
                (base_oid, base_entry_offset, base_crc),
                (target_oid, delta_entry_offset, delta_crc),
            ],
            pack_checksum.as_bytes(),
        );
        std::fs::write(&idx_path, &idx_data).unwrap();

        // Now read the delta object
        let pack = PackFile::open(&pack_path).unwrap();
        assert_eq!(pack.num_objects(), 2);

        let base_obj = pack.read_object(&base_oid).unwrap().unwrap();
        assert_eq!(base_obj.data, base_content.as_slice());

        let delta_obj = pack.read_object(&target_oid).unwrap().unwrap();
        assert_eq!(delta_obj.obj_type, ObjectType::Blob);
        assert_eq!(delta_obj.data, target_content.as_slice());
    }
}

//! Pack checksum and integrity verification.

use git_hash::hasher::Hasher;
use git_hash::ObjectId;

use crate::pack::PackFile;
use crate::PackError;

impl PackFile {
    /// Verify the pack file checksum.
    ///
    /// Computes SHA-1 over all pack content (excluding the trailing checksum)
    /// and compares it with the stored checksum.
    pub fn verify_checksum(&self) -> Result<(), PackError> {
        let data = self.data();
        let hash_len = self.hash_algo().digest_len();

        if data.len() < hash_len {
            return Err(PackError::InvalidHeader("pack too small for checksum".into()));
        }

        let content = &data[..data.len() - hash_len];
        let stored_checksum_bytes = &data[data.len() - hash_len..];

        let stored = ObjectId::from_bytes(stored_checksum_bytes, self.hash_algo())
            .map_err(|_| PackError::InvalidHeader("invalid checksum bytes".into()))?;

        let mut hasher = Hasher::new(self.hash_algo());
        hasher.update(content);
        let computed = hasher.finalize().map_err(PackError::Hash)?;

        if computed != stored {
            return Err(PackError::ChecksumMismatch {
                expected: stored,
                actual: computed,
            });
        }

        Ok(())
    }
}

/// Iterator over all objects in a pack file.
pub struct PackIter<'a> {
    pack: &'a PackFile,
    index_pos: u32,
}

impl PackFile {
    /// Iterate over all objects in the pack.
    ///
    /// Objects are yielded in index-sorted order (by OID).
    pub fn iter(&self) -> PackIter<'_> {
        PackIter {
            pack: self,
            index_pos: 0,
        }
    }
}

impl<'a> Iterator for PackIter<'a> {
    type Item = Result<(ObjectId, crate::PackedObject), PackError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index_pos >= self.pack.num_objects() {
            return None;
        }

        let oid = self.pack.index().oid_at_index(self.index_pos);
        let offset = self.pack.index().offset_at_index(self.index_pos);
        self.index_pos += 1;

        Some(self.pack.read_at_offset(offset).map(|obj| (oid, obj)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.pack.num_objects() - self.index_pos) as usize;
        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_pack() -> PackFile {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let pack_path = format!("{manifest_dir}/tests/fixtures/test.pack");
        PackFile::open(&pack_path).expect("failed to open fixture pack")
    }

    #[test]
    fn verify_c_git_pack_checksum() {
        let pack = fixture_pack();
        pack.verify_checksum().unwrap();
    }

    #[test]
    fn iterate_all_objects() {
        let pack = fixture_pack();
        let mut count = 0;
        for result in pack.iter() {
            let (oid, obj) = result.unwrap();
            assert!(!oid.is_null());
            assert!(!obj.data.is_empty() || obj.data.is_empty()); // just check no panic
            count += 1;
        }
        assert_eq!(count, 9);
    }

    #[test]
    fn verify_written_pack_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let objects = vec![
            (git_object::ObjectType::Blob, b"verify test".to_vec()),
        ];
        let (pack_path, _, _) =
            crate::write::create_pack(dir.path(), "verify", &objects).unwrap();

        let pack = PackFile::open(&pack_path).unwrap();
        pack.verify_checksum().unwrap();
    }
}

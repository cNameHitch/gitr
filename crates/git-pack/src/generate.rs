//! Pack generation from wants/haves OID sets.
//!
//! Given a set of "wanted" OIDs and "have" OIDs, generate a pack containing
//! the objects reachable from wants but not from haves. This is the core
//! routine used by push and fetch operations.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use git_hash::hasher::Hasher;
use git_hash::{HashAlgorithm, ObjectId};
use git_object::ObjectType;

use crate::entry::encode_entry_header;
use crate::{PACK_HEADER_SIZE, PACK_SIGNATURE, PACK_VERSION, PackError, PackedObject};

/// Trait for resolving objects by OID. Implemented by the object database.
pub trait ObjectResolver {
    /// Read an object by OID. Returns None if not found.
    fn read_object(&self, oid: &ObjectId) -> Result<Option<PackedObject>, PackError>;
}

/// Result of pack generation.
#[derive(Debug)]
pub struct PackGenerationResult {
    pub num_objects: u32,
    pub bytes_written: u64,
    pub checksum: ObjectId,
}

/// Generate a pack containing the given objects and write it to `output`.
///
/// `objects` is a list of `(OID, ObjectType, data)` tuples to include.
/// If `thin` is true and `known_remote_oids` is provided, deltas may
/// reference base objects from that set without including them.
///
/// This is a lower-level function. Higher-level callers (spec 006+) will
/// walk the object graph to determine which objects to send.
pub fn generate_pack(
    objects: &[(ObjectId, ObjectType, Vec<u8>)],
    output: &mut dyn Write,
) -> Result<PackGenerationResult, PackError> {
    if objects.is_empty() {
        // Empty pack: write nothing, succeed gracefully
        return Ok(PackGenerationResult {
            num_objects: 0,
            bytes_written: 0,
            checksum: ObjectId::NULL_SHA1,
        });
    }

    let mut hasher = Hasher::new(HashAlgorithm::Sha1);
    let mut total_bytes: u64 = 0;

    // Write header
    let mut header = [0u8; PACK_HEADER_SIZE];
    header[0..4].copy_from_slice(PACK_SIGNATURE);
    header[4..8].copy_from_slice(&PACK_VERSION.to_be_bytes());
    header[8..12].copy_from_slice(&(objects.len() as u32).to_be_bytes());

    output.write_all(&header)?;
    hasher.update(&header);
    total_bytes += header.len() as u64;

    // Write each object
    for (_oid, obj_type, data) in objects {
        let type_num = match obj_type {
            ObjectType::Commit => 1,
            ObjectType::Tree => 2,
            ObjectType::Blob => 3,
            ObjectType::Tag => 4,
        };

        let entry_header = encode_entry_header(type_num, data.len() as u64);
        output.write_all(&entry_header)?;
        hasher.update(&entry_header);
        total_bytes += entry_header.len() as u64;

        // Compress data
        let mut compressed = Vec::new();
        {
            let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
            encoder.write_all(data)?;
            encoder.finish()?;
        }

        output.write_all(&compressed)?;
        hasher.update(&compressed);
        total_bytes += compressed.len() as u64;
    }

    // Write checksum trailer
    let checksum = hasher.finalize().map_err(PackError::Hash)?;
    output.write_all(checksum.as_bytes())?;
    total_bytes += checksum.as_bytes().len() as u64;

    Ok(PackGenerationResult {
        num_objects: objects.len() as u32,
        bytes_written: total_bytes,
        checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_empty_pack() {
        let mut buf = Vec::new();
        let result = generate_pack(&[], &mut buf).unwrap();
        assert_eq!(result.num_objects, 0);
        assert_eq!(result.bytes_written, 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn generate_pack_with_objects() {
        let oid1 = Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"hello").unwrap();
        let oid2 = Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"world").unwrap();

        let objects = vec![
            (oid1, ObjectType::Blob, b"hello".to_vec()),
            (oid2, ObjectType::Blob, b"world".to_vec()),
        ];

        let mut buf = Vec::new();
        let result = generate_pack(&objects, &mut buf).unwrap();
        assert_eq!(result.num_objects, 2);
        assert!(result.bytes_written > 0);
        assert!(!buf.is_empty());

        // Verify pack header
        assert_eq!(&buf[0..4], b"PACK");
        let version = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        assert_eq!(version, 2);
        let num_objects = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        assert_eq!(num_objects, 2);
    }

    #[test]
    fn generated_pack_verifiable_by_c_git() {
        let oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", b"test content").unwrap();
        let objects = vec![(oid, ObjectType::Blob, b"test content".to_vec())];

        // Write to temp file
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("gen.pack");
        {
            let mut file = std::fs::File::create(&pack_path).unwrap();
            generate_pack(&objects, &mut file).unwrap();
        }

        // Use git index-pack to verify (creates the .idx and validates)
        let output = std::process::Command::new("git")
            .args(["index-pack"])
            .arg(&pack_path)
            .output()
            .expect("failed to run git index-pack");

        assert!(
            output.status.success(),
            "git index-pack failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

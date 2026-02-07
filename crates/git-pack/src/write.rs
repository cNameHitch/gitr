//! Pack generation: create .pack and .idx files.
//!
//! Provides `PackWriter` for creating new packfiles and
//! `build_pack_index` for generating .idx files from .pack files.

use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::write::ZlibEncoder;
use flate2::Compression;
use git_hash::hasher::Hasher;
use git_hash::{HashAlgorithm, ObjectId};
use git_object::ObjectType;

use crate::entry::encode_entry_header;
use crate::{IDX_SIGNATURE, IDX_VERSION, PACK_HEADER_SIZE, PACK_SIGNATURE, PACK_VERSION, PackError};

/// A written pack entry, used for index construction.
struct PackWriterEntry {
    oid: ObjectId,
    offset: u64,
    crc32: u32,
}

/// Builder for creating new packfiles.
pub struct PackWriter {
    file: std::fs::File,
    hasher: Hasher,
    num_objects: u32,
    entries: Vec<PackWriterEntry>,
    path: PathBuf,
    /// When true, allow delta bases that reference objects not in this pack.
    thin: bool,
    /// Current write position (byte offset).
    position: u64,
}

impl PackWriter {
    /// Create a new pack writer at the given path.
    ///
    /// Writes the pack header immediately; call `add_object` / `add_delta`
    /// to append entries, then `finish` to write the trailer.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, PackError> {
        let path = path.as_ref().to_path_buf();
        let mut file = std::fs::File::create(&path)?;
        let mut hasher = Hasher::new(HashAlgorithm::Sha1);

        // Write placeholder header (num_objects will be fixed in finish)
        let mut header = [0u8; PACK_HEADER_SIZE];
        header[0..4].copy_from_slice(PACK_SIGNATURE);
        header[4..8].copy_from_slice(&PACK_VERSION.to_be_bytes());
        header[8..12].copy_from_slice(&0u32.to_be_bytes()); // placeholder

        file.write_all(&header)?;
        hasher.update(&header);

        Ok(Self {
            file,
            hasher,
            num_objects: 0,
            entries: Vec::new(),
            path,
            thin: false,
            position: PACK_HEADER_SIZE as u64,
        })
    }

    /// Enable or disable thin pack mode.
    ///
    /// In thin pack mode, delta bases may reference objects not included
    /// in the pack. The receiver is expected to already have those objects.
    pub fn set_thin(&mut self, thin: bool) {
        self.thin = thin;
    }

    /// Add a full (non-delta) object to the pack.
    pub fn add_object(
        &mut self,
        obj_type: ObjectType,
        data: &[u8],
    ) -> Result<(), PackError> {
        let type_num = match obj_type {
            ObjectType::Commit => 1,
            ObjectType::Tree => 2,
            ObjectType::Blob => 3,
            ObjectType::Tag => 4,
        };

        let oid = Hasher::hash_object(
            HashAlgorithm::Sha1,
            std::str::from_utf8(obj_type.as_bytes()).unwrap(),
            data,
        )
        .map_err(PackError::Hash)?;

        let offset = self.position;
        let header = encode_entry_header(type_num, data.len() as u64);

        // Compress data
        let mut compressed = Vec::new();
        {
            let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
            encoder.write_all(data)?;
            encoder.finish()?;
        }

        // CRC32 of header + compressed data
        let mut crc = crc32fast::Hasher::new();
        crc.update(&header);
        crc.update(&compressed);
        let crc_val = crc.finalize();

        // Write to file and hasher
        self.write_bytes(&header)?;
        self.write_bytes(&compressed)?;

        self.entries.push(PackWriterEntry {
            oid,
            offset,
            crc32: crc_val,
        });
        self.num_objects += 1;

        Ok(())
    }

    /// Add a REF_DELTA entry referencing a base object by OID.
    pub fn add_delta(
        &mut self,
        base_oid: ObjectId,
        target_oid: ObjectId,
        delta_data: &[u8],
    ) -> Result<(), PackError> {
        let offset = self.position;
        let header = encode_entry_header(7, delta_data.len() as u64); // REF_DELTA

        // Compress delta data
        let mut compressed = Vec::new();
        {
            let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
            encoder.write_all(delta_data)?;
            encoder.finish()?;
        }

        // CRC32 of header + base_oid + compressed
        let mut crc = crc32fast::Hasher::new();
        crc.update(&header);
        crc.update(base_oid.as_bytes());
        crc.update(&compressed);
        let crc_val = crc.finalize();

        // Write to file and hasher
        self.write_bytes(&header)?;
        self.write_bytes(base_oid.as_bytes())?;
        self.write_bytes(&compressed)?;

        self.entries.push(PackWriterEntry {
            oid: target_oid,
            offset,
            crc32: crc_val,
        });
        self.num_objects += 1;

        Ok(())
    }

    /// Finish writing the pack: fix header, write checksum trailer.
    ///
    /// Returns the path to the .pack file and its checksum.
    pub fn finish(mut self) -> Result<(PathBuf, ObjectId), PackError> {
        // Fix the object count in the header
        use std::io::Seek;
        self.file.seek(std::io::SeekFrom::Start(0))?;

        let mut header = [0u8; PACK_HEADER_SIZE];
        header[0..4].copy_from_slice(PACK_SIGNATURE);
        header[4..8].copy_from_slice(&PACK_VERSION.to_be_bytes());
        header[8..12].copy_from_slice(&self.num_objects.to_be_bytes());

        self.file.write_all(&header)?;
        self.file.seek(std::io::SeekFrom::End(0))?;

        // Recompute hasher from scratch (since we modified the header)
        // Actually, let's fix the hasher — we stored the placeholder header
        // We need to recompute the hash. The simplest approach: re-read and hash.
        drop(self.file);

        // Read the pack file and compute proper checksum
        let pack_content = std::fs::read(&self.path)?;
        let mut hasher = Hasher::new(HashAlgorithm::Sha1);
        hasher.update(&pack_content);
        let checksum = hasher.finalize().map_err(PackError::Hash)?;

        // Append checksum to the file
        let mut file = std::fs::OpenOptions::new().append(true).open(&self.path)?;
        file.write_all(checksum.as_bytes())?;

        Ok((self.path.clone(), checksum))
    }

    /// Get the entries written so far (for index building).
    pub fn entries(&self) -> impl Iterator<Item = (&ObjectId, u64, u32)> {
        self.entries
            .iter()
            .map(|e| (&e.oid, e.offset, e.crc32))
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<(), PackError> {
        self.file.write_all(data)?;
        self.hasher.update(data);
        self.position += data.len() as u64;
        Ok(())
    }
}

/// Build a v2 pack index (.idx) from a list of (OID, offset, CRC32) entries
/// and a pack checksum. Writes the index to the given path.
pub fn build_pack_index(
    idx_path: &Path,
    entries: &mut [(ObjectId, u64, u32)],
    pack_checksum: &ObjectId,
) -> Result<PathBuf, PackError> {
    // Sort by OID
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(&IDX_SIGNATURE);
    buf.extend_from_slice(&IDX_VERSION.to_be_bytes());

    // Fanout table
    let mut fanout = [0u32; 256];
    for (oid, _, _) in entries.iter() {
        fanout[oid.first_byte() as usize] += 1;
    }
    for i in 1..256 {
        fanout[i] += fanout[i - 1];
    }
    for count in fanout {
        buf.extend_from_slice(&count.to_be_bytes());
    }

    // OIDs
    for (oid, _, _) in entries.iter() {
        buf.extend_from_slice(oid.as_bytes());
    }

    // CRC32
    for (_, _, crc) in entries.iter() {
        buf.extend_from_slice(&crc.to_be_bytes());
    }

    // Offsets — check if we need 64-bit table
    let mut large_offsets: Vec<u64> = Vec::new();
    for (_, offset, _) in entries.iter() {
        if *offset >= 0x8000_0000 {
            let idx = large_offsets.len() as u32;
            buf.extend_from_slice(&(0x8000_0000u32 | idx).to_be_bytes());
            large_offsets.push(*offset);
        } else {
            buf.extend_from_slice(&(*offset as u32).to_be_bytes());
        }
    }

    // 64-bit offset table
    for offset in &large_offsets {
        buf.extend_from_slice(&offset.to_be_bytes());
    }

    // Pack checksum
    buf.extend_from_slice(pack_checksum.as_bytes());

    // Index checksum
    let mut hasher = Hasher::new(HashAlgorithm::Sha1);
    hasher.update(&buf);
    let idx_checksum = hasher.finalize().map_err(PackError::Hash)?;
    buf.extend_from_slice(idx_checksum.as_bytes());

    let idx_path = idx_path.to_path_buf();
    std::fs::write(&idx_path, &buf)?;

    Ok(idx_path)
}

/// Convenience function: create a pack and its index from a set of objects.
///
/// Returns `(pack_path, idx_path, checksum)`.
pub fn create_pack(
    dir: &Path,
    name: &str,
    objects: &[(ObjectType, Vec<u8>)],
) -> Result<(PathBuf, PathBuf, ObjectId), PackError> {
    let pack_path = dir.join(format!("{name}.pack"));
    let idx_path = dir.join(format!("{name}.idx"));

    let mut writer = PackWriter::new(&pack_path)?;
    for (obj_type, data) in objects {
        writer.add_object(*obj_type, data)?;
    }

    // Collect entries before finishing
    let mut entries: Vec<(ObjectId, u64, u32)> = writer
        .entries()
        .map(|(oid, off, crc)| (*oid, off, crc))
        .collect();

    let (pack_path, checksum) = writer.finish()?;

    build_pack_index(&idx_path, &mut entries, &checksum)?;

    Ok((pack_path, idx_path, checksum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::PackFile;

    #[test]
    fn write_and_read_single_blob() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"test blob content";

        let (pack_path, _, _) =
            create_pack(dir.path(), "test", &[(ObjectType::Blob, content.to_vec())]).unwrap();

        // Read it back
        let pack = PackFile::open(&pack_path).unwrap();
        assert_eq!(pack.num_objects(), 1);

        let oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", content).unwrap();
        let obj = pack.read_object(&oid).unwrap().unwrap();
        assert_eq!(obj.obj_type, ObjectType::Blob);
        assert_eq!(obj.data, content);
    }

    #[test]
    fn write_multiple_object_types() {
        let dir = tempfile::tempdir().unwrap();
        let objects = vec![
            (ObjectType::Blob, b"blob data".to_vec()),
            (ObjectType::Blob, b"another blob".to_vec()),
        ];

        let (pack_path, _, _) = create_pack(dir.path(), "multi", &objects).unwrap();
        let pack = PackFile::open(&pack_path).unwrap();
        assert_eq!(pack.num_objects(), 2);

        for (obj_type, data) in &objects {
            let oid = Hasher::hash_object(
                HashAlgorithm::Sha1,
                std::str::from_utf8(obj_type.as_bytes()).unwrap(),
                data,
            )
            .unwrap();
            let obj = pack.read_object(&oid).unwrap().unwrap();
            assert_eq!(obj.data, *data);
        }
    }

    #[test]
    fn roundtrip_with_delta() {
        let dir = tempfile::tempdir().unwrap();
        let pack_path = dir.path().join("delta.pack");
        let idx_path = dir.path().join("delta.idx");

        let base_content = b"Hello, this is the base content for our delta test!";
        let target_content = b"Hello, this is the modified content for our delta test!";

        let mut writer = PackWriter::new(&pack_path).unwrap();

        // Add base object
        writer.add_object(ObjectType::Blob, base_content).unwrap();

        // Compute and add delta
        let base_oid = Hasher::hash_object(HashAlgorithm::Sha1, "blob", base_content).unwrap();
        let target_oid =
            Hasher::hash_object(HashAlgorithm::Sha1, "blob", target_content).unwrap();
        let delta = crate::delta::compute::compute_delta(base_content, target_content);
        writer
            .add_delta(base_oid, target_oid, &delta)
            .unwrap();

        let mut entries: Vec<(ObjectId, u64, u32)> = writer
            .entries()
            .map(|(oid, off, crc)| (*oid, off, crc))
            .collect();
        let (_, checksum) = writer.finish().unwrap();
        build_pack_index(&idx_path, &mut entries, &checksum).unwrap();

        // Read back
        let pack = PackFile::open(&pack_path).unwrap();
        let base_obj = pack.read_object(&base_oid).unwrap().unwrap();
        assert_eq!(base_obj.data, base_content.as_slice());

        let target_obj = pack.read_object(&target_oid).unwrap().unwrap();
        assert_eq!(target_obj.data, target_content.as_slice());
    }

    #[test]
    fn verify_with_c_git() {
        let dir = tempfile::tempdir().unwrap();
        let objects = vec![
            (ObjectType::Blob, b"test content for verify".to_vec()),
            (ObjectType::Blob, b"another test object".to_vec()),
        ];

        let (pack_path, _, _) = create_pack(dir.path(), "verify", &objects).unwrap();

        // Run git verify-pack on our generated pack
        let output = std::process::Command::new("git")
            .args(["verify-pack", "-v"])
            .arg(&pack_path)
            .output()
            .expect("failed to run git verify-pack");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "git verify-pack failed:\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
}

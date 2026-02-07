use std::fs;
use std::io::Write;
use std::path::Path;

use flate2::write::ZlibEncoder;
use git_hash::hasher::Hasher;
use git_hash::ObjectId;
use git_object::header;
use git_object::{Object, ObjectType};

use crate::{LooseError, LooseObjectStore};

impl LooseObjectStore {
    /// Write an object to the loose store. Returns the OID.
    ///
    /// No-op if the object already exists (idempotent).
    /// The file is written atomically (temp file + rename).
    pub fn write(&self, obj: &Object) -> Result<ObjectId, LooseError> {
        let content = obj.serialize_content();
        self.write_raw(obj.object_type(), &content)
    }

    /// Write raw bytes with a known type. Returns the OID.
    ///
    /// No-op if the object already exists (idempotent).
    pub fn write_raw(
        &self,
        obj_type: ObjectType,
        content: &[u8],
    ) -> Result<ObjectId, LooseError> {
        let hdr = header::write_header(obj_type, content.len());

        // Compute the OID from uncompressed header + content.
        let oid = {
            let mut hasher = Hasher::new(self.hash_algo);
            hasher.update(&hdr);
            hasher.update(content);
            hasher.finalize()?
        };

        // Skip if object already exists.
        if self.contains(&oid) {
            return Ok(oid);
        }

        // Ensure the fan-out directory exists.
        let final_path = self.object_path(&oid);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to temp file in the objects dir (same filesystem for atomic rename).
        let tmp_path = write_to_temp(&self.objects_dir, &hdr, content, self.compression_level)?;

        // Atomic move to final location.
        finalize_object(&tmp_path, &final_path)?;

        Ok(oid)
    }

    /// Write from a stream with known type and size. Returns the OID.
    pub fn write_stream(
        &self,
        obj_type: ObjectType,
        size: usize,
        reader: &mut dyn std::io::Read,
    ) -> Result<ObjectId, LooseError> {
        let mut content = Vec::with_capacity(size);
        reader.read_to_end(&mut content)?;

        if content.len() != size {
            return Err(LooseError::Corrupt {
                oid: String::new(),
                reason: format!(
                    "stream size mismatch: declared {}, got {}",
                    size,
                    content.len()
                ),
            });
        }

        self.write_raw(obj_type, &content)
    }
}

/// Compress header + content into a temp file under `objects_dir`.
fn write_to_temp(
    objects_dir: &Path,
    hdr: &[u8],
    content: &[u8],
    level: flate2::Compression,
) -> Result<std::path::PathBuf, LooseError> {
    let tmp_path = objects_dir.join(format!(
        "tmp_obj_{}",
        std::process::id()
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
    ));

    let file = fs::File::create(&tmp_path)?;
    let mut encoder = ZlibEncoder::new(file, level);
    encoder.write_all(hdr)?;
    encoder.write_all(content)?;
    encoder.finish()?;

    // Set read-only permissions (0444) on Unix, matching C git.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o444))?;
    }

    Ok(tmp_path)
}

/// Atomically move a temp file to its final destination.
///
/// If the destination already exists (race with another writer), the temp file
/// is removed and the write is treated as successful (content-addressed idempotency).
fn finalize_object(tmp: &Path, final_path: &Path) -> Result<(), LooseError> {
    match fs::rename(tmp, final_path) {
        Ok(()) => Ok(()),
        Err(_) if final_path.exists() => {
            // Another writer won the race — clean up our temp file.
            let _ = fs::remove_file(tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(tmp);
            Err(LooseError::Io(e))
        }
    }
}

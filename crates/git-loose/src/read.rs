use std::fs;
use std::io::Read;

use flate2::read::ZlibDecoder;
use git_hash::hasher::Hasher;
use git_object::header;
use git_object::{Object, ObjectType};

use crate::{LooseError, LooseObjectStore};

impl LooseObjectStore {
    /// Check if a loose object exists.
    pub fn contains(&self, oid: &git_hash::ObjectId) -> bool {
        self.object_path(oid).is_file()
    }

    /// Read a loose object by OID.
    ///
    /// Returns `Ok(None)` if the object does not exist.
    /// Returns `Err` if the object exists but is corrupt.
    pub fn read(&self, oid: &git_hash::ObjectId) -> Result<Option<Object>, LooseError> {
        let path = self.object_path(oid);
        let compressed = match fs::read(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LooseError::Io(e)),
        };

        let decompressed = decompress_all(&compressed, oid)?;
        let obj = Object::parse(&decompressed)?;
        Ok(Some(obj))
    }

    /// Read just the header (type + size) without decompressing the full content.
    ///
    /// Returns `Ok(None)` if the object does not exist.
    pub fn read_header(
        &self,
        oid: &git_hash::ObjectId,
    ) -> Result<Option<(ObjectType, usize)>, LooseError> {
        let path = self.object_path(oid);
        let compressed = match fs::read(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LooseError::Io(e)),
        };

        // Decompress just enough to read the header (type + size + null byte).
        // Headers are typically < 32 bytes, so 64 is plenty of room.
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut buf = [0u8; 64];
        let mut filled = 0;

        loop {
            if filled >= buf.len() {
                return Err(LooseError::Corrupt {
                    oid: oid.to_hex(),
                    reason: "header exceeds 64 bytes".into(),
                });
            }
            let n = decoder.read(&mut buf[filled..]).map_err(|e| {
                LooseError::Decompress {
                    oid: oid.to_hex(),
                    source: e,
                }
            })?;
            if n == 0 {
                return Err(LooseError::Corrupt {
                    oid: oid.to_hex(),
                    reason: "unexpected EOF before header null terminator".into(),
                });
            }
            filled += n;
            if buf[..filled].contains(&0) {
                break;
            }
        }

        let (obj_type, content_size, _header_len) = header::parse_header(&buf[..filled])?;
        Ok(Some((obj_type, content_size)))
    }

    /// Read a loose object and verify its hash matches the expected OID.
    ///
    /// Returns `Ok(None)` if the object does not exist.
    pub fn read_verified(&self, oid: &git_hash::ObjectId) -> Result<Option<Object>, LooseError> {
        let path = self.object_path(oid);
        let compressed = match fs::read(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LooseError::Io(e)),
        };

        let decompressed = decompress_all(&compressed, oid)?;

        // Verify hash of the raw decompressed data (header + content).
        let actual_oid = Hasher::digest(self.hash_algo, &decompressed)?;
        if actual_oid != *oid {
            return Err(LooseError::HashMismatch {
                path,
                expected: oid.to_hex(),
                actual: actual_oid.to_hex(),
            });
        }

        let obj = Object::parse(&decompressed)?;
        Ok(Some(obj))
    }
}

/// Zlib-decompress the full contents of a loose object file.
fn decompress_all(
    compressed: &[u8],
    oid: &git_hash::ObjectId,
) -> Result<Vec<u8>, LooseError> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|e| {
        LooseError::Decompress {
            oid: oid.to_hex(),
            source: e,
        }
    })?;
    Ok(decompressed)
}

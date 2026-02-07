//! Pluggable object storage backend trait.

use git_hash::ObjectId;
use git_object::{Object, ObjectType};

use crate::OdbError;

/// Trait for pluggable object storage backends.
///
/// Implementations provide access to objects in a specific storage format
/// (loose files, packfiles, etc.).
pub trait OdbBackend: Send + Sync {
    /// Read an object by OID.
    fn read(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError>;

    /// Read just the header (type + size).
    fn read_header(&self, oid: &ObjectId) -> Result<Option<(ObjectType, usize)>, OdbError>;

    /// Check if an object exists.
    fn contains(&self, oid: &ObjectId) -> bool;

    /// Write an object, returning its OID.
    fn write(&self, obj: &Object) -> Result<ObjectId, OdbError>;

    /// Find all OIDs matching the given hex prefix.
    fn lookup_prefix(&self, prefix: &str) -> Result<Vec<ObjectId>, OdbError>;
}

/// OdbBackend implementation for loose object storage.
impl OdbBackend for git_loose::LooseObjectStore {
    fn read(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError> {
        Ok(self.read(oid)?)
    }

    fn read_header(&self, oid: &ObjectId) -> Result<Option<(ObjectType, usize)>, OdbError> {
        Ok(self.read_header(oid)?)
    }

    fn contains(&self, oid: &ObjectId) -> bool {
        self.contains(oid)
    }

    fn write(&self, obj: &Object) -> Result<ObjectId, OdbError> {
        Ok(self.write(obj)?)
    }

    fn lookup_prefix(&self, prefix: &str) -> Result<Vec<ObjectId>, OdbError> {
        // For loose objects, iterate the fan-out directory matching the prefix
        let mut matches = Vec::new();
        if prefix.len() < 2 {
            // Need at least 2 hex chars for the fan-out directory
            // Iterate all objects and check prefix
            if let Ok(iter) = self.iter() {
                for result in iter {
                    let oid = result?;
                    if oid.starts_with_hex(prefix) {
                        matches.push(oid);
                    }
                }
            }
        } else {
            // Optimised: only iterate the matching fan-out directory
            if let Ok(iter) = self.iter() {
                for result in iter {
                    let oid = result?;
                    if oid.starts_with_hex(prefix) {
                        matches.push(oid);
                    }
                }
            }
        }
        Ok(matches)
    }
}

/// OdbBackend implementation for a single pack file.
impl OdbBackend for git_pack::pack::PackFile {
    fn read(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError> {
        match self.read_object(oid)? {
            Some(packed) => {
                let obj = Object::parse_content(packed.obj_type, &packed.data)
                    .map_err(|e| OdbError::Corrupt {
                        oid: *oid,
                        reason: e.to_string(),
                    })?;
                Ok(Some(obj))
            }
            None => Ok(None),
        }
    }

    fn read_header(&self, oid: &ObjectId) -> Result<Option<(ObjectType, usize)>, OdbError> {
        // For packs, we need to read the object to get the header info
        // The pack index only stores the offset, not type/size independently
        match self.read_object(oid)? {
            Some(packed) => Ok(Some((packed.obj_type, packed.data.len()))),
            None => Ok(None),
        }
    }

    fn contains(&self, oid: &ObjectId) -> bool {
        self.contains(oid)
    }

    fn write(&self, _obj: &Object) -> Result<ObjectId, OdbError> {
        // Pack files are read-only; writes go to loose storage
        Err(OdbError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "cannot write to pack files directly",
        )))
    }

    fn lookup_prefix(&self, prefix: &str) -> Result<Vec<ObjectId>, OdbError> {
        // Convert hex prefix to bytes for the pack index lookup
        let prefix_bytes = hex_prefix_to_bytes(prefix);
        let results = self.index().lookup_prefix(&prefix_bytes);
        Ok(results.into_iter().map(|(oid, _offset)| oid).collect())
    }
}

/// Convert a hex prefix string to raw bytes for pack index prefix lookup.
///
/// For even-length prefixes, this is a straightforward hex decode.
/// For odd-length prefixes, the last nibble is padded with 0.
pub(crate) fn hex_prefix_to_bytes(hex: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(hex.len().div_ceil(2));
    let chars: Vec<u8> = hex
        .bytes()
        .map(|b| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => 0,
        })
        .collect();

    for chunk in chars.chunks(2) {
        if chunk.len() == 2 {
            bytes.push((chunk[0] << 4) | chunk[1]);
        } else {
            bytes.push(chunk[0] << 4);
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_prefix_to_bytes_even() {
        assert_eq!(hex_prefix_to_bytes("abcd"), vec![0xab, 0xcd]);
    }

    #[test]
    fn hex_prefix_to_bytes_odd() {
        assert_eq!(hex_prefix_to_bytes("abc"), vec![0xab, 0xc0]);
    }

    #[test]
    fn hex_prefix_to_bytes_empty() {
        assert_eq!(hex_prefix_to_bytes(""), Vec::<u8>::new());
    }

    #[test]
    fn hex_prefix_to_bytes_single() {
        assert_eq!(hex_prefix_to_bytes("a"), vec![0xa0]);
    }
}

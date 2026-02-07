//! Resolve-undo extension (REUC).
//!
//! Stores the original pre-conflict versions of files so that a conflict
//! resolution can be undone with `git checkout -m`.

use bstr::BString;
use git_hash::ObjectId;
use git_object::FileMode;

use crate::extensions::{ResolveUndo, ResolveUndoEntry};
use crate::IndexError;

impl ResolveUndo {
    /// Extension signature.
    pub const SIGNATURE: &'static [u8; 4] = b"REUC";

    /// Parse a REUC extension from raw data.
    pub fn parse(data: &[u8]) -> Result<Self, IndexError> {
        let mut entries = Vec::new();
        let mut cursor = 0;

        while cursor < data.len() {
            // Read NUL-terminated path
            let nul_pos = data[cursor..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| IndexError::InvalidExtension {
                    sig: "REUC".into(),
                    reason: "missing NUL in path".into(),
                })?;
            let path = BString::from(&data[cursor..cursor + nul_pos]);
            cursor += nul_pos + 1;

            // Read 3 modes as octal ASCII, each NUL-terminated
            let mut modes = [None; 3];
            for mode in &mut modes {
                let nul_pos = data[cursor..]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or_else(|| IndexError::InvalidExtension {
                        sig: "REUC".into(),
                        reason: "missing NUL in mode".into(),
                    })?;
                let mode_str = std::str::from_utf8(&data[cursor..cursor + nul_pos])
                    .map_err(|_| IndexError::InvalidExtension {
                        sig: "REUC".into(),
                        reason: "invalid mode encoding".into(),
                    })?;
                let raw = u32::from_str_radix(mode_str, 8).map_err(|_| {
                    IndexError::InvalidExtension {
                        sig: "REUC".into(),
                        reason: format!("invalid mode: {mode_str}"),
                    }
                })?;
                if raw != 0 {
                    *mode = Some(FileMode::from_raw(raw));
                }
                cursor += nul_pos + 1;
            }

            // Read OIDs for non-zero modes (20 bytes each for SHA-1)
            let mut oids = [None; 3];
            for (i, oid_slot) in oids.iter_mut().enumerate() {
                if modes[i].is_some() {
                    if cursor + 20 > data.len() {
                        return Err(IndexError::InvalidExtension {
                            sig: "REUC".into(),
                            reason: "truncated OID".into(),
                        });
                    }
                    let oid = ObjectId::from_bytes(
                        &data[cursor..cursor + 20],
                        git_hash::HashAlgorithm::Sha1,
                    )
                    .map_err(|_| IndexError::InvalidExtension {
                        sig: "REUC".into(),
                        reason: "invalid OID".into(),
                    })?;
                    *oid_slot = Some(oid);
                    cursor += 20;
                }
            }

            entries.push(ResolveUndoEntry { path, modes, oids });
        }

        Ok(ResolveUndo { entries })
    }

    /// Serialize to raw bytes for writing.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        for entry in &self.entries {
            // Path + NUL
            buf.extend_from_slice(&entry.path);
            buf.push(0);

            // 3 modes as octal ASCII + NUL
            for mode in &entry.modes {
                let raw = mode.map(|m| m.raw()).unwrap_or(0);
                buf.extend_from_slice(format!("{raw:o}").as_bytes());
                buf.push(0);
            }

            // OIDs for non-zero modes
            for (i, oid) in entry.oids.iter().enumerate() {
                if entry.modes[i].is_some() {
                    if let Some(ref oid) = oid {
                        buf.extend_from_slice(oid.as_bytes());
                    }
                }
            }
        }

        buf
    }
}

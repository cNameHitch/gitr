use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use bstr::ByteSlice;
use git_hash::ObjectId;
use git_utils::lockfile::LockFile;

use crate::error::RefError;
use crate::name::RefName;

/// A single entry in the packed-refs file.
#[derive(Debug, Clone)]
pub struct PackedRef {
    pub name: RefName,
    pub oid: ObjectId,
    pub peeled: Option<ObjectId>,
}

/// Parsed packed-refs file.
///
/// The packed-refs file contains refs that have been "packed" from loose files
/// into a single file for efficiency. Format:
/// ```text
/// # pack-refs with: peeled fully-peeled sorted
/// <hex-oid> <refname>
/// ^<hex-oid>   (peeled value of annotated tag above)
/// ```
#[derive(Debug, Clone)]
pub struct PackedRefs {
    pub(crate) refs: Vec<PackedRef>,
    pub(crate) sorted: bool,
}

impl PackedRefs {
    /// Parse a packed-refs file.
    pub fn parse(data: &[u8]) -> Result<Self, RefError> {
        let mut refs = Vec::new();
        let mut sorted = false;

        for line in data.lines() {
            if line.is_empty() {
                continue;
            }

            // Header comment
            if line.starts_with(b"#") {
                if line.find(b"sorted").is_some() {
                    sorted = true;
                }
                continue;
            }

            // Peeled line: ^<hex-oid>
            if line.starts_with(b"^") {
                let hex = std::str::from_utf8(&line[1..]).map_err(|_| {
                    RefError::Parse("invalid UTF-8 in peeled OID".into())
                })?;
                let peeled_oid = ObjectId::from_hex(hex.trim())?;
                if let Some(last) = refs.last_mut() {
                    let pr: &mut PackedRef = last;
                    pr.peeled = Some(peeled_oid);
                }
                continue;
            }

            // Normal line: <hex-oid> <refname>
            let space_pos = line
                .find_byte(b' ')
                .ok_or_else(|| RefError::Parse("invalid packed-refs line".into()))?;

            let hex =
                std::str::from_utf8(&line[..space_pos]).map_err(|_| {
                    RefError::Parse("invalid UTF-8 in packed-refs OID".into())
                })?;
            let oid = ObjectId::from_hex(hex)?;

            let name_bytes = &line[space_pos + 1..];
            let name_str = std::str::from_utf8(name_bytes).map_err(|_| {
                RefError::Parse("invalid UTF-8 in packed-refs name".into())
            })?;
            let name = RefName::new(name_str.trim())?;

            refs.push(PackedRef {
                name,
                oid,
                peeled: None,
            });
        }

        Ok(Self { refs, sorted })
    }

    /// Load packed-refs from disk. Returns empty if file doesn't exist.
    pub fn load(git_dir: &Path) -> Result<Self, RefError> {
        let path = packed_refs_path(git_dir);
        if !path.exists() {
            return Ok(Self {
                refs: Vec::new(),
                sorted: true,
            });
        }

        let data = fs::read(&path).map_err(|e| RefError::IoPath {
            path: path.clone(),
            source: e,
        })?;
        Self::parse(&data)
    }

    /// Look up a ref by name using binary search (if sorted) or linear scan.
    pub fn find(&self, name: &RefName) -> Option<&PackedRef> {
        if self.sorted {
            self.refs
                .binary_search_by(|pr| pr.name.cmp(name))
                .ok()
                .map(|idx| &self.refs[idx])
        } else {
            self.refs.iter().find(|pr| pr.name == *name)
        }
    }

    /// Write the packed-refs file atomically using a lock file.
    pub fn write(&self, git_dir: &Path) -> Result<(), RefError> {
        let path = packed_refs_path(git_dir);
        let mut lock = LockFile::acquire(&path)?;

        // Write header
        lock.write_all(b"# pack-refs with: peeled fully-peeled sorted \n")
            .map_err(|e| RefError::IoPath {
                path: path.clone(),
                source: e,
            })?;

        // Write refs (must be sorted for binary search)
        let mut sorted_refs = self.refs.clone();
        sorted_refs.sort_by(|a, b| a.name.cmp(&b.name));

        for pr in &sorted_refs {
            let line = format!("{} {}\n", pr.oid.to_hex(), pr.name);
            lock.write_all(line.as_bytes()).map_err(|e| RefError::IoPath {
                path: path.clone(),
                source: e,
            })?;
            if let Some(peeled) = &pr.peeled {
                let peeled_line = format!("^{}\n", peeled.to_hex());
                lock.write_all(peeled_line.as_bytes())
                    .map_err(|e| RefError::IoPath {
                        path: path.clone(),
                        source: e,
                    })?;
            }
        }

        lock.commit()?;
        Ok(())
    }

    /// Add or update a ref in the packed-refs.
    pub fn upsert(&mut self, name: RefName, oid: ObjectId, peeled: Option<ObjectId>) {
        if let Some(existing) = self.refs.iter_mut().find(|pr| pr.name == name) {
            existing.oid = oid;
            existing.peeled = peeled;
        } else {
            self.refs.push(PackedRef {
                name,
                oid,
                peeled,
            });
            // Re-sort for binary search
            self.refs.sort_by(|a, b| a.name.cmp(&b.name));
            self.sorted = true;
        }
    }

    /// Remove a ref from packed-refs.
    pub fn remove(&mut self, name: &RefName) -> bool {
        let len_before = self.refs.len();
        self.refs.retain(|pr| pr.name != *name);
        self.refs.len() < len_before
    }

    /// Get all refs.
    pub fn refs(&self) -> &[PackedRef] {
        &self.refs
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// Get the path to the packed-refs file.
fn packed_refs_path(git_dir: &Path) -> PathBuf {
    git_dir.join("packed-refs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let packed = PackedRefs::parse(b"").unwrap();
        assert!(packed.refs.is_empty());
    }

    #[test]
    fn parse_with_header() {
        let data = b"# pack-refs with: peeled fully-peeled sorted \n\
                     da39a3ee5e6b4b0d3255bfef95601890afd80709 refs/heads/main\n";
        let packed = PackedRefs::parse(data).unwrap();
        assert!(packed.sorted);
        assert_eq!(packed.refs.len(), 1);
        assert_eq!(packed.refs[0].name.as_str(), "refs/heads/main");
    }

    #[test]
    fn parse_with_peeled() {
        let data = b"# pack-refs with: peeled fully-peeled sorted \n\
                     da39a3ee5e6b4b0d3255bfef95601890afd80709 refs/tags/v1.0\n\
                     ^aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
        let packed = PackedRefs::parse(data).unwrap();
        assert_eq!(packed.refs.len(), 1);
        assert!(packed.refs[0].peeled.is_some());
        assert_eq!(
            packed.refs[0].peeled.unwrap(),
            ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
        );
    }

    #[test]
    fn find_sorted() {
        let data = b"# pack-refs with: peeled fully-peeled sorted \n\
                     aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/heads/alpha\n\
                     bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/heads/beta\n\
                     cccccccccccccccccccccccccccccccccccccccc refs/tags/v1.0\n";
        let packed = PackedRefs::parse(data).unwrap();

        let name = RefName::new("refs/heads/beta").unwrap();
        let found = packed.find(&name).unwrap();
        assert_eq!(
            found.oid,
            ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap()
        );

        let name = RefName::new("refs/heads/nonexistent").unwrap();
        assert!(packed.find(&name).is_none());
    }

    #[test]
    fn upsert_and_remove() {
        let mut packed = PackedRefs {
            refs: Vec::new(),
            sorted: true,
        };

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        packed.upsert(name.clone(), oid, None);
        assert_eq!(packed.refs.len(), 1);

        // Update existing
        let new_oid = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        packed.upsert(name.clone(), new_oid, None);
        assert_eq!(packed.refs.len(), 1);
        assert_eq!(packed.refs[0].oid, new_oid);

        // Remove
        assert!(packed.remove(&name));
        assert!(packed.is_empty());
    }

    #[test]
    fn write_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let mut packed = PackedRefs {
            refs: Vec::new(),
            sorted: true,
        };

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        packed.upsert(name, oid, None);

        let name2 = RefName::new("refs/tags/v1.0").unwrap();
        let oid2 = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let peeled = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        packed.upsert(name2, oid2, Some(peeled));

        packed.write(git_dir).unwrap();

        // Reload and verify
        let loaded = PackedRefs::load(git_dir).unwrap();
        assert_eq!(loaded.refs.len(), 2);
        assert!(loaded.sorted);

        let main = loaded
            .find(&RefName::new("refs/heads/main").unwrap())
            .unwrap();
        assert_eq!(main.oid, oid);

        let tag = loaded
            .find(&RefName::new("refs/tags/v1.0").unwrap())
            .unwrap();
        assert_eq!(tag.oid, oid2);
        assert_eq!(tag.peeled, Some(peeled));
    }
}

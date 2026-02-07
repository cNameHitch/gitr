use std::cmp::Ordering;

use bstr::{BStr, BString, ByteSlice};
use git_hash::{HashAlgorithm, ObjectId};

use crate::ObjectError;

/// File mode for tree entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileMode {
    /// Regular file (100644)
    Regular,
    /// Executable file (100755)
    Executable,
    /// Symbolic link (120000)
    Symlink,
    /// Git submodule link (160000)
    Gitlink,
    /// Subdirectory (040000)
    Tree,
    /// Unknown mode (preserved for round-trip)
    Unknown(u32),
}

impl FileMode {
    /// Parse from octal ASCII bytes (e.g., `b"100644"`).
    pub fn from_bytes(s: &[u8]) -> Result<Self, ObjectError> {
        let raw = parse_octal(s)
            .ok_or_else(|| ObjectError::InvalidFileMode(String::from_utf8_lossy(s).into()))?;
        Ok(Self::from_raw(raw))
    }

    /// Create from the raw numeric value.
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0o100644 => Self::Regular,
            0o100755 => Self::Executable,
            0o120000 => Self::Symlink,
            0o160000 => Self::Gitlink,
            0o040000 => Self::Tree,
            other => Self::Unknown(other),
        }
    }

    /// Serialize to octal ASCII bytes (git's canonical format, no leading zeros for trees).
    pub fn as_bytes(&self) -> BString {
        BString::from(format!("{:o}", self.raw()))
    }

    /// Get the raw numeric value.
    pub fn raw(&self) -> u32 {
        match self {
            Self::Regular => 0o100644,
            Self::Executable => 0o100755,
            Self::Symlink => 0o120000,
            Self::Gitlink => 0o160000,
            Self::Tree => 0o40000,
            Self::Unknown(v) => *v,
        }
    }

    /// Is this a tree (directory) entry?
    pub fn is_tree(&self) -> bool {
        matches!(self, Self::Tree)
    }

    /// Is this a blob (file) entry?
    pub fn is_blob(&self) -> bool {
        matches!(self, Self::Regular | Self::Executable)
    }

    /// Is this a symlink?
    pub fn is_symlink(&self) -> bool {
        matches!(self, Self::Symlink)
    }

    /// Is this a gitlink (submodule)?
    pub fn is_gitlink(&self) -> bool {
        matches!(self, Self::Gitlink)
    }
}

/// Parse an octal ASCII string to u32.
fn parse_octal(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut val: u32 = 0;
    for &b in s {
        if !(b'0'..=b'7').contains(&b) {
            return None;
        }
        val = val.checked_mul(8)?.checked_add(u32::from(b - b'0'))?;
    }
    Some(val)
}

/// A single entry in a git tree object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: FileMode,
    pub name: BString,
    pub oid: ObjectId,
}

impl TreeEntry {
    /// Compare entries using git's tree sorting rules.
    ///
    /// Directories sort as if they have a trailing '/'. This means
    /// "foo" (dir) sorts before "foo.c" but after "foo-bar".
    pub fn cmp_entries(a: &TreeEntry, b: &TreeEntry) -> Ordering {
        base_name_compare(
            a.name.as_ref(),
            a.mode.is_tree(),
            b.name.as_ref(),
            b.mode.is_tree(),
        )
    }
}

impl PartialOrd for TreeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TreeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        Self::cmp_entries(self, other)
    }
}

/// Git's tree entry name comparison.
///
/// Faithfully implements C git's `base_name_compare`: after the common prefix,
/// directory names get an implicit trailing '/' for comparison.
fn base_name_compare(name1: &[u8], is_dir1: bool, name2: &[u8], is_dir2: bool) -> Ordering {
    let min_len = name1.len().min(name2.len());
    let cmp = name1[..min_len].cmp(&name2[..min_len]);
    if cmp != Ordering::Equal {
        return cmp;
    }
    // One name is a prefix of the other (or they're equal length).
    // Get the "next character" for each — null if at end, but '/' if it's a directory.
    let c1 = if name1.len() > min_len {
        name1[min_len]
    } else if is_dir1 {
        b'/'
    } else {
        0
    };
    let c2 = if name2.len() > min_len {
        name2[min_len]
    } else if is_dir2 {
        b'/'
    } else {
        0
    };
    c1.cmp(&c2)
}

/// A git tree object — a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Parse tree content from binary format.
    ///
    /// Each entry is: `<mode-ascii> <name>\0<oid-bytes>`
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError> {
        let mut entries = Vec::new();
        let mut pos = 0;

        while pos < content.len() {
            // Parse mode (octal ASCII until space).
            let space_pos = content[pos..]
                .iter()
                .position(|&b| b == b' ')
                .ok_or_else(|| ObjectError::InvalidTreeEntry {
                    offset: pos,
                    reason: "missing space after mode".into(),
                })?
                + pos;

            let mode = FileMode::from_bytes(&content[pos..space_pos]).map_err(|_| {
                ObjectError::InvalidTreeEntry {
                    offset: pos,
                    reason: "invalid mode".into(),
                }
            })?;

            // Parse name (until null byte).
            let name_start = space_pos + 1;
            let null_pos = content[name_start..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| ObjectError::InvalidTreeEntry {
                    offset: name_start,
                    reason: "missing null after name".into(),
                })?
                + name_start;

            let name = BString::from(&content[name_start..null_pos]);

            // Parse OID (raw bytes after null).
            let oid_start = null_pos + 1;
            // Determine hash size: try SHA-1 (20) first, then SHA-256 (32).
            let oid_len = 20; // SHA-1 is default for now.
            if oid_start + oid_len > content.len() {
                return Err(ObjectError::InvalidTreeEntry {
                    offset: oid_start,
                    reason: "truncated OID".into(),
                });
            }

            let oid = ObjectId::from_bytes(
                &content[oid_start..oid_start + oid_len],
                HashAlgorithm::Sha1,
            )?;

            entries.push(TreeEntry { mode, name, oid });
            pos = oid_start + oid_len;
        }

        Ok(Self { entries })
    }

    /// Serialize tree content to binary format.
    ///
    /// Entries are written in git canonical sort order.
    pub fn serialize_content(&self) -> Vec<u8> {
        let mut sorted = self.entries.clone();
        sorted.sort();

        let mut out = Vec::new();
        for entry in &sorted {
            out.extend_from_slice(&entry.mode.as_bytes());
            out.push(b' ');
            out.extend_from_slice(&entry.name);
            out.push(0);
            out.extend_from_slice(entry.oid.as_bytes());
        }
        out
    }

    /// Sort entries in git canonical order.
    pub fn sort(&mut self) {
        self.entries.sort();
    }

    /// Lookup an entry by name.
    pub fn find(&self, name: &BStr) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name.as_bstr() == name)
    }

    /// Iterate entries.
    pub fn iter(&self) -> impl Iterator<Item = &TreeEntry> {
        self.entries.iter()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_mode_from_bytes() {
        assert_eq!(FileMode::from_bytes(b"100644").unwrap(), FileMode::Regular);
        assert_eq!(
            FileMode::from_bytes(b"100755").unwrap(),
            FileMode::Executable
        );
        assert_eq!(FileMode::from_bytes(b"120000").unwrap(), FileMode::Symlink);
        assert_eq!(FileMode::from_bytes(b"160000").unwrap(), FileMode::Gitlink);
        assert_eq!(FileMode::from_bytes(b"40000").unwrap(), FileMode::Tree);
    }

    #[test]
    fn file_mode_roundtrip() {
        for mode in [
            FileMode::Regular,
            FileMode::Executable,
            FileMode::Symlink,
            FileMode::Gitlink,
            FileMode::Tree,
        ] {
            let bytes = mode.as_bytes();
            let parsed = FileMode::from_bytes(&bytes).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn file_mode_predicates() {
        assert!(FileMode::Tree.is_tree());
        assert!(!FileMode::Regular.is_tree());
        assert!(FileMode::Regular.is_blob());
        assert!(FileMode::Executable.is_blob());
        assert!(!FileMode::Tree.is_blob());
        assert!(FileMode::Symlink.is_symlink());
        assert!(FileMode::Gitlink.is_gitlink());
    }

    #[test]
    fn tree_sorting_dir_vs_file() {
        // "foo" (dir) sorts as "foo/", "foo.c" sorts as-is.
        // Since '/' (0x2F) > '.' (0x2E), "foo/" > "foo.c" => dir sorts AFTER foo.c.
        let dir_entry = TreeEntry {
            mode: FileMode::Tree,
            name: BString::from("foo"),
            oid: ObjectId::NULL_SHA1,
        };
        let file_entry = TreeEntry {
            mode: FileMode::Regular,
            name: BString::from("foo.c"),
            oid: ObjectId::NULL_SHA1,
        };
        assert_eq!(TreeEntry::cmp_entries(&dir_entry, &file_entry), Ordering::Greater);
    }

    #[test]
    fn tree_sorting_dir_after_hyphenated() {
        // "foo" (dir) should sort after "foo-bar" because "foo/" > "foo-"
        let dir_entry = TreeEntry {
            mode: FileMode::Tree,
            name: BString::from("foo"),
            oid: ObjectId::NULL_SHA1,
        };
        let file_entry = TreeEntry {
            mode: FileMode::Regular,
            name: BString::from("foo-bar"),
            oid: ObjectId::NULL_SHA1,
        };
        assert_eq!(TreeEntry::cmp_entries(&dir_entry, &file_entry), Ordering::Greater);
    }

    #[test]
    fn parse_empty_tree() {
        let tree = Tree::parse(b"").unwrap();
        assert!(tree.is_empty());
    }

    #[test]
    fn parse_single_entry() {
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        let mut data = Vec::new();
        data.extend_from_slice(b"100644 hello.txt\0");
        data.extend_from_slice(oid.as_bytes());

        let tree = Tree::parse(&data).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.entries[0].mode, FileMode::Regular);
        assert_eq!(tree.entries[0].name, "hello.txt");
        assert_eq!(tree.entries[0].oid, oid);
    }

    #[test]
    fn serialize_roundtrip() {
        let oid1 = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        let oid2 = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();

        let tree = Tree {
            entries: vec![
                TreeEntry {
                    mode: FileMode::Regular,
                    name: BString::from("b.txt"),
                    oid: oid1,
                },
                TreeEntry {
                    mode: FileMode::Tree,
                    name: BString::from("a-dir"),
                    oid: oid2,
                },
            ],
        };

        let serialized = tree.serialize_content();
        let parsed = Tree::parse(&serialized).unwrap();
        // Entries should be sorted in serialized form.
        assert_eq!(parsed.entries[0].name, "a-dir");
        assert_eq!(parsed.entries[1].name, "b.txt");
    }

    #[test]
    fn find_entry() {
        let oid = ObjectId::NULL_SHA1;
        let tree = Tree {
            entries: vec![
                TreeEntry {
                    mode: FileMode::Regular,
                    name: BString::from("README.md"),
                    oid,
                },
                TreeEntry {
                    mode: FileMode::Tree,
                    name: BString::from("src"),
                    oid,
                },
            ],
        };
        assert!(tree.find(BStr::new("README.md")).is_some());
        assert!(tree.find(BStr::new("nonexistent")).is_none());
    }
}

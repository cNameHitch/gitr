//! Cache tree extension (TREE).
//!
//! Caches tree OIDs for fast commit. Each node records the number of index entries
//! it covers and (if valid) the tree OID. When an entry changes, its ancestors
//! are invalidated (entry_count set to -1).

use bstr::{BStr, BString, ByteSlice};
use git_hash::ObjectId;

use crate::IndexError;

/// Cache tree extension — cached tree OIDs for fast commit.
#[derive(Debug, Clone)]
pub struct CacheTree {
    pub root: CacheTreeNode,
}

/// A single node in the cache tree.
#[derive(Debug, Clone)]
pub struct CacheTreeNode {
    /// Name of this subtree (empty for root).
    pub name: BString,
    /// Number of entries covered by this tree (-1 = invalid).
    pub entry_count: i32,
    /// Tree OID (valid only if entry_count >= 0).
    pub oid: Option<ObjectId>,
    /// Child subtrees.
    pub children: Vec<CacheTreeNode>,
}

impl CacheTree {
    /// Extension signature.
    pub const SIGNATURE: &'static [u8; 4] = b"TREE";

    /// Parse a TREE extension from raw data.
    pub fn parse(data: &[u8]) -> Result<Self, IndexError> {
        let mut cursor = 0;
        // Root node: skip the NUL-terminated empty path name
        if cursor < data.len() && data[cursor] == 0 {
            cursor += 1;
        }
        let root = Self::parse_entry(data, &mut cursor, b"")?;
        Ok(CacheTree { root })
    }

    fn parse_node(data: &[u8], cursor: &mut usize, name: &[u8]) -> Result<CacheTreeNode, IndexError> {
        Self::parse_entry(data, cursor, name)
    }

    fn parse_entry(data: &[u8], cursor: &mut usize, name: &[u8]) -> Result<CacheTreeNode, IndexError> {
        // Read entry_count as ASCII decimal terminated by space
        let entry_count_end = data[*cursor..]
            .iter()
            .position(|&b| b == b' ')
            .ok_or_else(|| IndexError::InvalidExtension {
                sig: "TREE".into(),
                reason: "missing entry count".into(),
            })?
            + *cursor;

        let entry_count_str = std::str::from_utf8(&data[*cursor..entry_count_end])
            .map_err(|_| IndexError::InvalidExtension {
                sig: "TREE".into(),
                reason: "invalid entry count".into(),
            })?;
        let entry_count: i32 = entry_count_str.parse().map_err(|_| IndexError::InvalidExtension {
            sig: "TREE".into(),
            reason: format!("invalid entry count: {entry_count_str}"),
        })?;
        *cursor = entry_count_end + 1; // skip space

        // Read subtree_count as ASCII decimal terminated by newline
        let subtree_count_end = data[*cursor..]
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| IndexError::InvalidExtension {
                sig: "TREE".into(),
                reason: "missing subtree count".into(),
            })?
            + *cursor;

        let subtree_count_str = std::str::from_utf8(&data[*cursor..subtree_count_end])
            .map_err(|_| IndexError::InvalidExtension {
                sig: "TREE".into(),
                reason: "invalid subtree count".into(),
            })?;
        let subtree_count: usize = subtree_count_str.parse().map_err(|_| IndexError::InvalidExtension {
            sig: "TREE".into(),
            reason: format!("invalid subtree count: {subtree_count_str}"),
        })?;
        *cursor = subtree_count_end + 1; // skip newline

        // If valid (entry_count >= 0), read the OID (raw 20 bytes for SHA-1)
        let oid = if entry_count >= 0 {
            if *cursor + 20 > data.len() {
                return Err(IndexError::InvalidExtension {
                    sig: "TREE".into(),
                    reason: "truncated OID".into(),
                });
            }
            let oid = ObjectId::from_bytes(&data[*cursor..*cursor + 20], git_hash::HashAlgorithm::Sha1)
                .map_err(|_| IndexError::InvalidExtension {
                    sig: "TREE".into(),
                    reason: "invalid OID".into(),
                })?;
            *cursor += 20;
            Some(oid)
        } else {
            None
        };

        // Parse children
        let mut children = Vec::with_capacity(subtree_count);
        for _ in 0..subtree_count {
            // Read child name (NUL-terminated)
            let name_end = data[*cursor..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| IndexError::InvalidExtension {
                    sig: "TREE".into(),
                    reason: "missing child name".into(),
                })?
                + *cursor;
            let child_name = &data[*cursor..name_end];
            *cursor = name_end + 1; // skip NUL

            let child = Self::parse_node(data, cursor, child_name)?;
            children.push(child);
        }

        Ok(CacheTreeNode {
            name: BString::from(name),
            entry_count,
            oid,
            children,
        })
    }

    /// Serialize to raw bytes for writing.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        Self::serialize_node(&self.root, &mut buf, true);
        buf
    }

    fn serialize_node(node: &CacheTreeNode, buf: &mut Vec<u8>, is_root: bool) {
        // Write name (NUL-terminated, except for root which has no name prefix)
        if !is_root {
            buf.extend_from_slice(&node.name);
            buf.push(0);
        }

        // Write entry_count (ASCII) + space
        buf.extend_from_slice(node.entry_count.to_string().as_bytes());
        buf.push(b' ');

        // Write subtree_count (ASCII) + newline
        buf.extend_from_slice(node.children.len().to_string().as_bytes());
        buf.push(b'\n');

        // Write OID if valid
        if node.entry_count >= 0 {
            if let Some(ref oid) = node.oid {
                buf.extend_from_slice(oid.as_bytes());
            }
        }

        // Write children
        for child in &node.children {
            Self::serialize_node(child, buf, false);
        }
    }

    /// Invalidate the entry for the given path and all ancestors.
    pub fn invalidate(&mut self, path: &BStr) {
        Self::invalidate_node(&mut self.root, path.as_bytes());
    }

    fn invalidate_node(node: &mut CacheTreeNode, path: &[u8]) -> bool {
        // Find the first path component
        let slash_pos = path.iter().position(|&b| b == b'/');

        match slash_pos {
            Some(pos) => {
                let component = &path[..pos];
                let rest = &path[pos + 1..];
                // Find matching child and recurse
                for child in &mut node.children {
                    if child.name.as_bytes() == component
                        && Self::invalidate_node(child, rest)
                    {
                        node.entry_count = -1;
                        node.oid = None;
                        return true;
                    }
                }
                false
            }
            None => {
                // Leaf: invalidate this node
                node.entry_count = -1;
                node.oid = None;
                true
            }
        }
    }

    /// Get the tree OID for the root (if valid).
    pub fn root_oid(&self) -> Option<&ObjectId> {
        if self.root.entry_count >= 0 {
            self.root.oid.as_ref()
        } else {
            None
        }
    }
}

//! Index file writing.

use std::io::Write;
use std::path::Path;

use git_hash::{HashAlgorithm, ObjectId};
use git_hash::hasher::Hasher;
use git_object::{FileMode, ObjectType, Tree, TreeEntry};
use git_odb::ObjectDatabase;

use crate::entry::IndexEntry;
use crate::extensions::tree::CacheTree;
use crate::extensions::ResolveUndo;
use crate::{Index, IndexError, Stage};

/// Magic bytes at the start of every index file.
const INDEX_SIGNATURE: &[u8; 4] = b"DIRC";

/// Write the index to a file atomically using a lock file.
pub fn write_index(index: &Index, path: &Path) -> Result<(), IndexError> {
    let mut lock = git_utils::lockfile::LockFile::acquire(path)
        .map_err(|_| IndexError::LockFailed {
            path: path.to_path_buf(),
        })?;

    let data = serialize_index(index)?;
    lock.write_all(&data)?;
    lock.commit().map_err(|_| IndexError::LockFailed {
        path: path.to_path_buf(),
    })?;

    Ok(())
}

/// Serialize the index to bytes.
fn serialize_index(index: &Index) -> Result<Vec<u8>, IndexError> {
    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(INDEX_SIGNATURE);
    buf.extend_from_slice(&2u32.to_be_bytes()); // always write v2
    buf.extend_from_slice(&(index.entries.len() as u32).to_be_bytes());

    // Entries (must be sorted)
    for entry in index.iter() {
        write_entry(&mut buf, entry);
    }

    // Extensions
    if let Some(ref tree) = index.cache_tree {
        let tree_data = tree.serialize();
        buf.extend_from_slice(CacheTree::SIGNATURE);
        buf.extend_from_slice(&(tree_data.len() as u32).to_be_bytes());
        buf.extend_from_slice(&tree_data);
    }

    if let Some(ref reuc) = index.resolve_undo {
        let reuc_data = reuc.serialize();
        buf.extend_from_slice(ResolveUndo::SIGNATURE);
        buf.extend_from_slice(&(reuc_data.len() as u32).to_be_bytes());
        buf.extend_from_slice(&reuc_data);
    }

    // Unknown extensions (preserved for round-trip)
    for ext in &index.unknown_extensions {
        buf.extend_from_slice(&ext.signature);
        buf.extend_from_slice(&(ext.data.len() as u32).to_be_bytes());
        buf.extend_from_slice(&ext.data);
    }

    // Checksum
    let checksum = Hasher::digest(HashAlgorithm::Sha1, &buf)
        .map_err(|_| IndexError::InvalidHeader("checksum computation failed".into()))?;
    buf.extend_from_slice(checksum.as_bytes());

    Ok(buf)
}

/// Write a single v2 cache entry.
fn write_entry(buf: &mut Vec<u8>, entry: &IndexEntry) {
    let entry_start = buf.len();

    // Stat data (40 bytes)
    buf.extend_from_slice(&entry.stat.ctime_secs.to_be_bytes());
    buf.extend_from_slice(&entry.stat.ctime_nsecs.to_be_bytes());
    buf.extend_from_slice(&entry.stat.mtime_secs.to_be_bytes());
    buf.extend_from_slice(&entry.stat.mtime_nsecs.to_be_bytes());
    buf.extend_from_slice(&entry.stat.dev.to_be_bytes());
    buf.extend_from_slice(&entry.stat.ino.to_be_bytes());
    buf.extend_from_slice(&entry.mode.raw().to_be_bytes());
    buf.extend_from_slice(&entry.stat.uid.to_be_bytes());
    buf.extend_from_slice(&entry.stat.gid.to_be_bytes());
    buf.extend_from_slice(&entry.stat.size.to_be_bytes());

    // OID (20 bytes)
    buf.extend_from_slice(entry.oid.as_bytes());

    // Flags (16 bits)
    let name_len = std::cmp::min(entry.path.len(), 0xFFF) as u16;
    let mut flags: u16 = name_len;
    flags |= (entry.stage.as_u8() as u16) << 12;
    if entry.flags.assume_valid {
        flags |= 0x8000;
    }
    // Note: we write v2, so no extended flag bit
    buf.extend_from_slice(&flags.to_be_bytes());

    // Path
    buf.extend_from_slice(&entry.path);

    // Pad using C git formula: entry_size = ((40 + 20 + 2 + name_len + 8) & ~7)
    // The padding fills with NUL bytes from after the path to the end of the entry
    let entry_size = (40 + 20 + 2 + entry.path.len() + 8) & !7;
    let current_len = buf.len() - entry_start;
    let padding = entry_size - current_len;
    for _ in 0..padding {
        buf.push(0);
    }
}

/// Create a tree hierarchy from the current index entries.
pub fn write_tree_from_index(index: &Index, odb: &ObjectDatabase) -> Result<ObjectId, IndexError> {
    // Only include stage-0 entries
    let entries: Vec<&IndexEntry> = index.iter().filter(|e| e.stage == Stage::Normal).collect();

    if entries.is_empty() {
        // Write an empty tree
        let tree = Tree::new();
        let tree_bytes = tree.serialize_content();
        return Ok(odb.write_raw(ObjectType::Tree, &tree_bytes)?);
    }

    build_tree(&entries, b"", odb)
}

/// Recursively build tree objects from sorted index entries.
fn build_tree(
    entries: &[&IndexEntry],
    prefix: &[u8],
    odb: &ObjectDatabase,
) -> Result<ObjectId, IndexError> {
    let mut tree_entries: Vec<TreeEntry> = Vec::new();
    let mut i = 0;

    while i < entries.len() {
        let entry = entries[i];
        let path = &entry.path[prefix.len()..];

        if let Some(slash_pos) = path.iter().position(|&b| b == b'/') {
            // This is a subtree entry
            let dir_name = &path[..slash_pos];
            // Collect all entries under this subtree
            let subtree_end = entries[i..]
                .iter()
                .position(|e| {
                    let p = &e.path[prefix.len()..];
                    !p.starts_with(dir_name) || (p.len() > slash_pos && p[slash_pos] != b'/')
                })
                .map(|pos| i + pos)
                .unwrap_or(entries.len());

            let subtree_entries = &entries[i..subtree_end];

            // Build prefix for recursion
            let mut new_prefix = prefix.to_vec();
            new_prefix.extend_from_slice(dir_name);
            new_prefix.push(b'/');

            let subtree_oid = build_tree(subtree_entries, &new_prefix, odb)?;

            tree_entries.push(TreeEntry {
                mode: FileMode::Tree,
                name: dir_name.into(),
                oid: subtree_oid,
            });

            i = subtree_end;
        } else {
            // Direct entry (blob/symlink/gitlink)
            tree_entries.push(TreeEntry {
                mode: entry.mode,
                name: path.into(),
                oid: entry.oid,
            });
            i += 1;
        }
    }

    let mut tree = Tree::new();
    tree.entries = tree_entries;
    tree.sort();
    let tree_bytes = tree.serialize_content();
    Ok(odb.write_raw(ObjectType::Tree, &tree_bytes)?)
}

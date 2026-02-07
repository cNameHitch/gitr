//! Tree-to-tree diff.
//!
//! Walks two tree objects in parallel (sorted by git's tree entry order),
//! identifying added, deleted, modified, and type-changed entries.
//! Supports recursive diffing into nested trees.

use bstr::BString;
use git_hash::ObjectId;
use git_object::{FileMode, Object, Tree, TreeEntry};
use git_odb::ObjectDatabase;

use crate::algorithm;
use crate::binary::is_binary;
use crate::{DiffError, DiffOptions, DiffResult, FileDiff, FileStatus};

/// Diff two trees, producing a list of file-level changes.
///
/// Either tree OID can be None to represent an empty tree (e.g., for initial commits).
pub fn diff_trees(
    odb: &ObjectDatabase,
    old_tree: Option<&ObjectId>,
    new_tree: Option<&ObjectId>,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError> {
    let old = match old_tree {
        Some(oid) => Some(read_tree(odb, oid)?),
        None => None,
    };
    let new = match new_tree {
        Some(oid) => Some(read_tree(odb, oid)?),
        None => None,
    };

    let old_entries = old.as_ref().map_or(&[][..], |t| &t.entries);
    let new_entries = new.as_ref().map_or(&[][..], |t| &t.entries);

    let mut files = Vec::new();
    let prefix = BString::from("");
    diff_tree_entries(odb, old_entries, new_entries, &prefix, options, &mut files)?;

    Ok(DiffResult { files })
}

/// Read and parse a tree object from the ODB.
fn read_tree(odb: &ObjectDatabase, oid: &ObjectId) -> Result<Tree, DiffError> {
    let obj = odb
        .read(oid)
        .map_err(|e| DiffError::ObjectRead {
            oid: *oid,
            source: e,
        })?
        .ok_or(DiffError::ObjectNotFound(*oid))?;

    match obj {
        Object::Tree(tree) => Ok(tree),
        other => Err(DiffError::UnexpectedObjectType {
            oid: *oid,
            expected: "tree",
            actual: other.object_type().to_string(),
        }),
    }
}

/// Read blob data from the ODB. Returns empty vec for None OID.
pub(crate) fn read_blob(odb: &ObjectDatabase, oid: &ObjectId) -> Result<Vec<u8>, DiffError> {
    let obj = odb
        .read(oid)
        .map_err(|e| DiffError::ObjectRead {
            oid: *oid,
            source: e,
        })?
        .ok_or(DiffError::ObjectNotFound(*oid))?;

    match obj {
        Object::Blob(blob) => Ok(blob.data.to_vec()),
        other => Err(DiffError::UnexpectedObjectType {
            oid: *oid,
            expected: "blob",
            actual: other.object_type().to_string(),
        }),
    }
}

/// Recursively diff two sets of sorted tree entries.
fn diff_tree_entries(
    odb: &ObjectDatabase,
    old_entries: &[TreeEntry],
    new_entries: &[TreeEntry],
    prefix: &BString,
    options: &DiffOptions,
    files: &mut Vec<FileDiff>,
) -> Result<(), DiffError> {
    let mut oi = 0;
    let mut ni = 0;

    while oi < old_entries.len() || ni < new_entries.len() {
        match (old_entries.get(oi), new_entries.get(ni)) {
            (Some(old_entry), Some(new_entry)) => {
                let cmp = TreeEntry::cmp_entries(old_entry, new_entry);
                match cmp {
                    std::cmp::Ordering::Less => {
                        // Entry only in old -> deleted
                        collect_deleted(odb, old_entry, prefix, options, files)?;
                        oi += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        // Entry only in new -> added
                        collect_added(odb, new_entry, prefix, options, files)?;
                        ni += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        // Entry in both
                        if old_entry.oid != new_entry.oid
                            || old_entry.mode != new_entry.mode
                        {
                            collect_modified(
                                odb, old_entry, new_entry, prefix, options, files,
                            )?;
                        }
                        oi += 1;
                        ni += 1;
                    }
                }
            }
            (Some(old_entry), None) => {
                collect_deleted(odb, old_entry, prefix, options, files)?;
                oi += 1;
            }
            (None, Some(new_entry)) => {
                collect_added(odb, new_entry, prefix, options, files)?;
                ni += 1;
            }
            (None, None) => break,
        }
    }

    Ok(())
}

/// Build the full path for a tree entry.
fn full_path(prefix: &BString, name: &BString) -> BString {
    if prefix.is_empty() {
        name.clone()
    } else {
        let mut p = prefix.clone();
        p.push(b'/');
        p.extend_from_slice(name);
        p
    }
}

/// Collect a deleted entry (recursing into trees).
fn collect_deleted(
    odb: &ObjectDatabase,
    entry: &TreeEntry,
    prefix: &BString,
    options: &DiffOptions,
    files: &mut Vec<FileDiff>,
) -> Result<(), DiffError> {
    let path = full_path(prefix, &entry.name);

    if entry.mode.is_tree() {
        // Recurse into the deleted tree
        let tree = read_tree(odb, &entry.oid)?;
        diff_tree_entries(odb, &tree.entries, &[], &path, options, files)?;
    } else {
        if !matches_pathspec(&path, options) {
            return Ok(());
        }
        let blob_data = read_blob(odb, &entry.oid)?;
        let binary = is_binary(&blob_data);
        let hunks = if binary {
            Vec::new()
        } else {
            algorithm::diff_lines(&blob_data, &[], options.algorithm, options.context_lines)
        };
        files.push(FileDiff {
            status: FileStatus::Deleted,
            old_path: Some(path),
            new_path: None,
            old_mode: Some(entry.mode),
            new_mode: None,
            old_oid: Some(entry.oid),
            new_oid: None,
            hunks,
            is_binary: binary,
            similarity: None,
        });
    }
    Ok(())
}

/// Collect an added entry (recursing into trees).
fn collect_added(
    odb: &ObjectDatabase,
    entry: &TreeEntry,
    prefix: &BString,
    options: &DiffOptions,
    files: &mut Vec<FileDiff>,
) -> Result<(), DiffError> {
    let path = full_path(prefix, &entry.name);

    if entry.mode.is_tree() {
        let tree = read_tree(odb, &entry.oid)?;
        diff_tree_entries(odb, &[], &tree.entries, &path, options, files)?;
    } else {
        if !matches_pathspec(&path, options) {
            return Ok(());
        }
        let blob_data = read_blob(odb, &entry.oid)?;
        let binary = is_binary(&blob_data);
        let hunks = if binary {
            Vec::new()
        } else {
            algorithm::diff_lines(&[], &blob_data, options.algorithm, options.context_lines)
        };
        files.push(FileDiff {
            status: FileStatus::Added,
            old_path: None,
            new_path: Some(path),
            old_mode: None,
            new_mode: Some(entry.mode),
            old_oid: None,
            new_oid: Some(entry.oid),
            hunks,
            is_binary: binary,
            similarity: None,
        });
    }
    Ok(())
}

/// Collect a modified or type-changed entry.
fn collect_modified(
    odb: &ObjectDatabase,
    old_entry: &TreeEntry,
    new_entry: &TreeEntry,
    prefix: &BString,
    options: &DiffOptions,
    files: &mut Vec<FileDiff>,
) -> Result<(), DiffError> {
    let path = full_path(prefix, &old_entry.name);

    let old_is_tree = old_entry.mode.is_tree();
    let new_is_tree = new_entry.mode.is_tree();

    if old_is_tree && new_is_tree {
        // Both are trees: recurse
        let old_tree = read_tree(odb, &old_entry.oid)?;
        let new_tree = read_tree(odb, &new_entry.oid)?;
        diff_tree_entries(
            odb,
            &old_tree.entries,
            &new_tree.entries,
            &path,
            options,
            files,
        )?;
    } else if old_is_tree && !new_is_tree {
        // Tree replaced by file: delete tree contents, add file
        let old_tree = read_tree(odb, &old_entry.oid)?;
        diff_tree_entries(odb, &old_tree.entries, &[], &path, options, files)?;
        if matches_pathspec(&path, options) {
            let blob_data = read_blob(odb, &new_entry.oid)?;
            let binary = is_binary(&blob_data);
            let hunks = if binary {
                Vec::new()
            } else {
                algorithm::diff_lines(&[], &blob_data, options.algorithm, options.context_lines)
            };
            files.push(FileDiff {
                status: FileStatus::Added,
                old_path: None,
                new_path: Some(path),
                old_mode: None,
                new_mode: Some(new_entry.mode),
                old_oid: None,
                new_oid: Some(new_entry.oid),
                hunks,
                is_binary: binary,
                similarity: None,
            });
        }
    } else if !old_is_tree && new_is_tree {
        // File replaced by tree: delete file, add tree contents
        if matches_pathspec(&path, options) {
            let blob_data = read_blob(odb, &old_entry.oid)?;
            let binary = is_binary(&blob_data);
            let hunks = if binary {
                Vec::new()
            } else {
                algorithm::diff_lines(&blob_data, &[], options.algorithm, options.context_lines)
            };
            files.push(FileDiff {
                status: FileStatus::Deleted,
                old_path: Some(path.clone()),
                new_path: None,
                old_mode: Some(old_entry.mode),
                new_mode: None,
                old_oid: Some(old_entry.oid),
                new_oid: None,
                hunks,
                is_binary: binary,
                similarity: None,
            });
        }
        let new_tree = read_tree(odb, &new_entry.oid)?;
        diff_tree_entries(odb, &[], &new_tree.entries, &path, options, files)?;
    } else {
        // Both are non-tree entries
        if !matches_pathspec(&path, options) {
            return Ok(());
        }

        let status = if old_entry.mode != new_entry.mode
            && !mode_content_type_equal(old_entry.mode, new_entry.mode)
        {
            FileStatus::TypeChanged
        } else {
            FileStatus::Modified
        };

        let old_data = read_blob(odb, &old_entry.oid)?;
        let new_data = read_blob(odb, &new_entry.oid)?;
        let binary = is_binary(&old_data) || is_binary(&new_data);
        let hunks = if binary {
            Vec::new()
        } else {
            algorithm::diff_lines(&old_data, &new_data, options.algorithm, options.context_lines)
        };

        files.push(FileDiff {
            status,
            old_path: Some(path.clone()),
            new_path: Some(path),
            old_mode: Some(old_entry.mode),
            new_mode: Some(new_entry.mode),
            old_oid: Some(old_entry.oid),
            new_oid: Some(new_entry.oid),
            hunks,
            is_binary: binary,
            similarity: None,
        });
    }

    Ok(())
}

/// Check whether two modes represent the same content type
/// (e.g., Regular and Executable are both blob types).
fn mode_content_type_equal(a: FileMode, b: FileMode) -> bool {
    a.is_blob() == b.is_blob()
        && a.is_symlink() == b.is_symlink()
        && a.is_gitlink() == b.is_gitlink()
}

/// Check if a path matches the pathspec filter.
fn matches_pathspec(path: &BString, options: &DiffOptions) -> bool {
    match &options.pathspec {
        None => true,
        Some(specs) => specs.iter().any(|spec| {
            path.starts_with(spec.as_slice())
                || spec.starts_with(path.as_slice())
        }),
    }
}

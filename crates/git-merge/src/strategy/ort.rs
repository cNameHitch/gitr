//! ORT (Ostensibly Recursive's Twin) merge strategy.
//!
//! This is the default merge strategy since git 2.34. It performs a
//! three-way tree merge with rename detection, handling all conflict types.

use std::collections::{BTreeMap, HashMap};

use bstr::BString;
use git_diff::tree::diff_trees;
use git_diff::{DiffOptions, FileDiff, FileStatus};
use git_hash::ObjectId;
use git_object::{Blob, FileMode, Object, Tree, TreeEntry};
use git_odb::ObjectDatabase;
use git_repository::Repository;

use crate::conflict::record_conflict_in_index;
use crate::content::{merge_content, MergeLabels};
use crate::strategy::MergeStrategy;
use crate::{
    ConflictEntry, ConflictSide, ConflictType, MergeError, MergeOptions, MergeResult,
};

/// The ORT merge strategy.
pub struct OrtStrategy;

impl MergeStrategy for OrtStrategy {
    fn merge(
        &self,
        repo: &mut Repository,
        ours_commit: &ObjectId,
        theirs_commit: &ObjectId,
        base_commit: &ObjectId,
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError> {
        let odb = repo.odb();

        // Read the three commits and extract their tree OIDs.
        let base_tree_id = read_commit_tree(odb, base_commit)?;
        let ours_tree_id = read_commit_tree(odb, ours_commit)?;
        let theirs_tree_id = read_commit_tree(odb, theirs_commit)?;

        // If ours and theirs trees are the same, nothing to merge.
        if ours_tree_id == theirs_tree_id {
            return Ok(MergeResult::clean(ours_tree_id));
        }

        // If base == ours, fast-forward to theirs.
        if base_tree_id == ours_tree_id {
            return Ok(MergeResult::clean(theirs_tree_id));
        }

        // If base == theirs, already up-to-date.
        if base_tree_id == theirs_tree_id {
            return Ok(MergeResult::clean(ours_tree_id));
        }

        // Perform three-way tree diff.
        let diff_opts = DiffOptions {
            detect_renames: true,
            rename_threshold: options.rename_threshold,
            ..DiffOptions::default()
        };

        let base_ours_diff = diff_trees(odb, Some(&base_tree_id), Some(&ours_tree_id), &diff_opts)?;
        let base_theirs_diff = diff_trees(odb, Some(&base_tree_id), Some(&theirs_tree_id), &diff_opts)?;

        // Build change maps keyed by path.
        let ours_changes = build_change_map(&base_ours_diff.files);
        let theirs_changes = build_change_map(&base_theirs_diff.files);

        // Collect all paths that changed in either side.
        let mut all_paths: Vec<&BString> = ours_changes
            .keys()
            .chain(theirs_changes.keys())
            .collect();
        all_paths.sort();
        all_paths.dedup();

        // Process each changed path.
        let mut conflicts = Vec::new();
        // Build result tree entries starting from the base tree.
        let base_tree = read_tree(odb, &base_tree_id)?;
        let mut result_entries = tree_to_flat_map(odb, &base_tree, &BString::from(""))?;

        let labels = MergeLabels {
            base: "base",
            ours: "HEAD",
            theirs: "merge",
        };

        for path in &all_paths {
            let o_change = ours_changes.get(*path);
            let t_change = theirs_changes.get(*path);

            match (o_change, t_change) {
                (Some(ours_fd), None) => {
                    // Only ours changed — take ours.
                    apply_change_to_map(&mut result_entries, path, ours_fd);
                }
                (None, Some(theirs_fd)) => {
                    // Only theirs changed — take theirs.
                    apply_change_to_map(&mut result_entries, path, theirs_fd);
                }
                (Some(ours_fd), Some(theirs_fd)) => {
                    // Both sides changed the same path.
                    match (ours_fd.status, theirs_fd.status) {
                        // Both modified the same file — content merge.
                        (FileStatus::Modified, FileStatus::Modified) => {
                            let conflict_or_clean = merge_file_content(
                                odb,
                                path,
                                ours_fd,
                                theirs_fd,
                                &base_tree_id,
                                options,
                                &labels,
                            )?;
                            match conflict_or_clean {
                                FileResolution::Clean { oid, mode } => {
                                    result_entries.insert(
                                        (*path).clone(),
                                        FlatEntry { oid, mode },
                                    );
                                }
                                FileResolution::Conflict(entry) => {
                                    conflicts.push(*entry);
                                }
                            }
                        }
                        // One deleted, the other modified — modify/delete conflict.
                        (FileStatus::Deleted, FileStatus::Modified)
                        | (FileStatus::Modified, FileStatus::Deleted) => {
                            let (modifier, _deleter, is_ours_delete) =
                                if ours_fd.status == FileStatus::Deleted {
                                    (theirs_fd, ours_fd, true)
                                } else {
                                    (ours_fd, theirs_fd, false)
                                };

                            let modified_oid = modifier.new_oid.unwrap_or_else(|| {
                                modifier.old_oid.unwrap_or(ObjectId::NULL_SHA1)
                            });
                            let modified_mode = modifier.new_mode.unwrap_or(FileMode::Regular);

                            let base_oid = modifier.old_oid.unwrap_or(ObjectId::NULL_SHA1);
                            let base_mode = modifier.old_mode.unwrap_or(FileMode::Regular);

                            let (ours_side, theirs_side) = if is_ours_delete {
                                (
                                    None,
                                    Some(ConflictSide {
                                        oid: modified_oid,
                                        mode: modified_mode,
                                        path: (*path).clone(),
                                    }),
                                )
                            } else {
                                (
                                    Some(ConflictSide {
                                        oid: modified_oid,
                                        mode: modified_mode,
                                        path: (*path).clone(),
                                    }),
                                    None,
                                )
                            };

                            conflicts.push(ConflictEntry {
                                path: (*path).clone(),
                                conflict_type: ConflictType::ModifyDelete,
                                base: Some(ConflictSide {
                                    oid: base_oid,
                                    mode: base_mode,
                                    path: (*path).clone(),
                                }),
                                ours: ours_side,
                                theirs: theirs_side,
                            });
                        }
                        // Both added the same path — add/add conflict.
                        (FileStatus::Added, FileStatus::Added) => {
                            let ours_oid =
                                ours_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);
                            let theirs_oid =
                                theirs_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);
                            let ours_mode =
                                ours_fd.new_mode.unwrap_or(FileMode::Regular);
                            let theirs_mode =
                                theirs_fd.new_mode.unwrap_or(FileMode::Regular);

                            if ours_oid == theirs_oid && ours_mode == theirs_mode {
                                // Identical adds — clean.
                                result_entries.insert(
                                    (*path).clone(),
                                    FlatEntry {
                                        oid: ours_oid,
                                        mode: ours_mode,
                                    },
                                );
                            } else {
                                conflicts.push(ConflictEntry {
                                    path: (*path).clone(),
                                    conflict_type: ConflictType::AddAdd,
                                    base: None,
                                    ours: Some(ConflictSide {
                                        oid: ours_oid,
                                        mode: ours_mode,
                                        path: (*path).clone(),
                                    }),
                                    theirs: Some(ConflictSide {
                                        oid: theirs_oid,
                                        mode: theirs_mode,
                                        path: (*path).clone(),
                                    }),
                                });
                            }
                        }
                        // Both deleted — clean (already gone).
                        (FileStatus::Deleted, FileStatus::Deleted) => {
                            result_entries.remove(*path);
                        }
                        // Renamed on both sides.
                        (FileStatus::Renamed, FileStatus::Renamed) => {
                            let ours_new_path = ours_fd.new_path.as_ref();
                            let theirs_new_path = theirs_fd.new_path.as_ref();

                            if ours_new_path == theirs_new_path {
                                // Same rename — clean, take ours content.
                                let oid = ours_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);
                                let mode = ours_fd.new_mode.unwrap_or(FileMode::Regular);
                                result_entries.remove(*path);
                                if let Some(new_path) = ours_new_path {
                                    result_entries
                                        .insert(new_path.clone(), FlatEntry { oid, mode });
                                }
                            } else {
                                // Different rename targets — rename/rename conflict.
                                let base_oid =
                                    ours_fd.old_oid.unwrap_or(ObjectId::NULL_SHA1);
                                let base_mode =
                                    ours_fd.old_mode.unwrap_or(FileMode::Regular);

                                conflicts.push(ConflictEntry {
                                    path: (*path).clone(),
                                    conflict_type: ConflictType::RenameRename,
                                    base: Some(ConflictSide {
                                        oid: base_oid,
                                        mode: base_mode,
                                        path: (*path).clone(),
                                    }),
                                    ours: ours_new_path.map(|p| ConflictSide {
                                        oid: ours_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1),
                                        mode: ours_fd.new_mode.unwrap_or(FileMode::Regular),
                                        path: p.clone(),
                                    }),
                                    theirs: theirs_new_path.map(|p| ConflictSide {
                                        oid: theirs_fd
                                            .new_oid
                                            .unwrap_or(ObjectId::NULL_SHA1),
                                        mode: theirs_fd
                                            .new_mode
                                            .unwrap_or(FileMode::Regular),
                                        path: p.clone(),
                                    }),
                                });
                            }
                        }
                        // Renamed on one side, modified on the other — follow rename, merge content.
                        (FileStatus::Renamed, FileStatus::Modified)
                        | (FileStatus::Modified, FileStatus::Renamed) => {
                            let (rename_fd, _modify_fd) =
                                if ours_fd.status == FileStatus::Renamed {
                                    (ours_fd, theirs_fd)
                                } else {
                                    (theirs_fd, ours_fd)
                                };

                            let new_path = rename_fd
                                .new_path
                                .as_ref()
                                .unwrap_or(*path);

                            // Content merge at the new location.
                            let base_oid =
                                rename_fd.old_oid.unwrap_or(ObjectId::NULL_SHA1);
                            let ours_oid =
                                ours_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);
                            let theirs_oid =
                                theirs_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);

                            let base_data = read_blob_data(odb, &base_oid)?;
                            let ours_data = read_blob_data(odb, &ours_oid)?;
                            let theirs_data = read_blob_data(odb, &theirs_oid)?;

                            let merge_result = merge_content(
                                &base_data,
                                &ours_data,
                                &theirs_data,
                                options,
                                &labels,
                            );

                            // Remove old path.
                            result_entries.remove(*path);

                            let mode = rename_fd.new_mode.unwrap_or(FileMode::Regular);

                            if merge_result.is_clean() {
                                let blob = Object::Blob(Blob::new(merge_result.content().to_vec()));
                                let new_oid = odb.write(&blob)?;
                                result_entries.insert(
                                    new_path.clone(),
                                    FlatEntry { oid: new_oid, mode },
                                );
                            } else {
                                conflicts.push(ConflictEntry {
                                    path: new_path.clone(),
                                    conflict_type: ConflictType::Content,
                                    base: Some(ConflictSide {
                                        oid: base_oid,
                                        mode,
                                        path: (*path).clone(),
                                    }),
                                    ours: Some(ConflictSide {
                                        oid: ours_oid,
                                        mode,
                                        path: new_path.clone(),
                                    }),
                                    theirs: Some(ConflictSide {
                                        oid: theirs_oid,
                                        mode,
                                        path: new_path.clone(),
                                    }),
                                });
                            }
                        }
                        // Other combinations — take the more significant change.
                        _ => {
                            // Default: prefer ours for unknown combinations.
                            apply_change_to_map(&mut result_entries, path, ours_fd);
                        }
                    }
                }
                (None, None) => {
                    // No changes — shouldn't happen.
                }
            }
        }

        if conflicts.is_empty() {
            // Build result tree and write to ODB.
            let tree_oid = write_flat_map_as_tree(odb, &result_entries)?;
            Ok(MergeResult::clean(tree_oid))
        } else {
            // Record conflicts in index.
            let index = repo.index_mut()?;
            for conflict in &conflicts {
                record_conflict_in_index(index, conflict);
            }
            Ok(MergeResult::conflicted(conflicts))
        }
    }
}

/// Resolution of a single file merge.
enum FileResolution {
    Clean { oid: ObjectId, mode: FileMode },
    Conflict(Box<ConflictEntry>),
}

/// Merge two modifications of the same file using three-way content merge.
fn merge_file_content(
    odb: &ObjectDatabase,
    path: &BString,
    ours_fd: &FileDiff,
    theirs_fd: &FileDiff,
    _base_tree: &ObjectId,
    options: &MergeOptions,
    labels: &MergeLabels<'_>,
) -> Result<FileResolution, MergeError> {
    let base_oid = ours_fd.old_oid.unwrap_or(ObjectId::NULL_SHA1);
    let ours_oid = ours_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);
    let theirs_oid = theirs_fd.new_oid.unwrap_or(ObjectId::NULL_SHA1);

    // If OIDs are the same, it's a clean merge.
    if ours_oid == theirs_oid {
        let mode = ours_fd.new_mode.unwrap_or(FileMode::Regular);
        return Ok(FileResolution::Clean { oid: ours_oid, mode });
    }

    let base_data = read_blob_data(odb, &base_oid)?;
    let ours_data = read_blob_data(odb, &ours_oid)?;
    let theirs_data = read_blob_data(odb, &theirs_oid)?;

    let merge_result = merge_content(&base_data, &ours_data, &theirs_data, options, labels);
    let mode = ours_fd.new_mode.unwrap_or(FileMode::Regular);

    if merge_result.is_clean() {
        let blob = Object::Blob(Blob::new(merge_result.content().to_vec()));
        let oid = odb.write(&blob)?;
        Ok(FileResolution::Clean { oid, mode })
    } else {
        Ok(FileResolution::Conflict(Box::new(ConflictEntry {
            path: path.clone(),
            conflict_type: ConflictType::Content,
            base: Some(ConflictSide {
                oid: base_oid,
                mode,
                path: path.clone(),
            }),
            ours: Some(ConflictSide {
                oid: ours_oid,
                mode,
                path: path.clone(),
            }),
            theirs: Some(ConflictSide {
                oid: theirs_oid,
                mode,
                path: path.clone(),
            }),
        })))
    }
}

/// Read the tree OID from a commit.
fn read_commit_tree(odb: &ObjectDatabase, commit_oid: &ObjectId) -> Result<ObjectId, MergeError> {
    let obj = odb
        .read(commit_oid)?
        .ok_or(MergeError::ObjectNotFound(*commit_oid))?;

    match obj {
        Object::Commit(c) => Ok(c.tree),
        other => Err(MergeError::UnexpectedObjectType {
            oid: *commit_oid,
            expected: "commit",
            actual: other.object_type().to_string(),
        }),
    }
}

/// Read a tree from ODB.
fn read_tree(odb: &ObjectDatabase, tree_oid: &ObjectId) -> Result<Tree, MergeError> {
    let obj = odb
        .read(tree_oid)?
        .ok_or(MergeError::ObjectNotFound(*tree_oid))?;

    match obj {
        Object::Tree(t) => Ok(t),
        other => Err(MergeError::UnexpectedObjectType {
            oid: *tree_oid,
            expected: "tree",
            actual: other.object_type().to_string(),
        }),
    }
}

/// Read blob data from ODB. Returns empty for null OID.
fn read_blob_data(odb: &ObjectDatabase, oid: &ObjectId) -> Result<Vec<u8>, MergeError> {
    if oid.is_null() {
        return Ok(Vec::new());
    }

    let obj = odb
        .read(oid)?
        .ok_or(MergeError::ObjectNotFound(*oid))?;

    match obj {
        Object::Blob(b) => Ok(b.data),
        other => Err(MergeError::UnexpectedObjectType {
            oid: *oid,
            expected: "blob",
            actual: other.object_type().to_string(),
        }),
    }
}

/// Flat entry: OID + mode for a single file.
#[derive(Debug, Clone)]
struct FlatEntry {
    oid: ObjectId,
    mode: FileMode,
}

/// Build a map of path → FileDiff from a diff result.
fn build_change_map(files: &[FileDiff]) -> HashMap<BString, &FileDiff> {
    let mut map = HashMap::new();
    for fd in files {
        let path = fd.path().clone();
        map.insert(path, fd);
    }
    map
}

/// Flatten a tree into a BTreeMap of path → FlatEntry (recursive).
fn tree_to_flat_map(
    odb: &ObjectDatabase,
    tree: &Tree,
    prefix: &BString,
) -> Result<BTreeMap<BString, FlatEntry>, MergeError> {
    let mut map = BTreeMap::new();
    for entry in &tree.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.push(b'/');
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            let sub = read_tree(odb, &entry.oid)?;
            let sub_map = tree_to_flat_map(odb, &sub, &path)?;
            map.extend(sub_map);
        } else {
            map.insert(
                path,
                FlatEntry {
                    oid: entry.oid,
                    mode: entry.mode,
                },
            );
        }
    }
    Ok(map)
}

/// Apply a file diff change to the flat entry map.
fn apply_change_to_map(
    map: &mut BTreeMap<BString, FlatEntry>,
    path: &BString,
    fd: &FileDiff,
) {
    match fd.status {
        FileStatus::Deleted => {
            map.remove(path);
        }
        FileStatus::Added | FileStatus::Modified | FileStatus::TypeChanged => {
            if let (Some(oid), Some(mode)) = (fd.new_oid, fd.new_mode) {
                map.insert(path.clone(), FlatEntry { oid, mode });
            }
        }
        FileStatus::Renamed => {
            map.remove(path);
            if let (Some(new_path), Some(oid), Some(mode)) =
                (fd.new_path.as_ref(), fd.new_oid, fd.new_mode)
            {
                map.insert(new_path.clone(), FlatEntry { oid, mode });
            }
        }
        FileStatus::Copied => {
            if let (Some(new_path), Some(oid), Some(mode)) =
                (fd.new_path.as_ref(), fd.new_oid, fd.new_mode)
            {
                map.insert(new_path.clone(), FlatEntry { oid, mode });
            }
        }
        _ => {}
    }
}

/// Write a flat map of paths back as a nested tree structure to ODB.
fn write_flat_map_as_tree(
    odb: &ObjectDatabase,
    map: &BTreeMap<BString, FlatEntry>,
) -> Result<ObjectId, MergeError> {
    // Group entries by their top-level component.
    let mut top_entries: BTreeMap<BString, Vec<(BString, &FlatEntry)>> = BTreeMap::new();
    let mut direct_entries: Vec<TreeEntry> = Vec::new();

    for (path, entry) in map {
        if let Some(slash_pos) = path.iter().position(|&b| b == b'/') {
            let dir = BString::from(&path[..slash_pos]);
            let rest = BString::from(&path[slash_pos + 1..]);
            top_entries.entry(dir).or_default().push((rest, entry));
        } else {
            direct_entries.push(TreeEntry {
                mode: entry.mode,
                name: path.clone(),
                oid: entry.oid,
            });
        }
    }

    // Recursively build sub-trees.
    for (dir_name, sub_entries) in &top_entries {
        let sub_map: BTreeMap<BString, FlatEntry> = sub_entries
            .iter()
            .map(|(p, e)| (p.clone(), (*e).clone()))
            .collect();
        let sub_tree_oid = write_flat_map_as_tree(odb, &sub_map)?;
        direct_entries.push(TreeEntry {
            mode: FileMode::Tree,
            name: dir_name.clone(),
            oid: sub_tree_oid,
        });
    }

    // Sort entries by git's tree entry ordering.
    direct_entries.sort_by(TreeEntry::cmp_entries);

    let tree = Tree {
        entries: direct_entries,
    };
    let obj = Object::Tree(tree);
    let oid = odb.write(&obj)?;
    Ok(oid)
}

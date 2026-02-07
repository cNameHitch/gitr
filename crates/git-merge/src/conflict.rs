//! Conflict recording in the index and working tree.
//!
//! When a merge produces conflicts, this module writes conflict markers to
//! the working tree and records stage entries (1=base, 2=ours, 3=theirs) in
//! the index.

use std::fs;
use std::path::Path;

use bstr::{BStr, BString};
use git_hash::ObjectId;
use git_index::{Index, IndexEntry, Stage, StatData, EntryFlags};
use git_object::FileMode;
use git_odb::ObjectDatabase;

use crate::{ConflictEntry, MergeError};

/// Write conflict markers to a file in the working tree.
pub fn write_conflict_markers(
    work_tree: &Path,
    path: &BStr,
    content: &[u8],
) -> Result<(), MergeError> {
    let file_path = work_tree.join(path.to_string());
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file_path, content)?;
    Ok(())
}

/// Write clean merged content to a file in the working tree.
pub fn write_merged_content(
    work_tree: &Path,
    path: &BStr,
    content: &[u8],
) -> Result<(), MergeError> {
    let file_path = work_tree.join(path.to_string());
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file_path, content)?;
    Ok(())
}

/// Record conflict stages (1=base, 2=ours, 3=theirs) in the index.
///
/// Removes any existing stage-0 entry for the path and adds the
/// appropriate conflict stage entries.
pub fn record_conflict_in_index(
    index: &mut Index,
    conflict: &ConflictEntry,
) {
    let path: &BStr = conflict.path.as_ref();

    // Remove any existing stage-0 entry.
    index.remove(path, Stage::Normal);

    // Add stage 1 (base) if present.
    if let Some(ref base) = conflict.base {
        index.add(IndexEntry {
            path: conflict.path.clone(),
            oid: base.oid,
            mode: base.mode,
            stage: Stage::Base,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        });
    }

    // Add stage 2 (ours) if present.
    if let Some(ref ours) = conflict.ours {
        index.add(IndexEntry {
            path: conflict.path.clone(),
            oid: ours.oid,
            mode: ours.mode,
            stage: Stage::Ours,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        });
    }

    // Add stage 3 (theirs) if present.
    if let Some(ref theirs) = conflict.theirs {
        index.add(IndexEntry {
            path: conflict.path.clone(),
            oid: theirs.oid,
            mode: theirs.mode,
            stage: Stage::Theirs,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        });
    }
}

/// Record a clean merge result in the index (stage 0).
///
/// Writes the blob to the ODB, removes any conflict stages, and sets
/// a single stage-0 entry.
pub fn record_clean_merge_in_index(
    index: &mut Index,
    odb: &ObjectDatabase,
    path: &BStr,
    content: &[u8],
    mode: FileMode,
) -> Result<ObjectId, MergeError> {
    use git_object::Object;

    let blob = Object::Blob(git_object::Blob {
        data: content.to_vec(),
    });
    let oid = odb.write(&blob)?;

    // Remove any conflict stages.
    index.remove(path, Stage::Base);
    index.remove(path, Stage::Ours);
    index.remove(path, Stage::Theirs);

    // Add clean stage-0 entry.
    index.add(IndexEntry {
        path: BString::from(path),
        oid,
        mode,
        stage: Stage::Normal,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    });

    Ok(oid)
}

/// Remove all conflict stages for a path from the index and add a resolved stage-0 entry.
///
/// This is what `git add <file>` does after conflict resolution.
pub fn resolve_conflict(
    index: &mut Index,
    path: &BStr,
    oid: ObjectId,
    mode: FileMode,
) {
    index.remove(path, Stage::Base);
    index.remove(path, Stage::Ours);
    index.remove(path, Stage::Theirs);

    index.add(IndexEntry {
        path: BString::from(path),
        oid,
        mode,
        stage: Stage::Normal,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::ByteSlice;
    use git_hash::ObjectId;
    use crate::ConflictSide;

    fn test_oid(byte: u8) -> ObjectId {
        ObjectId::Sha1([byte; 20])
    }

    #[test]
    fn record_content_conflict() {
        let mut index = Index::new();
        let conflict = ConflictEntry {
            path: BString::from("file.txt"),
            conflict_type: crate::ConflictType::Content,
            base: Some(ConflictSide {
                oid: test_oid(1),
                mode: FileMode::Regular,
                path: BString::from("file.txt"),
            }),
            ours: Some(ConflictSide {
                oid: test_oid(2),
                mode: FileMode::Regular,
                path: BString::from("file.txt"),
            }),
            theirs: Some(ConflictSide {
                oid: test_oid(3),
                mode: FileMode::Regular,
                path: BString::from("file.txt"),
            }),
        };

        record_conflict_in_index(&mut index, &conflict);

        let path: &BStr = b"file.txt".as_bstr();
        assert!(index.get(path, Stage::Base).is_some());
        assert!(index.get(path, Stage::Ours).is_some());
        assert!(index.get(path, Stage::Theirs).is_some());
        assert!(index.get(path, Stage::Normal).is_none());
        assert!(index.has_conflicts(path));
    }

    #[test]
    fn record_modify_delete_conflict() {
        let mut index = Index::new();
        let conflict = ConflictEntry {
            path: BString::from("deleted.txt"),
            conflict_type: crate::ConflictType::ModifyDelete,
            base: Some(ConflictSide {
                oid: test_oid(1),
                mode: FileMode::Regular,
                path: BString::from("deleted.txt"),
            }),
            ours: Some(ConflictSide {
                oid: test_oid(2),
                mode: FileMode::Regular,
                path: BString::from("deleted.txt"),
            }),
            theirs: None, // Deleted on their side.
        };

        record_conflict_in_index(&mut index, &conflict);

        let path: &BStr = b"deleted.txt".as_bstr();
        assert!(index.get(path, Stage::Base).is_some());
        assert!(index.get(path, Stage::Ours).is_some());
        assert!(index.get(path, Stage::Theirs).is_none());
    }

    #[test]
    fn resolve_conflict_clears_stages() {
        let mut index = Index::new();

        // Add conflict stages.
        let path: &BStr = b"file.txt".as_bstr();
        index.add(IndexEntry {
            path: BString::from("file.txt"),
            oid: test_oid(1),
            mode: FileMode::Regular,
            stage: Stage::Base,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        });
        index.add(IndexEntry {
            path: BString::from("file.txt"),
            oid: test_oid(2),
            mode: FileMode::Regular,
            stage: Stage::Ours,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        });
        index.add(IndexEntry {
            path: BString::from("file.txt"),
            oid: test_oid(3),
            mode: FileMode::Regular,
            stage: Stage::Theirs,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        });

        assert!(index.has_conflicts(path));

        // Resolve.
        resolve_conflict(&mut index, path, test_oid(4), FileMode::Regular);

        assert!(!index.has_conflicts(path));
        assert!(index.get(path, Stage::Normal).is_some());
        assert!(index.get(path, Stage::Base).is_none());
        assert!(index.get(path, Stage::Ours).is_none());
        assert!(index.get(path, Stage::Theirs).is_none());
    }
}

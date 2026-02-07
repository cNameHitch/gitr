//! Integration tests for conflict recording in the index.

use bstr::{BString, ByteSlice};
use git_hash::ObjectId;
use git_index::{Index, Stage};
use git_merge::conflict::{record_conflict_in_index, resolve_conflict};
use git_merge::{ConflictEntry, ConflictSide, ConflictType};
use git_object::FileMode;

fn oid(byte: u8) -> ObjectId {
    ObjectId::Sha1([byte; 20])
}

#[test]
fn content_conflict_sets_three_stages() {
    let mut index = Index::new();

    let conflict = ConflictEntry {
        path: BString::from("src/main.rs"),
        conflict_type: ConflictType::Content,
        base: Some(ConflictSide {
            oid: oid(1),
            mode: FileMode::Regular,
            path: BString::from("src/main.rs"),
        }),
        ours: Some(ConflictSide {
            oid: oid(2),
            mode: FileMode::Regular,
            path: BString::from("src/main.rs"),
        }),
        theirs: Some(ConflictSide {
            oid: oid(3),
            mode: FileMode::Regular,
            path: BString::from("src/main.rs"),
        }),
    };

    record_conflict_in_index(&mut index, &conflict);

    let path = b"src/main.rs".as_bstr();
    assert!(index.has_conflicts(path));

    // Verify all three stages.
    let base = index.get(path, Stage::Base).expect("stage 1 missing");
    assert_eq!(base.oid, oid(1));

    let ours = index.get(path, Stage::Ours).expect("stage 2 missing");
    assert_eq!(ours.oid, oid(2));

    let theirs = index.get(path, Stage::Theirs).expect("stage 3 missing");
    assert_eq!(theirs.oid, oid(3));

    // No stage-0 entry.
    assert!(index.get(path, Stage::Normal).is_none());
}

#[test]
fn modify_delete_sets_two_stages() {
    let mut index = Index::new();

    let conflict = ConflictEntry {
        path: BString::from("file.txt"),
        conflict_type: ConflictType::ModifyDelete,
        base: Some(ConflictSide {
            oid: oid(10),
            mode: FileMode::Regular,
            path: BString::from("file.txt"),
        }),
        ours: Some(ConflictSide {
            oid: oid(20),
            mode: FileMode::Regular,
            path: BString::from("file.txt"),
        }),
        theirs: None, // deleted on their side
    };

    record_conflict_in_index(&mut index, &conflict);

    let path = b"file.txt".as_bstr();
    assert!(index.get(path, Stage::Base).is_some());
    assert!(index.get(path, Stage::Ours).is_some());
    assert!(index.get(path, Stage::Theirs).is_none());
}

#[test]
fn resolve_replaces_stages_with_stage0() {
    let mut index = Index::new();

    // Create a conflict.
    let conflict = ConflictEntry {
        path: BString::from("resolved.txt"),
        conflict_type: ConflictType::Content,
        base: Some(ConflictSide {
            oid: oid(1),
            mode: FileMode::Regular,
            path: BString::from("resolved.txt"),
        }),
        ours: Some(ConflictSide {
            oid: oid(2),
            mode: FileMode::Regular,
            path: BString::from("resolved.txt"),
        }),
        theirs: Some(ConflictSide {
            oid: oid(3),
            mode: FileMode::Regular,
            path: BString::from("resolved.txt"),
        }),
    };
    record_conflict_in_index(&mut index, &conflict);

    let path = b"resolved.txt".as_bstr();
    assert!(index.has_conflicts(path));

    // Resolve the conflict.
    resolve_conflict(&mut index, path, oid(99), FileMode::Regular);

    assert!(!index.has_conflicts(path));
    let entry = index.get(path, Stage::Normal).expect("stage 0 missing");
    assert_eq!(entry.oid, oid(99));
}

#[test]
fn add_add_conflict() {
    let mut index = Index::new();

    let conflict = ConflictEntry {
        path: BString::from("new_file.txt"),
        conflict_type: ConflictType::AddAdd,
        base: None, // no base (both sides adding)
        ours: Some(ConflictSide {
            oid: oid(10),
            mode: FileMode::Regular,
            path: BString::from("new_file.txt"),
        }),
        theirs: Some(ConflictSide {
            oid: oid(20),
            mode: FileMode::Regular,
            path: BString::from("new_file.txt"),
        }),
    };

    record_conflict_in_index(&mut index, &conflict);

    let path = b"new_file.txt".as_bstr();
    assert!(index.get(path, Stage::Base).is_none());
    assert!(index.get(path, Stage::Ours).is_some());
    assert!(index.get(path, Stage::Theirs).is_some());
}

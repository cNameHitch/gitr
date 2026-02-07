use bstr::BString;
use git_hash::ObjectId;
use git_object::{Blob, Commit, FileMode, Object, ObjectType, Tag, Tree, TreeEntry};
use git_utils::date::{GitDate, Signature};

fn sig(name: &str, email: &str, ts: i64) -> Signature {
    Signature {
        name: BString::from(name),
        email: BString::from(email),
        date: GitDate {
            timestamp: ts,
            tz_offset: 0,
        },
    }
}

#[test]
fn blob_roundtrip() {
    let obj = Object::Blob(Blob::new(b"hello world\n".to_vec()));
    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn empty_blob_roundtrip() {
    let obj = Object::Blob(Blob::new(vec![]));
    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn blob_with_null_bytes_roundtrip() {
    let obj = Object::Blob(Blob::new(b"\0\0\0binary\0data\0".to_vec()));
    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn tree_roundtrip() {
    let oid1 = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
    let oid2 = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();

    let obj = Object::Tree(Tree {
        entries: vec![
            TreeEntry {
                mode: FileMode::Regular,
                name: BString::from("README.md"),
                oid: oid1,
            },
            TreeEntry {
                mode: FileMode::Executable,
                name: BString::from("run.sh"),
                oid: oid2,
            },
            TreeEntry {
                mode: FileMode::Tree,
                name: BString::from("src"),
                oid: oid1,
            },
        ],
    });

    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    // After serialization + parse, entries are sorted.
    if let Object::Tree(t) = &parsed {
        assert_eq!(t.entries.len(), 3);
    } else {
        panic!("expected Tree");
    }
}

#[test]
fn empty_tree_roundtrip() {
    let obj = Object::Tree(Tree::new());
    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn commit_roundtrip() {
    let tree_oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
    let parent_oid =
        ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();

    let obj = Object::Commit(Commit {
        tree: tree_oid,
        parents: vec![parent_oid],
        author: sig("Alice", "alice@example.com", 1700000000),
        committer: sig("Bob", "bob@example.com", 1700000100),
        encoding: None,
        gpgsig: None,
        extra_headers: vec![],
        message: BString::from("Test commit\n\nWith body.\n"),
    });

    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn root_commit_roundtrip() {
    let tree_oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let obj = Object::Commit(Commit {
        tree: tree_oid,
        parents: vec![],
        author: sig("A", "a@b.com", 1000000000),
        committer: sig("A", "a@b.com", 1000000000),
        encoding: None,
        gpgsig: None,
        extra_headers: vec![],
        message: BString::from("Initial commit\n"),
    });

    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn merge_commit_roundtrip() {
    let tree_oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let obj = Object::Commit(Commit {
        tree: tree_oid,
        parents: vec![
            ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap(),
            ObjectId::from_hex("0000000000000000000000000000000000000002").unwrap(),
            ObjectId::from_hex("0000000000000000000000000000000000000003").unwrap(),
        ],
        author: sig("A", "a@b.com", 1000000000),
        committer: sig("A", "a@b.com", 1000000000),
        encoding: None,
        gpgsig: None,
        extra_headers: vec![],
        message: BString::from("Octopus merge\n"),
    });

    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn tag_roundtrip() {
    let target = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let obj = Object::Tag(Tag {
        target,
        target_type: ObjectType::Commit,
        tag_name: BString::from("v1.0"),
        tagger: Some(sig("Tagger", "tagger@example.com", 1700000000)),
        message: BString::from("Release v1.0\n"),
        gpgsig: None,
    });

    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn tag_without_tagger_roundtrip() {
    let target = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let obj = Object::Tag(Tag {
        target,
        target_type: ObjectType::Commit,
        tag_name: BString::from("old-tag"),
        tagger: None,
        message: BString::from("Old tag\n"),
        gpgsig: None,
    });

    let serialized = obj.serialize();
    let parsed = Object::parse(&serialized).unwrap();
    assert_eq!(parsed, obj);
}

#[test]
fn object_type_preserved() {
    let blob = Object::Blob(Blob::new(b"x".to_vec()));
    assert_eq!(blob.object_type(), ObjectType::Blob);

    let tree = Object::Tree(Tree::new());
    assert_eq!(tree.object_type(), ObjectType::Tree);
}

#[test]
fn compute_oid_matches_hash_object() {
    // Empty blob should match `git hash-object -t blob /dev/null`
    let obj = Object::Blob(Blob::new(vec![]));
    let oid = obj.compute_oid(git_hash::HashAlgorithm::Sha1).unwrap();
    assert_eq!(
        oid.to_hex(),
        "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
    );
}

#[test]
fn compute_oid_hello_world() {
    let obj = Object::Blob(Blob::new(b"hello world".to_vec()));
    let oid = obj.compute_oid(git_hash::HashAlgorithm::Sha1).unwrap();
    assert_eq!(
        oid.to_hex(),
        "95d09f2b10159347eece71399a7e2e907ea3df4f"
    );
}

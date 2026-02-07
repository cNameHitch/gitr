//! Integration tests: read objects from C git-generated packfiles.
//!
//! The fixture pack was created by running:
//! ```sh
//! git init && echo "Hello, World!" > hello.txt && git add . && git commit -m "initial"
//! echo "Modified content" > hello.txt && git add . && git commit -m "modify"
//! echo "Another file" > other.txt && git add . && git commit -m "add other"
//! git gc --aggressive
//! ```

use git_hash::ObjectId;
use git_object::ObjectType;
use git_pack::pack::PackFile;

fn fixture_pack() -> PackFile {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let pack_path = format!("{manifest_dir}/tests/fixtures/test.pack");
    PackFile::open(&pack_path).expect("failed to open fixture pack")
}

#[test]
fn open_c_git_pack() {
    let pack = fixture_pack();
    assert_eq!(pack.num_objects(), 9);
}

#[test]
fn read_known_blob() {
    let pack = fixture_pack();
    // "Hello, World!\n" blob
    let oid = ObjectId::from_hex("8ab686eafeb1f44702738c8b0f24f2567c36da6d").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Blob);
    assert_eq!(obj.data, b"Hello, World!\n");
}

#[test]
fn read_modified_blob() {
    let pack = fixture_pack();
    // "Modified content\n" blob
    let oid = ObjectId::from_hex("c217c63469eca3538ca896a55f1990121a909f9e").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Blob);
    assert_eq!(obj.data, b"Modified content\n");
}

#[test]
fn read_another_blob() {
    let pack = fixture_pack();
    // "Another file\n" blob
    let oid = ObjectId::from_hex("b0b9fc8f6cc2f8f110306ed7f6d1ce079541b41f").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Blob);
    assert_eq!(obj.data, b"Another file\n");
}

#[test]
fn read_commit_objects() {
    let pack = fixture_pack();

    // Latest commit: "add other"
    let oid = ObjectId::from_hex("5e58e79ed8941533e4353e1bde446360146b5a9f").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Commit);
    assert!(String::from_utf8_lossy(&obj.data).contains("add other"));

    // Middle commit: "modify"
    let oid = ObjectId::from_hex("41c9e03b1f6379dc63a09e441b3a47e5297358a8").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Commit);
    assert!(String::from_utf8_lossy(&obj.data).contains("modify"));
}

#[test]
fn read_delta_object() {
    let pack = fixture_pack();

    // The initial commit (98330ec) is stored as a delta of the modify commit (41c9e03)
    let oid = ObjectId::from_hex("98330ec338a352a9c88af3f844f26e9c1f0b1ce0").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Commit);
    assert!(String::from_utf8_lossy(&obj.data).contains("initial"));
}

#[test]
fn read_tree_objects() {
    let pack = fixture_pack();

    // Tree with two entries (hello.txt + other.txt)
    let oid = ObjectId::from_hex("50592f954e1e75402f22f429e1d663aee4ef5021").unwrap();
    let obj = pack.read_object(&oid).unwrap().unwrap();
    assert_eq!(obj.obj_type, ObjectType::Tree);
}

#[test]
fn read_all_objects_via_index() {
    let pack = fixture_pack();

    let mut count = 0;
    for (oid, _offset) in pack.index().iter() {
        let obj = pack.read_object(&oid).unwrap();
        assert!(obj.is_some(), "failed to read object {oid}");
        count += 1;
    }
    assert_eq!(count, 9);
}

#[test]
fn missing_oid_returns_none() {
    let pack = fixture_pack();
    let missing = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();
    assert_eq!(pack.read_object(&missing).unwrap(), None);
}

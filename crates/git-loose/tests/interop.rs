//! Integration tests: verify interoperability with C git.
//!
//! These tests create objects with one implementation and read them with
//! the other, ensuring byte-identical compatibility.

use std::process::Command;

use git_hash::{HashAlgorithm, ObjectId};
use git_loose::LooseObjectStore;
use git_object::{Blob, Object, ObjectType};

/// Create a temporary bare git repository and return (tempdir, objects_dir).
fn setup_git_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--bare"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git init failed");
    let objects_dir = dir.path().join("objects");
    (dir, objects_dir)
}

/// Use C git to write a blob and return the hex OID.
fn git_hash_object(repo_dir: &std::path::Path, content: &[u8]) -> String {
    let mut child = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(repo_dir)
        .env("GIT_DIR", repo_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    use std::io::Write;
    child.stdin.take().unwrap().write_all(content).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "git hash-object failed");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Use C git to read an object's content.
fn git_cat_file(repo_dir: &std::path::Path, oid_hex: &str) -> Vec<u8> {
    let output = Command::new("git")
        .args(["cat-file", "-p", oid_hex])
        .current_dir(repo_dir)
        .env("GIT_DIR", repo_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git cat-file -p failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// Use C git to get an object's type.
fn git_cat_file_type(repo_dir: &std::path::Path, oid_hex: &str) -> String {
    let output = Command::new("git")
        .args(["cat-file", "-t", oid_hex])
        .current_dir(repo_dir)
        .env("GIT_DIR", repo_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git cat-file -t failed");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Use C git to get an object's size.
fn git_cat_file_size(repo_dir: &std::path::Path, oid_hex: &str) -> usize {
    let output = Command::new("git")
        .args(["cat-file", "-s", oid_hex])
        .current_dir(repo_dir)
        .env("GIT_DIR", repo_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "git cat-file -s failed");
    String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

// ── US1: Read objects created by C git ──────────────────────────────────────

#[test]
fn read_blob_created_by_c_git() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"hello, gitr!\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = store.read(&oid).unwrap().expect("object should exist");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data, content),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn read_empty_blob_created_by_c_git() {
    let (dir, objects_dir) = setup_git_repo();
    let oid_hex = git_hash_object(dir.path(), b"");

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = store.read(&oid).unwrap().expect("object should exist");
    match &obj {
        Object::Blob(blob) => assert!(blob.data.is_empty()),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn read_header_matches_c_git() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"some content here\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let (obj_type, size) = store
        .read_header(&oid)
        .unwrap()
        .expect("header should exist");

    let c_type = git_cat_file_type(dir.path(), &oid_hex);
    let c_size = git_cat_file_size(dir.path(), &oid_hex);

    assert_eq!(obj_type, ObjectType::Blob);
    assert_eq!(obj_type.to_string(), c_type);
    assert_eq!(size, c_size);
}

#[test]
fn contains_returns_true_for_existing() {
    let (dir, objects_dir) = setup_git_repo();
    let oid_hex = git_hash_object(dir.path(), b"test data");

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    assert!(store.contains(&oid));
}

#[test]
fn contains_returns_false_for_missing() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();
    assert!(!store.contains(&oid));
}

#[test]
fn read_returns_none_for_missing() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();
    assert!(store.read(&oid).unwrap().is_none());
}

#[test]
fn read_header_returns_none_for_missing() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();
    assert!(store.read_header(&oid).unwrap().is_none());
}

#[test]
fn read_large_blob_created_by_c_git() {
    let (dir, objects_dir) = setup_git_repo();
    let content: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
    let oid_hex = git_hash_object(dir.path(), &content);

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = store.read(&oid).unwrap().expect("object should exist");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data.len(), content.len()),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn read_verified_detects_valid_object() {
    let (dir, objects_dir) = setup_git_repo();
    let oid_hex = git_hash_object(dir.path(), b"verified content");

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = store
        .read_verified(&oid)
        .unwrap()
        .expect("object should exist");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data, b"verified content"),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

// ── US2: Write objects readable by C git ────────────────────────────────────

#[test]
fn write_blob_readable_by_c_git() {
    let (dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"written by gitr\n";
    let obj = Object::Blob(Blob {
        data: content.to_vec(),
    });
    let oid = store.write(&obj).unwrap();

    let c_content = git_cat_file(dir.path(), &oid.to_hex());
    assert_eq!(c_content, content);

    let c_type = git_cat_file_type(dir.path(), &oid.to_hex());
    assert_eq!(c_type, "blob");
}

#[test]
fn write_empty_blob_readable_by_c_git() {
    let (dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let obj = Object::Blob(Blob { data: vec![] });
    let oid = store.write(&obj).unwrap();

    let c_content = git_cat_file(dir.path(), &oid.to_hex());
    assert!(c_content.is_empty());
}

#[test]
fn write_raw_matches_c_git_hash() {
    let (dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"raw write test\n";
    let gitr_oid = store.write_raw(ObjectType::Blob, content).unwrap();

    let c_oid_hex = git_hash_object(dir.path(), content);
    assert_eq!(gitr_oid.to_hex(), c_oid_hex);
}

#[test]
fn write_is_idempotent() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"idempotent test";
    let oid1 = store.write_raw(ObjectType::Blob, content).unwrap();
    let oid2 = store.write_raw(ObjectType::Blob, content).unwrap();
    assert_eq!(oid1, oid2);
}

#[test]
fn write_creates_fanout_directory() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"fanout directory test";
    let oid = store.write_raw(ObjectType::Blob, content).unwrap();

    let path = store.object_path(&oid);
    assert!(path.exists());
    assert!(path.parent().unwrap().is_dir());
}

#[test]
fn write_stream_matches_write_raw() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"stream write test";
    let oid1 = store.write_raw(ObjectType::Blob, content).unwrap();

    let mut cursor = std::io::Cursor::new(content);
    let oid2 = store
        .write_stream(ObjectType::Blob, content.len(), &mut cursor)
        .unwrap();
    assert_eq!(oid1, oid2);
}

// ── Roundtrip ───────────────────────────────────────────────────────────────

#[test]
fn roundtrip_blob() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);

    let content = b"roundtrip content\n";
    let obj = Object::Blob(Blob {
        data: content.to_vec(),
    });
    let oid = store.write(&obj).unwrap();

    let read_obj = store.read(&oid).unwrap().expect("should exist");
    assert_eq!(obj, read_obj);
}

// ── US3: Enumeration ────────────────────────────────────────────────────────

#[test]
fn enumerate_finds_all_objects() {
    let (dir, objects_dir) = setup_git_repo();

    let mut expected_oids = std::collections::HashSet::new();
    for i in 0..5 {
        let content = format!("object number {}", i);
        let hex = git_hash_object(dir.path(), content.as_bytes());
        expected_oids.insert(hex);
    }

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let found_oids: std::collections::HashSet<String> =
        store.iter().unwrap().map(|r| r.unwrap().to_hex()).collect();

    for expected in &expected_oids {
        assert!(found_oids.contains(expected), "missing OID: {}", expected);
    }
}

#[test]
fn enumerate_empty_store() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let count = store.iter().unwrap().count();
    assert_eq!(count, 0);
}

// ── US4: Streaming ──────────────────────────────────────────────────────────

#[test]
fn stream_read_blob() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"streaming test content\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let mut stream = store.stream(&oid).unwrap().expect("should exist");
    assert_eq!(stream.object_type(), ObjectType::Blob);
    assert_eq!(stream.size(), content.len());

    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut buf).unwrap();
    assert_eq!(buf, content);
    assert_eq!(stream.bytes_remaining(), 0);
}

#[test]
fn stream_returns_none_for_missing() {
    let (_dir, objects_dir) = setup_git_repo();
    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();
    assert!(store.stream(&oid).unwrap().is_none());
}

#[test]
fn stream_partial_read() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"partial read content that is fairly long to test partial reads\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let store = LooseObjectStore::open(&objects_dir, HashAlgorithm::Sha1);
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let mut stream = store.stream(&oid).unwrap().expect("should exist");

    let mut buf = [0u8; 10];
    let n = std::io::Read::read(&mut stream, &mut buf).unwrap();
    assert_eq!(n, 10);
    assert_eq!(&buf[..10], &content[..10]);
    assert_eq!(stream.bytes_remaining(), content.len() - 10);
}

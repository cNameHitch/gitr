//! Integration tests: alternates chain resolution.
//!
//! Tests verify that objects from alternate object stores are accessible
//! through the primary ODB, including nested alternates and circular
//! chain detection.

use std::fs;
use std::process::Command;

use git_hash::ObjectId;
use git_object::{Object, ObjectType};
use git_odb::ObjectDatabase;

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

/// Set up alternates file pointing to another repo's objects directory.
fn setup_alternates(objects_dir: &std::path::Path, alternate_objects_dir: &std::path::Path) {
    let info_dir = objects_dir.join("info");
    fs::create_dir_all(&info_dir).unwrap();
    let alternates_path = info_dir.join("alternates");
    fs::write(
        &alternates_path,
        format!("{}\n", alternate_objects_dir.display()),
    )
    .unwrap();
}

// ── US3: Alternates ─────────────────────────────────────────────────────────

#[test]
fn read_object_from_alternate() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (dir_b, objects_dir_b) = setup_git_repo();

    // Create an object in repo B
    let content = b"object in alternate repo\n";
    let oid_hex = git_hash_object(dir_b.path(), content);

    // Set up repo A to use repo B as alternate
    setup_alternates(&objects_dir_a, &objects_dir_b);

    // Open repo A's ODB — it should find objects from repo B
    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = odb.read(&oid).unwrap().expect("should find in alternate");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data, content),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn contains_finds_object_in_alternate() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (dir_b, objects_dir_b) = setup_git_repo();

    let oid_hex = git_hash_object(dir_b.path(), b"alternate exists check");
    setup_alternates(&objects_dir_a, &objects_dir_b);

    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    assert!(odb.contains(&oid));
}

#[test]
fn local_objects_preferred_over_alternates() {
    let (dir_a, objects_dir_a) = setup_git_repo();
    let (dir_b, objects_dir_b) = setup_git_repo();

    // Write same content to both repos
    let content = b"same content both repos\n";
    let oid_hex_a = git_hash_object(dir_a.path(), content);
    let oid_hex_b = git_hash_object(dir_b.path(), content);
    assert_eq!(oid_hex_a, oid_hex_b);

    setup_alternates(&objects_dir_a, &objects_dir_b);

    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex_a).unwrap();

    // Should succeed (loose local is checked first)
    let obj = odb.read(&oid).unwrap().expect("should exist");
    assert_eq!(obj.object_type(), ObjectType::Blob);
}

#[test]
fn nested_alternates_chain() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (_dir_b, objects_dir_b) = setup_git_repo();
    let (dir_c, objects_dir_c) = setup_git_repo();

    // Create object only in repo C
    let content = b"deep in the chain\n";
    let oid_hex = git_hash_object(dir_c.path(), content);

    // A → B → C
    setup_alternates(&objects_dir_a, &objects_dir_b);
    setup_alternates(&objects_dir_b, &objects_dir_c);

    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = odb.read(&oid).unwrap().expect("should find in nested alternate");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data, content),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn circular_alternates_detected() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (_dir_b, objects_dir_b) = setup_git_repo();

    // A → B → A (circular)
    setup_alternates(&objects_dir_a, &objects_dir_b);
    setup_alternates(&objects_dir_b, &objects_dir_a);

    let result = ObjectDatabase::open(&objects_dir_a);
    assert!(result.is_err(), "circular alternates should be detected");
}

#[test]
fn missing_alternate_path_skipped() {
    let (_dir_a, objects_dir_a) = setup_git_repo();

    // Point to non-existent path
    let info_dir = objects_dir_a.join("info");
    fs::create_dir_all(&info_dir).unwrap();
    fs::write(
        info_dir.join("alternates"),
        "/nonexistent/path/objects\n",
    )
    .unwrap();

    // Should open successfully (skips missing alternates)
    let odb = ObjectDatabase::open(&objects_dir_a);
    assert!(odb.is_ok());
}

#[test]
fn alternates_with_comments_and_blank_lines() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (dir_b, objects_dir_b) = setup_git_repo();

    let content = b"filtered alternates test\n";
    let oid_hex = git_hash_object(dir_b.path(), content);

    let info_dir = objects_dir_a.join("info");
    fs::create_dir_all(&info_dir).unwrap();
    fs::write(
        info_dir.join("alternates"),
        format!(
            "# This is a comment\n\n{}\n# Another comment\n",
            objects_dir_b.display()
        ),
    )
    .unwrap();

    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    assert!(odb.contains(&oid));
}

#[test]
fn read_header_from_alternate() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (dir_b, objects_dir_b) = setup_git_repo();

    let content = b"alternate header test\n";
    let oid_hex = git_hash_object(dir_b.path(), content);
    setup_alternates(&objects_dir_a, &objects_dir_b);

    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let info = odb
        .read_header(&oid)
        .unwrap()
        .expect("header should exist in alternate");
    assert_eq!(info.obj_type, ObjectType::Blob);
    assert_eq!(info.size, content.len());
}

#[test]
fn iter_all_oids_includes_alternates() {
    let (_dir_a, objects_dir_a) = setup_git_repo();
    let (dir_b, objects_dir_b) = setup_git_repo();

    let content = b"iterable alternate object\n";
    let oid_hex = git_hash_object(dir_b.path(), content);
    setup_alternates(&objects_dir_a, &objects_dir_b);

    let odb = ObjectDatabase::open(&objects_dir_a).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let all_oids: Vec<ObjectId> = odb
        .iter_all_oids()
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(
        all_oids.contains(&oid),
        "alternate OID should be in iterator"
    );
}

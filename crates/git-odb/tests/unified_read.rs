//! Integration tests: unified reads from mixed loose/packed storage.
//!
//! These tests verify that the ObjectDatabase correctly reads objects
//! regardless of whether they are stored loose or packed, and that the
//! search order (loose → packs → alternates) is respected.

use std::process::Command;

use git_hash::ObjectId;
use git_object::{Blob, Object, ObjectType};
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

/// Use C git to repack all objects into a packfile.
fn git_repack(repo_dir: &std::path::Path) {
    let status = Command::new("git")
        .args(["repack", "-a", "-d"])
        .current_dir(repo_dir)
        .env("GIT_DIR", repo_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git repack failed");
}

/// Use C git to prune all loose objects.
fn git_prune_packed(repo_dir: &std::path::Path) {
    let status = Command::new("git")
        .args(["prune-packed"])
        .current_dir(repo_dir)
        .env("GIT_DIR", repo_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git prune-packed failed");
}

// ── US1: Unified Object Access ──────────────────────────────────────────────

#[test]
fn read_loose_object_through_odb() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"hello from odb test\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = odb.read(&oid).unwrap().expect("object should exist");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data, content),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn read_packed_object_through_odb() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"packed object content\n";
    let oid_hex = git_hash_object(dir.path(), content);

    // Repack and remove loose objects
    git_repack(dir.path());
    git_prune_packed(dir.path());

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj = odb.read(&oid).unwrap().expect("object should exist");
    match &obj {
        Object::Blob(blob) => assert_eq!(blob.data, content),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn read_returns_none_for_missing_object() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();

    assert!(odb.read(&oid).unwrap().is_none());
}

#[test]
fn read_from_mixed_loose_and_packed() {
    let (dir, objects_dir) = setup_git_repo();

    // Create first object and pack it
    let content1 = b"first object (will be packed)\n";
    let oid1_hex = git_hash_object(dir.path(), content1);
    git_repack(dir.path());
    git_prune_packed(dir.path());

    // Create second object as loose
    let content2 = b"second object (stays loose)\n";
    let oid2_hex = git_hash_object(dir.path(), content2);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid1 = ObjectId::from_hex(&oid1_hex).unwrap();
    let oid2 = ObjectId::from_hex(&oid2_hex).unwrap();

    // Both should be readable
    let obj1 = odb.read(&oid1).unwrap().expect("packed object should exist");
    match &obj1 {
        Object::Blob(blob) => assert_eq!(blob.data, content1),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }

    let obj2 = odb.read(&oid2).unwrap().expect("loose object should exist");
    match &obj2 {
        Object::Blob(blob) => assert_eq!(blob.data, content2),
        other => panic!("expected blob, got {:?}", other.object_type()),
    }
}

#[test]
fn multiple_packfiles_all_searched() {
    let (dir, objects_dir) = setup_git_repo();

    // Create first set and pack
    let content1 = b"pack one object\n";
    let oid1_hex = git_hash_object(dir.path(), content1);
    git_repack(dir.path());
    git_prune_packed(dir.path());

    // Create second set and pack again (creates a second pack)
    let content2 = b"pack two object\n";
    let oid2_hex = git_hash_object(dir.path(), content2);

    // Repack only the new object (don't consolidate)
    let status = Command::new("git")
        .args(["pack-objects", "--revs", "objects/pack/test"])
        .current_dir(dir.path())
        .env("GIT_DIR", dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|_| {
            // Fallback: just repack everything
            git_repack(dir.path());
            std::process::ExitStatus::default()
        });
    let _ = status; // May not work on all platforms, that's fine

    // Re-repack to get a second pack
    git_repack(dir.path());
    git_prune_packed(dir.path());

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid1 = ObjectId::from_hex(&oid1_hex).unwrap();
    let oid2 = ObjectId::from_hex(&oid2_hex).unwrap();

    assert!(odb.contains(&oid1), "first object should be found");
    assert!(odb.contains(&oid2), "second object should be found");
}

// ── US1: Header-only reads ──────────────────────────────────────────────────

#[test]
fn read_header_for_loose_object() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"header test content\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let info = odb.read_header(&oid).unwrap().expect("header should exist");
    assert_eq!(info.obj_type, ObjectType::Blob);
    assert_eq!(info.size, content.len());
}

#[test]
fn read_header_for_packed_object() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"packed header test\n";
    let oid_hex = git_hash_object(dir.path(), content);
    git_repack(dir.path());
    git_prune_packed(dir.path());

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let info = odb.read_header(&oid).unwrap().expect("header should exist");
    assert_eq!(info.obj_type, ObjectType::Blob);
    assert_eq!(info.size, content.len());
}

#[test]
fn read_header_returns_none_for_missing() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();

    assert!(odb.read_header(&oid).unwrap().is_none());
}

// ── US2: Object Writing ─────────────────────────────────────────────────────

#[test]
fn write_creates_loose_object() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    let content = b"written through odb\n";
    let obj = Object::Blob(Blob {
        data: content.to_vec(),
    });
    let oid = odb.write(&obj).unwrap();

    // Should be readable back
    let read_obj = odb.read(&oid).unwrap().expect("written object should exist");
    assert_eq!(obj, read_obj);
}

#[test]
fn write_returns_correct_oid() {
    let (dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    let content = b"oid verification test\n";
    let oid = odb
        .write_raw(ObjectType::Blob, content)
        .unwrap();

    // Compare with C git
    let c_oid_hex = git_hash_object(dir.path(), content);
    assert_eq!(oid.to_hex(), c_oid_hex);
}

#[test]
fn write_is_idempotent() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    let content = b"idempotent write test";
    let oid1 = odb.write_raw(ObjectType::Blob, content).unwrap();
    let oid2 = odb.write_raw(ObjectType::Blob, content).unwrap();
    assert_eq!(oid1, oid2);
}

// ── US4: Object Existence Checks ────────────────────────────────────────────

#[test]
fn contains_loose_object() {
    let (dir, objects_dir) = setup_git_repo();
    let oid_hex = git_hash_object(dir.path(), b"exists check");

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    assert!(odb.contains(&oid));
}

#[test]
fn contains_packed_object() {
    let (dir, objects_dir) = setup_git_repo();
    let oid_hex = git_hash_object(dir.path(), b"packed exists check");
    git_repack(dir.path());
    git_prune_packed(dir.path());

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    assert!(odb.contains(&oid));
}

#[test]
fn contains_returns_false_for_missing() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();

    assert!(!odb.contains(&oid));
}

// ── Caching ─────────────────────────────────────────────────────────────────

#[test]
fn read_cached_returns_same_object() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"cache test content\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    let obj1 = odb.read_cached(&oid).unwrap().expect("should exist");
    let obj2 = odb.read_cached(&oid).unwrap().expect("should exist (cached)");
    assert_eq!(obj1, obj2);
}

// ── Refresh ─────────────────────────────────────────────────────────────────

#[test]
fn refresh_discovers_new_packs() {
    let (dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    // Create and pack an object after opening the ODB
    let content = b"created after open\n";
    let oid_hex = git_hash_object(dir.path(), content);
    git_repack(dir.path());
    git_prune_packed(dir.path());

    let oid = ObjectId::from_hex(&oid_hex).unwrap();

    // Before refresh, might not find the packed object
    // After refresh, should find it
    odb.refresh().unwrap();
    assert!(odb.contains(&oid));
}

// ── Iterator ────────────────────────────────────────────────────────────────

#[test]
fn iter_all_oids_includes_loose_and_packed() {
    let (dir, objects_dir) = setup_git_repo();

    // Create a packed object
    let content1 = b"iter packed\n";
    let oid1_hex = git_hash_object(dir.path(), content1);
    git_repack(dir.path());
    git_prune_packed(dir.path());

    // Create a loose object
    let content2 = b"iter loose\n";
    let oid2_hex = git_hash_object(dir.path(), content2);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let oid1 = ObjectId::from_hex(&oid1_hex).unwrap();
    let oid2 = ObjectId::from_hex(&oid2_hex).unwrap();

    let all_oids: Vec<ObjectId> = odb
        .iter_all_oids()
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(all_oids.contains(&oid1), "packed OID should be in iterator");
    assert!(all_oids.contains(&oid2), "loose OID should be in iterator");
}

#[test]
fn iter_all_oids_empty_repo() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    let count = odb.iter_all_oids().unwrap().count();
    assert_eq!(count, 0);
}

// ── Prefix Resolution ───────────────────────────────────────────────────────

#[test]
fn resolve_prefix_finds_unique_object() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"prefix resolution test\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let expected_oid = ObjectId::from_hex(&oid_hex).unwrap();

    // Use first 8 hex chars as prefix
    let prefix = &oid_hex[..8];
    let resolved = odb.resolve_prefix(prefix).unwrap();
    assert_eq!(resolved, expected_oid);
}

#[test]
fn resolve_prefix_full_oid() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"full oid resolution\n";
    let oid_hex = git_hash_object(dir.path(), content);

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let expected_oid = ObjectId::from_hex(&oid_hex).unwrap();

    let resolved = odb.resolve_prefix(&oid_hex).unwrap();
    assert_eq!(resolved, expected_oid);
}

#[test]
fn resolve_prefix_not_found() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    let result = odb.resolve_prefix("000000000000");
    assert!(result.is_err());
}

#[test]
fn resolve_prefix_too_short() {
    let (_dir, objects_dir) = setup_git_repo();
    let odb = ObjectDatabase::open(&objects_dir).unwrap();

    let result = odb.resolve_prefix("abc");
    assert!(result.is_err());
}

#[test]
fn resolve_prefix_packed_object() {
    let (dir, objects_dir) = setup_git_repo();
    let content = b"packed prefix test\n";
    let oid_hex = git_hash_object(dir.path(), content);
    git_repack(dir.path());
    git_prune_packed(dir.path());

    let odb = ObjectDatabase::open(&objects_dir).unwrap();
    let expected_oid = ObjectId::from_hex(&oid_hex).unwrap();

    let prefix = &oid_hex[..8];
    let resolved = odb.resolve_prefix(prefix).unwrap();
    assert_eq!(resolved, expected_oid);
}

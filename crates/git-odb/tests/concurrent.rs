//! Thread-safety stress tests for the ObjectDatabase.
//!
//! Verifies that concurrent reads from multiple threads work correctly
//! and don't corrupt data or panic.

use std::process::Command;
use std::sync::Arc;
use std::thread;

use git_hash::ObjectId;
use git_object::Object;
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

/// Use C git to repack all objects.
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

#[test]
fn concurrent_reads_from_loose_objects() {
    let (dir, objects_dir) = setup_git_repo();

    // Create a set of objects
    let mut oids = Vec::new();
    let mut contents = Vec::new();
    for i in 0..50 {
        let content = format!("concurrent test object {}\n", i);
        let oid_hex = git_hash_object(dir.path(), content.as_bytes());
        oids.push(ObjectId::from_hex(&oid_hex).unwrap());
        contents.push(content.into_bytes());
    }

    let odb = Arc::new(ObjectDatabase::open(&objects_dir).unwrap());

    let mut handles = Vec::new();
    for thread_id in 0..10 {
        let odb = Arc::clone(&odb);
        let oids = oids.clone();
        let contents = contents.clone();

        handles.push(thread::spawn(move || {
            for (i, oid) in oids.iter().enumerate() {
                let obj = odb.read(oid).unwrap().expect("object should exist");
                match &obj {
                    Object::Blob(blob) => {
                        assert_eq!(
                            blob.data, contents[i],
                            "thread {} got wrong content for object {}",
                            thread_id, i
                        );
                    }
                    other => panic!(
                        "thread {} expected blob, got {:?}",
                        thread_id,
                        other.object_type()
                    ),
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn concurrent_reads_from_packed_objects() {
    let (dir, objects_dir) = setup_git_repo();

    // Create objects and pack them
    let mut oids = Vec::new();
    let mut contents = Vec::new();
    for i in 0..50 {
        let content = format!("packed concurrent object {}\n", i);
        let oid_hex = git_hash_object(dir.path(), content.as_bytes());
        oids.push(ObjectId::from_hex(&oid_hex).unwrap());
        contents.push(content.into_bytes());
    }

    git_repack(dir.path());

    let odb = Arc::new(ObjectDatabase::open(&objects_dir).unwrap());

    let mut handles = Vec::new();
    for thread_id in 0..10 {
        let odb = Arc::clone(&odb);
        let oids = oids.clone();
        let contents = contents.clone();

        handles.push(thread::spawn(move || {
            for (i, oid) in oids.iter().enumerate() {
                let obj = odb.read(oid).unwrap().expect("object should exist");
                match &obj {
                    Object::Blob(blob) => {
                        assert_eq!(
                            blob.data, contents[i],
                            "thread {} got wrong content for packed object {}",
                            thread_id, i
                        );
                    }
                    other => panic!(
                        "thread {} expected blob, got {:?}",
                        thread_id,
                        other.object_type()
                    ),
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn concurrent_existence_checks() {
    let (dir, objects_dir) = setup_git_repo();

    let mut existing_oids = Vec::new();
    for i in 0..20 {
        let content = format!("exists check {}\n", i);
        let oid_hex = git_hash_object(dir.path(), content.as_bytes());
        existing_oids.push(ObjectId::from_hex(&oid_hex).unwrap());
    }

    let missing_oid = ObjectId::from_hex("0000000000000000000000000000000000000000").unwrap();

    let odb = Arc::new(ObjectDatabase::open(&objects_dir).unwrap());

    let mut handles = Vec::new();
    for _ in 0..10 {
        let odb = Arc::clone(&odb);
        let existing_oids = existing_oids.clone();

        handles.push(thread::spawn(move || {
            for oid in &existing_oids {
                assert!(odb.contains(oid), "should find existing object");
            }
            assert!(!odb.contains(&missing_oid), "should not find missing object");
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn concurrent_reads_mixed_loose_packed() {
    let (dir, objects_dir) = setup_git_repo();

    // Create first batch and pack
    let mut packed_oids = Vec::new();
    for i in 0..25 {
        let content = format!("packed obj {}\n", i);
        let oid_hex = git_hash_object(dir.path(), content.as_bytes());
        packed_oids.push(ObjectId::from_hex(&oid_hex).unwrap());
    }
    git_repack(dir.path());

    // Create second batch as loose
    let mut loose_oids = Vec::new();
    for i in 0..25 {
        let content = format!("loose obj {}\n", i);
        let oid_hex = git_hash_object(dir.path(), content.as_bytes());
        loose_oids.push(ObjectId::from_hex(&oid_hex).unwrap());
    }

    let odb = Arc::new(ObjectDatabase::open(&objects_dir).unwrap());

    let mut handles = Vec::new();
    for _ in 0..10 {
        let odb = Arc::clone(&odb);
        let packed_oids = packed_oids.clone();
        let loose_oids = loose_oids.clone();

        handles.push(thread::spawn(move || {
            for oid in packed_oids.iter().chain(loose_oids.iter()) {
                assert!(odb.contains(oid));
                let obj = odb.read(oid).unwrap();
                assert!(obj.is_some(), "object {} should exist", oid);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

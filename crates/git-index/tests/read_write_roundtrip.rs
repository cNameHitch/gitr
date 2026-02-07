//! Round-trip tests: create index with C git, read with gitr, write, verify.

use std::path::Path;
use std::process::Command;

use bstr::BStr;
use git_hash::ObjectId;
use git_index::{Index, IndexEntry, Stage};
use git_index::entry::{EntryFlags, StatData};
use git_object::FileMode;

/// Helper to check if git is available.
fn has_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a temporary git repo with some staged files, return the index path.
fn setup_git_repo(dir: &Path) -> std::path::PathBuf {
    let run = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .expect("git command failed");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };

    run(&["init"]);
    run(&["config", "user.email", "test@test.com"]);
    run(&["config", "user.name", "Test"]);

    // Create some files
    std::fs::write(dir.join("hello.txt"), b"Hello, world!\n").unwrap();
    std::fs::write(dir.join("README.md"), b"# Test\n").unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.rs"), b"fn main() {}\n").unwrap();
    std::fs::write(dir.join("src/lib.rs"), b"pub fn hello() {}\n").unwrap();

    // Stage all files
    run(&["add", "."]);

    dir.join(".git/index")
}

#[test]
fn read_c_git_index() {
    if !has_git() {
        eprintln!("Skipping test: git not available");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let index_path = setup_git_repo(dir.path());

    let index = Index::read_from(&index_path).expect("failed to read index");

    assert_eq!(index.len(), 4);
    assert_eq!(index.version(), 2);

    // Verify entries are sorted
    let paths: Vec<&BStr> = index.iter().map(|e| BStr::new(&e.path)).collect();
    assert_eq!(
        paths,
        vec![
            BStr::new(b"README.md"),
            BStr::new(b"hello.txt"),
            BStr::new(b"src/lib.rs"),
            BStr::new(b"src/main.rs"),
        ]
    );

    // Check specific entry
    let entry = index.get(BStr::new(b"hello.txt"), Stage::Normal).unwrap();
    assert_eq!(entry.mode, FileMode::Regular);
    assert_eq!(entry.stage, Stage::Normal);
    assert!(!entry.oid.is_null());
}

#[test]
fn roundtrip_read_write_read() {
    if !has_git() {
        eprintln!("Skipping test: git not available");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let index_path = setup_git_repo(dir.path());

    // Read original
    let index1 = Index::read_from(&index_path).expect("failed to read index");

    // Write to a new path
    let output_path = dir.path().join("index_out");
    index1.write_to(&output_path).expect("failed to write index");

    // Read back
    let index2 = Index::read_from(&output_path).expect("failed to re-read index");

    // Compare
    assert_eq!(index1.len(), index2.len());
    for (e1, e2) in index1.iter().zip(index2.iter()) {
        assert_eq!(e1.path, e2.path);
        assert_eq!(e1.oid, e2.oid);
        assert_eq!(e1.mode, e2.mode);
        assert_eq!(e1.stage, e2.stage);
        assert_eq!(e1.stat, e2.stat);
    }
}

#[test]
fn written_index_readable_by_c_git() {
    if !has_git() {
        eprintln!("Skipping test: git not available");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let index_path = setup_git_repo(dir.path());

    // Read, write back over original
    let index = Index::read_from(&index_path).expect("failed to read index");
    index.write_to(&index_path).expect("failed to write index");

    // Verify C git can read it
    let output = Command::new("git")
        .args(["ls-files", "--stage"])
        .current_dir(dir.path())
        .output()
        .expect("git ls-files failed");

    assert!(
        output.status.success(),
        "git ls-files failed after gitr wrote the index: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let ls_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = ls_output.lines().collect();
    assert_eq!(lines.len(), 4);

    // Verify the files are listed
    assert!(lines.iter().any(|l| l.ends_with("README.md")));
    assert!(lines.iter().any(|l| l.ends_with("hello.txt")));
    assert!(lines.iter().any(|l| l.ends_with("src/lib.rs")));
    assert!(lines.iter().any(|l| l.ends_with("src/main.rs")));
}

#[test]
fn empty_index_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index");

    let index = Index::new();
    index.write_to(&path).expect("failed to write empty index");

    let index2 = Index::read_from(&path).expect("failed to read empty index");
    assert_eq!(index2.len(), 0);
}

#[test]
fn add_and_remove_entries() {
    let mut index = Index::new();

    let entry1 = IndexEntry {
        path: "file_a.txt".into(),
        oid: ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Normal,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    };

    let entry2 = IndexEntry {
        path: "file_b.txt".into(),
        oid: ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Normal,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    };

    index.add(entry1);
    index.add(entry2);
    assert_eq!(index.len(), 2);

    // Entries should be sorted
    assert_eq!(&index.iter().next().unwrap().path[..], b"file_a.txt");

    // Remove
    assert!(index.remove(BStr::new(b"file_a.txt"), Stage::Normal));
    assert_eq!(index.len(), 1);
    assert!(!index.remove(BStr::new(b"file_a.txt"), Stage::Normal));
}

#[test]
fn conflict_detection() {
    let mut index = Index::new();

    // Add normal entry
    index.add(IndexEntry {
        path: "conflict.txt".into(),
        oid: ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Base,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    });

    index.add(IndexEntry {
        path: "conflict.txt".into(),
        oid: ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Ours,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    });

    index.add(IndexEntry {
        path: "conflict.txt".into(),
        oid: ObjectId::from_hex("cccccccccccccccccccccccccccccccccccccccc").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Theirs,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    });

    assert!(index.has_conflicts(BStr::new(b"conflict.txt")));
    assert_eq!(index.conflicts().len(), 1);
    assert_eq!(index.get_all(BStr::new(b"conflict.txt")).len(), 3);
}

#[test]
fn update_existing_entry() {
    let mut index = Index::new();

    let entry1 = IndexEntry {
        path: "file.txt".into(),
        oid: ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Normal,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    };

    index.add(entry1);
    assert_eq!(index.len(), 1);

    // Update with new OID
    let entry2 = IndexEntry {
        path: "file.txt".into(),
        oid: ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
        mode: FileMode::Regular,
        stage: Stage::Normal,
        stat: StatData::default(),
        flags: EntryFlags::default(),
    };

    index.add(entry2);
    assert_eq!(index.len(), 1);
    assert_eq!(
        index.get(BStr::new(b"file.txt"), Stage::Normal).unwrap().oid,
        ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap()
    );
}

//! Integration tests: verify reflog interoperability with C git.

use std::process::Command;

use bstr::BString;
use git_hash::ObjectId;
use git_ref::{FilesRefStore, RefName, RefStore, RefTransaction};
use git_utils::date::{GitDate, Signature};

fn setup_git_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();

    let status = Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial commit"])
        .current_dir(dir.path())
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let git_dir = dir.path().join(".git");
    (dir, git_dir)
}

fn git_rev_parse(repo_dir: &std::path::Path, refspec: &str) -> String {
    let output = Command::new("git")
        .args(["rev-parse", refspec])
        .current_dir(repo_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git rev-parse {} failed: {}",
        refspec,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn make_store(git_dir: &std::path::Path) -> FilesRefStore {
    let mut store = FilesRefStore::new(git_dir);
    store.set_committer(Signature {
        name: BString::from("Test User"),
        email: BString::from("test@example.com"),
        date: GitDate::new(1234567890, 0),
    });
    store
}

// ── US4: Read C git reflogs ─────────────────────────────────────────────────

#[test]
fn read_c_git_reflog() {
    let (dir, git_dir) = setup_git_repo();

    // Make a second commit to have multiple reflog entries
    let status = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "second commit"])
        .current_dir(dir.path())
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let store = FilesRefStore::new(&git_dir);
    let head = RefName::new("HEAD").unwrap();
    let entries = store.reflog(&head).unwrap();

    // Should have at least 2 entries (initial commit + second commit)
    assert!(
        entries.len() >= 2,
        "expected at least 2 reflog entries, got {}",
        entries.len()
    );

    // Most recent entry (index 0) should have the current HEAD as new_oid
    let current_oid_hex = git_rev_parse(dir.path(), "HEAD");
    assert_eq!(entries[0].new_oid.to_hex(), current_oid_hex);
}

#[test]
fn reflog_count_matches_c_git() {
    let (dir, git_dir) = setup_git_repo();

    // Make several commits
    for i in 2..=5 {
        let status = Command::new("git")
            .args([
                "commit",
                "--allow-empty",
                "-m",
                &format!("commit {}", i),
            ])
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    // Count C git reflog entries
    let output = Command::new("git")
        .args(["reflog", "show", "--format=%H", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c_count = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .count();

    let store = FilesRefStore::new(&git_dir);
    let head = RefName::new("HEAD").unwrap();
    let entries = store.reflog(&head).unwrap();

    assert_eq!(entries.len(), c_count);
}

// ── US4: Gitr reflog entries readable ───────────────────────────────────────

#[test]
fn gitr_creates_valid_reflog() {
    let (dir, git_dir) = setup_git_repo();
    let store = make_store(&git_dir);

    let head_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let oid = ObjectId::from_hex(&head_oid_hex).unwrap();

    let name = RefName::new("refs/heads/reflog-test").unwrap();
    let mut tx = RefTransaction::new();
    tx.create(name.clone(), oid, "branch: Created from HEAD");
    store.commit_transaction(tx).unwrap();

    // Read reflog with gitr
    let entries = store.reflog(&name).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].old_oid.is_null());
    assert_eq!(entries[0].new_oid, oid);
    assert_eq!(
        entries[0].message,
        BString::from("branch: Created from HEAD")
    );
}

// ── US3: Enumerate refs matches C git ───────────────────────────────────────

#[test]
fn enumerate_branches_matches_c_git() {
    let (dir, git_dir) = setup_git_repo();

    // Create branches
    for name in &["alpha", "beta", "gamma"] {
        let status = Command::new("git")
            .args(["branch", name])
            .current_dir(dir.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    // Get branch list from C git
    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname)", "refs/heads/"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c_output = String::from_utf8(output.stdout).unwrap();
    let c_count = c_output.trim().lines().count();

    let store = FilesRefStore::new(&git_dir);
    let gitr_refs: Vec<String> = store
        .iter(Some("refs/heads/"))
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|r| r.name().to_string())
        .collect();

    // Both should have the same count
    assert_eq!(gitr_refs.len(), c_count);

    for gitr_ref in &gitr_refs {
        assert!(
            c_output.contains(gitr_ref),
            "gitr ref {} not found in C git output",
            gitr_ref
        );
    }
}

#[test]
fn enumerate_tags_matches_c_git() {
    let (dir, git_dir) = setup_git_repo();

    for name in &["v1.0", "v2.0", "v3.0"] {
        let status = Command::new("git")
            .args(["tag", name])
            .current_dir(dir.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname)", "refs/tags/"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c_output = String::from_utf8(output.stdout).unwrap();
    let c_count = c_output.trim().lines().count();

    let store = FilesRefStore::new(&git_dir);
    let gitr_refs: Vec<String> = store
        .iter(Some("refs/tags/"))
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|r| r.name().to_string())
        .collect();

    assert_eq!(gitr_refs.len(), c_count);

    for gitr_ref in &gitr_refs {
        assert!(
            c_output.contains(gitr_ref),
            "gitr ref {} not found in C git output",
            gitr_ref
        );
    }
}

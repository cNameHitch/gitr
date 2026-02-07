//! Integration tests: verify ref update interoperability with C git.
//!
//! These tests update refs with gitr and verify with C git,
//! then update with C git and verify with gitr.

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

fn test_sig() -> Signature {
    Signature {
        name: BString::from("Test User"),
        email: BString::from("test@example.com"),
        date: GitDate::new(1234567890, 0),
    }
}

fn make_store(git_dir: &std::path::Path) -> FilesRefStore {
    let mut store = FilesRefStore::new(git_dir);
    store.set_committer(test_sig());
    store
}

// ── US2: Create ref with gitr, verify with C git ────────────────────────────

#[test]
fn create_branch_readable_by_c_git() {
    let (dir, git_dir) = setup_git_repo();
    let store = make_store(&git_dir);

    let head_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let oid = ObjectId::from_hex(&head_oid_hex).unwrap();

    let name = RefName::new("refs/heads/gitr-branch").unwrap();
    let mut tx = RefTransaction::new();
    tx.create(name, oid, "branch: Created from HEAD");
    store.commit_transaction(tx).unwrap();

    // Verify with C git
    let c_oid = git_rev_parse(dir.path(), "refs/heads/gitr-branch");
    assert_eq!(c_oid, head_oid_hex);
}

#[test]
fn update_branch_readable_by_c_git() {
    let (dir, git_dir) = setup_git_repo();
    let store = make_store(&git_dir);

    // Make two commits
    let first_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let first_oid = ObjectId::from_hex(&first_oid_hex).unwrap();

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

    let second_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let second_oid = ObjectId::from_hex(&second_oid_hex).unwrap();

    // Create a branch pointing to first commit
    let name = RefName::new("refs/heads/test-update").unwrap();
    let mut tx = RefTransaction::new();
    tx.create(name.clone(), first_oid, "branch: Created");
    store.commit_transaction(tx).unwrap();

    // Update to second commit
    let mut tx = RefTransaction::new();
    tx.update(name, first_oid, second_oid, "branch: Updated");
    store.commit_transaction(tx).unwrap();

    // Verify with C git
    let c_oid = git_rev_parse(dir.path(), "refs/heads/test-update");
    assert_eq!(c_oid, second_oid_hex);
}

#[test]
fn delete_branch_verified_by_c_git() {
    let (dir, git_dir) = setup_git_repo();
    let store = make_store(&git_dir);

    let head_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let oid = ObjectId::from_hex(&head_oid_hex).unwrap();

    let name = RefName::new("refs/heads/to-delete").unwrap();
    let mut tx = RefTransaction::new();
    tx.create(name.clone(), oid, "branch: Created");
    store.commit_transaction(tx).unwrap();

    // Verify it exists
    assert!(git_rev_parse(dir.path(), "refs/heads/to-delete") == head_oid_hex);

    // Delete it
    let mut tx = RefTransaction::new();
    tx.delete(name, oid, "branch: Deleted");
    store.commit_transaction(tx).unwrap();

    // Verify it's gone via C git
    let output = Command::new("git")
        .args(["rev-parse", "refs/heads/to-delete"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "ref should not exist after delete");
}

// ── US2: Update with C git, verify with gitr ────────────────────────────────

#[test]
fn read_c_git_branch_update() {
    let (dir, git_dir) = setup_git_repo();

    // Create a branch with C git
    let status = Command::new("git")
        .args(["branch", "c-branch"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let c_oid = git_rev_parse(dir.path(), "refs/heads/c-branch");

    let store = FilesRefStore::new(&git_dir);
    let name = RefName::new("refs/heads/c-branch").unwrap();
    let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();
    assert_eq!(gitr_oid.to_hex(), c_oid);
}

// ── US2 Acceptance Scenario 3: CAS failure ──────────────────────────────────

#[test]
fn cas_failure_rejects_update() {
    let (dir, git_dir) = setup_git_repo();
    let store = make_store(&git_dir);

    let head_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let oid = ObjectId::from_hex(&head_oid_hex).unwrap();

    let name = RefName::new("refs/heads/cas-test").unwrap();
    let mut tx = RefTransaction::new();
    tx.create(name.clone(), oid, "branch: Created");
    store.commit_transaction(tx).unwrap();

    // Try to update with wrong old value
    let wrong_oid = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();
    let new_oid = ObjectId::from_hex("0000000000000000000000000000000000000002").unwrap();

    let mut tx = RefTransaction::new();
    tx.update(name.clone(), wrong_oid, new_oid, "should fail");
    let result = store.commit_transaction(tx);
    assert!(result.is_err(), "CAS should fail with wrong old value");

    // Original value should be unchanged
    let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();
    assert_eq!(gitr_oid, oid);
}

// ── US2 Acceptance Scenario 5: Transaction atomicity ────────────────────────

#[test]
fn transaction_multiple_refs() {
    let (dir, git_dir) = setup_git_repo();
    let store = make_store(&git_dir);

    let head_oid_hex = git_rev_parse(dir.path(), "HEAD");
    let oid = ObjectId::from_hex(&head_oid_hex).unwrap();

    let mut tx = RefTransaction::new();
    tx.create(
        RefName::new("refs/heads/branch-a").unwrap(),
        oid,
        "branch: Created",
    );
    tx.create(
        RefName::new("refs/heads/branch-b").unwrap(),
        oid,
        "branch: Created",
    );
    tx.create(
        RefName::new("refs/heads/branch-c").unwrap(),
        oid,
        "branch: Created",
    );
    store.commit_transaction(tx).unwrap();

    // All three should exist and be readable by C git
    for branch in &["branch-a", "branch-b", "branch-c"] {
        let c_oid = git_rev_parse(dir.path(), &format!("refs/heads/{}", branch));
        assert_eq!(c_oid, head_oid_hex, "branch {} mismatch", branch);
    }
}

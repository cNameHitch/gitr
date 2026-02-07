//! Integration tests: verify ref resolution interoperability with C git.
//!
//! These tests create refs with C git and resolve them with gitr,
//! ensuring identical behavior.

use std::process::Command;

use git_ref::{FilesRefStore, RefName, RefStore};

/// Create a temporary git repository and return (tempdir, git_dir path).
fn setup_git_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();

    let status = Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git init failed");

    // Create an initial commit so HEAD points to something
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
    assert!(status.success(), "git commit failed");

    let git_dir = dir.path().join(".git");
    (dir, git_dir)
}

/// Get the OID that a ref points to via C git.
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

/// Create a branch via C git.
fn git_branch(repo_dir: &std::path::Path, name: &str) {
    let status = Command::new("git")
        .args(["branch", name])
        .current_dir(repo_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git branch {} failed", name);
}

/// Create a tag via C git.
fn git_tag(repo_dir: &std::path::Path, name: &str) {
    let status = Command::new("git")
        .args(["tag", name])
        .current_dir(repo_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git tag {} failed", name);
}

// ── US1: Resolve refs created by C git ──────────────────────────────────────

#[test]
fn resolve_branch_ref() {
    let (dir, git_dir) = setup_git_repo();
    git_branch(dir.path(), "feature");

    let c_oid = git_rev_parse(dir.path(), "refs/heads/feature");

    let store = FilesRefStore::new(&git_dir);
    let name = RefName::new("refs/heads/feature").unwrap();
    let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();

    assert_eq!(gitr_oid.to_hex(), c_oid);
}

#[test]
fn resolve_head_symbolic() {
    let (dir, git_dir) = setup_git_repo();

    let c_oid = git_rev_parse(dir.path(), "HEAD");

    let store = FilesRefStore::new(&git_dir);
    let head = RefName::new("HEAD").unwrap();

    // Should resolve HEAD (symbolic -> refs/heads/main -> OID)
    let gitr_oid = store.resolve_to_oid(&head).unwrap().unwrap();
    assert_eq!(gitr_oid.to_hex(), c_oid);

    // resolve() should return the symbolic ref
    let reference = store.resolve(&head).unwrap().unwrap();
    assert!(reference.is_symbolic());
}

#[test]
fn resolve_detached_head() {
    let (dir, git_dir) = setup_git_repo();

    let c_oid = git_rev_parse(dir.path(), "HEAD");

    // Detach HEAD
    let status = Command::new("git")
        .args(["checkout", "--detach"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let store = FilesRefStore::new(&git_dir);
    let head = RefName::new("HEAD").unwrap();

    let reference = store.resolve(&head).unwrap().unwrap();
    assert!(reference.is_direct());

    let gitr_oid = store.resolve_to_oid(&head).unwrap().unwrap();
    assert_eq!(gitr_oid.to_hex(), c_oid);
}

#[test]
fn resolve_tag_ref() {
    let (dir, git_dir) = setup_git_repo();
    git_tag(dir.path(), "v1.0");

    let c_oid = git_rev_parse(dir.path(), "refs/tags/v1.0");

    let store = FilesRefStore::new(&git_dir);
    let name = RefName::new("refs/tags/v1.0").unwrap();
    let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();

    assert_eq!(gitr_oid.to_hex(), c_oid);
}

#[test]
fn resolve_nonexistent_ref() {
    let (_dir, git_dir) = setup_git_repo();
    let store = FilesRefStore::new(&git_dir);
    let name = RefName::new("refs/heads/nonexistent").unwrap();
    assert!(store.resolve_to_oid(&name).unwrap().is_none());
}

#[test]
fn resolve_multiple_branches() {
    let (dir, git_dir) = setup_git_repo();

    // Create several branches
    git_branch(dir.path(), "alpha");
    git_branch(dir.path(), "beta");
    git_branch(dir.path(), "gamma");

    let store = FilesRefStore::new(&git_dir);

    for branch in &["alpha", "beta", "gamma"] {
        let c_oid = git_rev_parse(dir.path(), &format!("refs/heads/{}", branch));
        let name = RefName::new(format!("refs/heads/{}", branch)).unwrap();
        let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();
        assert_eq!(gitr_oid.to_hex(), c_oid, "mismatch for branch {}", branch);
    }
}

#[test]
fn resolve_packed_ref() {
    let (dir, git_dir) = setup_git_repo();
    git_branch(dir.path(), "packed-branch");

    // Pack refs
    let status = Command::new("git")
        .args(["pack-refs", "--all"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let c_oid = git_rev_parse(dir.path(), "refs/heads/packed-branch");

    let store = FilesRefStore::new(&git_dir);
    let name = RefName::new("refs/heads/packed-branch").unwrap();
    let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();

    assert_eq!(gitr_oid.to_hex(), c_oid);
}

#[test]
fn resolve_after_second_commit() {
    let (dir, git_dir) = setup_git_repo();

    // Make a second commit
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

    let c_oid = git_rev_parse(dir.path(), "HEAD");

    let store = FilesRefStore::new(&git_dir);
    let head = RefName::new("HEAD").unwrap();
    let gitr_oid = store.resolve_to_oid(&head).unwrap().unwrap();

    assert_eq!(gitr_oid.to_hex(), c_oid);
}

// ── US1 Acceptance Scenario 5: Symbolic ref chain ───────────────────────────

#[test]
fn resolve_symbolic_ref_chain() {
    let (dir, git_dir) = setup_git_repo();

    let head_oid = git_rev_parse(dir.path(), "HEAD");

    // Create a symbolic ref chain: refs/test/a -> refs/test/b -> refs/heads/main_or_master
    // First find the actual default branch name
    let default_branch = {
        let output = Command::new("git")
            .args(["symbolic-ref", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };

    // Create chain: refs/test/b -> default branch, refs/test/a -> refs/test/b
    let status = Command::new("git")
        .args(["symbolic-ref", "refs/test/b", &default_branch])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new("git")
        .args(["symbolic-ref", "refs/test/a", "refs/test/b"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let store = FilesRefStore::new(&git_dir);
    let name = RefName::new("refs/test/a").unwrap();
    let gitr_oid = store.resolve_to_oid(&name).unwrap().unwrap();

    assert_eq!(gitr_oid.to_hex(), head_oid);
}

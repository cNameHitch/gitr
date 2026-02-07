//! Merge-base computation tests.
//!
//! Creates git repositories with known history graphs and verifies that
//! merge-base computation matches C git's `git merge-base`.

use std::path::Path;
use std::process::Command;

use git_hash::ObjectId;
use git_repository::Repository;
use git_revwalk::{merge_base, merge_base_one, is_ancestor};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "author@test.com")
        .env("GIT_COMMITTER_NAME", "Test Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@test.com")
        .output()
        .expect("failed to run git");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git {:?} failed: {}", args, stderr);
    }
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn git_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "author@test.com")
        .env("GIT_COMMITTER_NAME", "Test Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@test.com");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("failed to run git");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git {:?} failed: {}", args, stderr);
    }
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Create a diamond merge history:
///   A -> B -> D
///   A -> C -> D
/// Returns (A, B, C, D) OIDs.
fn create_diamond_repo(dir: &Path) -> (String, String, String, String) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // A
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    git(dir, &["add", "a.txt"]);
    git_env(
        dir,
        &["commit", "-m", "A"],
        &[
            ("GIT_AUTHOR_DATE", "1700000000 +0000"),
            ("GIT_COMMITTER_DATE", "1700000000 +0000"),
        ],
    );
    let a = git(dir, &["rev-parse", "HEAD"]);

    // Branch: feature
    git(dir, &["checkout", "-b", "feature"]);

    // C
    std::fs::write(dir.join("c.txt"), "c").unwrap();
    git(dir, &["add", "c.txt"]);
    git_env(
        dir,
        &["commit", "-m", "C"],
        &[
            ("GIT_AUTHOR_DATE", "1700001000 +0000"),
            ("GIT_COMMITTER_DATE", "1700001000 +0000"),
        ],
    );
    let c = git(dir, &["rev-parse", "HEAD"]);

    // Back to main, B
    git(dir, &["checkout", "main"]);
    std::fs::write(dir.join("b.txt"), "b").unwrap();
    git(dir, &["add", "b.txt"]);
    git_env(
        dir,
        &["commit", "-m", "B"],
        &[
            ("GIT_AUTHOR_DATE", "1700002000 +0000"),
            ("GIT_COMMITTER_DATE", "1700002000 +0000"),
        ],
    );
    let b = git(dir, &["rev-parse", "HEAD"]);

    // Merge -> D
    git_env(
        dir,
        &["merge", "feature", "-m", "D"],
        &[
            ("GIT_AUTHOR_DATE", "1700003000 +0000"),
            ("GIT_COMMITTER_DATE", "1700003000 +0000"),
        ],
    );
    let d = git(dir, &["rev-parse", "HEAD"]);

    (a, b, c, d)
}

#[test]
fn merge_base_diamond() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c, _d) = create_diamond_repo(dir.path());

    // git merge-base B C should return A
    let expected = git(dir.path(), &["merge-base", &b, &c]);

    let repo = Repository::open(dir.path()).unwrap();
    let b_oid = ObjectId::from_hex(&b).unwrap();
    let c_oid = ObjectId::from_hex(&c).unwrap();

    let bases = merge_base(&repo, &b_oid, &c_oid).unwrap();

    assert_eq!(bases.len(), 1, "should have exactly one merge base");
    assert_eq!(bases[0].to_hex(), expected, "merge base should be A");
    assert_eq!(bases[0].to_hex(), a, "merge base should be A");
}

#[test]
fn merge_base_one_returns_single() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c, _d) = create_diamond_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let b_oid = ObjectId::from_hex(&b).unwrap();
    let c_oid = ObjectId::from_hex(&c).unwrap();

    let base = merge_base_one(&repo, &b_oid, &c_oid).unwrap();

    assert!(base.is_some(), "should find a merge base");
    assert_eq!(base.unwrap().to_hex(), a, "merge base should be A");
}

#[test]
fn merge_base_same_commit() {
    let dir = tempfile::tempdir().unwrap();
    let (_a, b, _c, _d) = create_diamond_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let b_oid = ObjectId::from_hex(&b).unwrap();

    let bases = merge_base(&repo, &b_oid, &b_oid).unwrap();

    assert_eq!(bases.len(), 1);
    assert_eq!(bases[0].to_hex(), b, "merge base of X with X should be X");
}

#[test]
fn is_ancestor_direct_parent() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, _c, _d) = create_diamond_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let a_oid = ObjectId::from_hex(&a).unwrap();
    let b_oid = ObjectId::from_hex(&b).unwrap();

    assert!(
        is_ancestor(&repo, &a_oid, &b_oid).unwrap(),
        "A should be ancestor of B"
    );
    assert!(
        !is_ancestor(&repo, &b_oid, &a_oid).unwrap(),
        "B should NOT be ancestor of A"
    );
}

#[test]
fn is_ancestor_self() {
    let dir = tempfile::tempdir().unwrap();
    let (a, _b, _c, _d) = create_diamond_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let a_oid = ObjectId::from_hex(&a).unwrap();

    assert!(
        is_ancestor(&repo, &a_oid, &a_oid).unwrap(),
        "a commit should be its own ancestor"
    );
}

#[test]
fn is_ancestor_across_merge() {
    let dir = tempfile::tempdir().unwrap();
    let (a, _b, _c, d) = create_diamond_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let a_oid = ObjectId::from_hex(&a).unwrap();
    let d_oid = ObjectId::from_hex(&d).unwrap();

    assert!(
        is_ancestor(&repo, &a_oid, &d_oid).unwrap(),
        "A should be ancestor of D through merge"
    );
}

#[test]
fn merge_base_no_common_ancestor() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@test.com"]);

    // Create orphan branch with a separate history.
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    git(dir.path(), &["add", "a.txt"]);
    git(dir.path(), &["commit", "-m", "A"]);
    let a = git(dir.path(), &["rev-parse", "HEAD"]);

    // Create an orphan branch
    git(dir.path(), &["checkout", "--orphan", "orphan"]);
    git(dir.path(), &["rm", "-rf", "."]);
    std::fs::write(dir.path().join("b.txt"), "b").unwrap();
    git(dir.path(), &["add", "b.txt"]);
    git(dir.path(), &["commit", "-m", "B-orphan"]);
    let b = git(dir.path(), &["rev-parse", "HEAD"]);

    let repo = Repository::open(dir.path()).unwrap();
    let a_oid = ObjectId::from_hex(&a).unwrap();
    let b_oid = ObjectId::from_hex(&b).unwrap();

    let bases = merge_base(&repo, &a_oid, &b_oid).unwrap();
    assert!(bases.is_empty(), "no common ancestor for orphan branches");
}

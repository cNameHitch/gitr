//! Walk order tests.
//!
//! Creates git repositories with known history and verifies that RevWalk
//! produces commit lists matching `git rev-list` output.

use std::path::Path;
use std::process::Command;

use git_hash::ObjectId;
use git_repository::Repository;
use git_revwalk::{RevWalk, SortOrder, WalkOptions};

/// Helper: run a git command in the given directory and return stdout.
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

/// Helper: run git with custom env vars.
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

/// Create a simple linear history: A -> B -> C (C is HEAD).
fn create_linear_repo(dir: &Path) -> Vec<String> {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // Commit A
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

    // Commit B
    std::fs::write(dir.join("b.txt"), "b").unwrap();
    git(dir, &["add", "b.txt"]);
    git_env(
        dir,
        &["commit", "-m", "B"],
        &[
            ("GIT_AUTHOR_DATE", "1700001000 +0000"),
            ("GIT_COMMITTER_DATE", "1700001000 +0000"),
        ],
    );

    // Commit C
    std::fs::write(dir.join("c.txt"), "c").unwrap();
    git(dir, &["add", "c.txt"]);
    git_env(
        dir,
        &["commit", "-m", "C"],
        &[
            ("GIT_AUTHOR_DATE", "1700002000 +0000"),
            ("GIT_COMMITTER_DATE", "1700002000 +0000"),
        ],
    );

    // Get actual commit OIDs.
    let rev_list = git(dir, &["rev-list", "HEAD"]);
    rev_list.lines().map(String::from).collect()
}

/// Create a repo with a merge:
///   A -> B -> D (merge)
///   A -> C -/
fn create_merge_repo(dir: &Path) -> Vec<String> {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // Commit A
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

    // Branch: feature
    git(dir, &["checkout", "-b", "feature"]);

    // Commit C (on feature)
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

    // Back to main
    git(dir, &["checkout", "main"]);

    // Commit B (on main)
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

    // Merge feature into main -> D
    git_env(
        dir,
        &["merge", "feature", "-m", "D"],
        &[
            ("GIT_AUTHOR_DATE", "1700003000 +0000"),
            ("GIT_COMMITTER_DATE", "1700003000 +0000"),
        ],
    );

    // Get rev-list
    let rev_list = git(dir, &["rev-list", "HEAD"]);
    rev_list.lines().map(String::from).collect()
}

#[test]
fn linear_chronological_order() {
    let dir = tempfile::tempdir().unwrap();
    let expected = create_linear_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.set_sort(SortOrder::Chronological);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert_eq!(result, expected, "chronological order should match git rev-list");
}

#[test]
fn linear_reverse_order() {
    let dir = tempfile::tempdir().unwrap();
    let expected = create_linear_repo(dir.path());
    let mut expected_rev = expected.clone();
    expected_rev.reverse();

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.set_sort(SortOrder::Reverse);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert_eq!(result, expected_rev, "reverse order should be oldest first");
}

#[test]
fn linear_topological_order() {
    let dir = tempfile::tempdir().unwrap();
    let expected = create_linear_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.set_sort(SortOrder::Topological);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    // Topological: parents appear after children.
    // For a linear history, this should be the same as chronological.
    assert_eq!(result, expected, "topo order for linear history should match chronological");
}

#[test]
fn merge_chronological_order() {
    let dir = tempfile::tempdir().unwrap();
    let expected = create_merge_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.set_sort(SortOrder::Chronological);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert_eq!(result, expected, "chronological order should match git rev-list for merge history");
}

#[test]
fn merge_topological_order() {
    let dir = tempfile::tempdir().unwrap();
    let _ = create_merge_repo(dir.path());

    // Get git's topo order
    let expected: Vec<String> = git(dir.path(), &["rev-list", "--topo-order", "HEAD"])
        .lines()
        .map(String::from)
        .collect();

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.set_sort(SortOrder::Topological);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    // In topological order, parents must appear after children.
    // Verify this invariant.
    for (i, oid_hex) in result.iter().enumerate() {
        let oid = ObjectId::from_hex(oid_hex).unwrap();
        let obj = repo.odb().read(&oid).unwrap().unwrap();
        if let git_object::Object::Commit(commit) = obj {
            for parent in &commit.parents {
                let parent_hex = parent.to_hex();
                if let Some(parent_pos) = result.iter().position(|h| *h == parent_hex) {
                    assert!(
                        parent_pos > i,
                        "parent {} at position {} should appear after child {} at position {}",
                        parent_hex, parent_pos, oid_hex, i
                    );
                }
            }
        }
    }

    assert_eq!(result.len(), expected.len(), "should produce same number of commits");
}

#[test]
fn first_parent_only() {
    let dir = tempfile::tempdir().unwrap();
    let _ = create_merge_repo(dir.path());

    // Get git's first-parent output.
    let expected: Vec<String> = git(dir.path(), &["rev-list", "--first-parent", "HEAD"])
        .lines()
        .map(String::from)
        .collect();

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    let opts = WalkOptions {
        first_parent_only: true,
        ..WalkOptions::default()
    };
    walk.set_options(opts);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert_eq!(result, expected, "first-parent walk should match git rev-list --first-parent");
}

#[test]
fn max_count_limits_output() {
    let dir = tempfile::tempdir().unwrap();
    let _ = create_linear_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    let opts = WalkOptions {
        max_count: Some(2),
        ..WalkOptions::default()
    };
    walk.set_options(opts);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert_eq!(result.len(), 2, "max_count should limit output");
}

#[test]
fn skip_commits() {
    let dir = tempfile::tempdir().unwrap();
    let all = create_linear_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    let opts = WalkOptions {
        skip: Some(1),
        ..WalkOptions::default()
    };
    walk.set_options(opts);
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert_eq!(result, all[1..], "skip=1 should skip first commit");
}

#[test]
fn empty_repo_produces_no_commits() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.push_head().unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert!(result.is_empty(), "empty repo should produce no commits");
}

#[test]
fn push_specific_commit() {
    let dir = tempfile::tempdir().unwrap();
    let all = create_linear_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();

    // Push the middle commit (B)
    let b_oid = ObjectId::from_hex(&all[1]).unwrap();
    walk.push(b_oid).unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    // Should get B and A (not C since we didn't push it).
    assert_eq!(result, &all[1..], "pushing B should walk B and A");
}

#[test]
fn hide_excludes_ancestors() {
    let dir = tempfile::tempdir().unwrap();
    let all = create_linear_repo(dir.path());

    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.push_head().unwrap();

    // Hide A (oldest) — should exclude A from output.
    let a_oid = ObjectId::from_hex(&all[2]).unwrap();
    walk.hide(a_oid).unwrap();

    let result: Vec<String> = walk
        .map(|r| r.unwrap().to_hex())
        .collect();

    assert!(!result.contains(&all[2]), "hidden commit should be excluded");
}

//! Integration tests for porcelain commands.
//!
//! These tests create temporary git repositories using C git, then run our
//! `gitr` binary against them and verify the output/behavior matches.

mod common;
use common::*;

/// Create a test repo with some content using C git.
fn setup_test_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test Author"]);
    git(dir, &["config", "user.email", "test@example.com"]);

    std::fs::write(dir.join("hello.txt"), "hello world\n").unwrap();
    std::fs::write(dir.join("foo.txt"), "foo content\n").unwrap();

    git(dir, &["add", "hello.txt", "foo.txt"]);
    git(dir, &["commit", "-m", "initial commit"]);
}

// ============== init tests ==============

#[test]
fn init_creates_git_dir() {
    let dir = tempfile::tempdir().unwrap();
    let repo_dir = dir.path().join("new-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let result = gitr(repo_dir.as_path(), &["init"]);
    assert_eq!(result.exit_code, 0);
    assert!(repo_dir.join(".git").exists(), ".git directory should exist");
    assert!(
        repo_dir.join(".git/HEAD").exists(),
        "HEAD file should exist"
    );
    assert!(
        repo_dir.join(".git/objects").exists(),
        "objects dir should exist"
    );
    assert!(
        repo_dir.join(".git/refs").exists(),
        "refs dir should exist"
    );
}

#[test]
fn init_bare_creates_bare_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo_dir = dir.path().join("bare-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let result = gitr(repo_dir.as_path(), &["init", "--bare"]);
    assert_eq!(result.exit_code, 0);
    // Bare repo has HEAD directly, not in .git/
    assert!(repo_dir.join("HEAD").exists(), "HEAD should exist in bare repo");
    assert!(
        repo_dir.join("objects").exists(),
        "objects dir should exist in bare repo"
    );
}

// ============== add tests ==============

#[test]
fn add_stages_new_file() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Create a new file
    std::fs::write(dir.path().join("new.txt"), "new content\n").unwrap();

    // Stage it with gitr
    let result = gitr(dir.path(), &["add", "new.txt"]);
    assert_eq!(result.exit_code, 0);

    // Verify it's in the index via ls-files
    let out = gitr(dir.path(), &["ls-files"]);
    assert_eq!(out.exit_code, 0);
    assert!(out.stdout.contains("new.txt"), "new.txt should be in the index");
}

// ============== rm tests ==============

#[test]
fn rm_cached_removes_from_index() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Remove from index (keep on disk)
    let result = gitr(dir.path(), &["rm", "--cached", "foo.txt"]);
    assert_eq!(result.exit_code, 0);

    // Verify it's gone from the index
    let out = gitr(dir.path(), &["ls-files"]);
    assert_eq!(out.exit_code, 0);
    assert!(
        !out.stdout.contains("foo.txt"),
        "foo.txt should not be in the index"
    );

    // But should still exist on disk
    assert!(dir.path().join("foo.txt").exists());
}

// ============== status tests ==============

#[test]
fn status_clean_repo() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["status"]);
    assert_eq!(result.exit_code, 0);
}

#[test]
fn status_shows_untracked() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    std::fs::write(dir.path().join("untracked.txt"), "untracked\n").unwrap();

    let result = gitr(dir.path(), &["status"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("untracked.txt"),
        "status should mention untracked file"
    );
}

// ============== commit tests ==============

#[test]
fn commit_creates_new_commit() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Make a change and stage it
    std::fs::write(dir.path().join("hello.txt"), "updated content\n").unwrap();
    git(dir.path(), &["add", "hello.txt"]);

    // Commit with gitr
    let result = gitr(dir.path(), &["commit", "-m", "update hello"]);
    assert_eq!(result.exit_code, 0);

    // Verify via rev-parse that HEAD advanced
    let old_head = git(dir.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();
    let new_head = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    assert_ne!(old_head, new_head, "HEAD should have advanced");
}

// ============== branch tests ==============

#[test]
fn branch_list() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["branch"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("master") || result.stdout.contains("main"),
        "branch list should contain the default branch"
    );
}

#[test]
fn branch_create() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["branch", "feature-x"]);
    assert_eq!(result.exit_code, 0);

    // Verify the branch exists
    let out = gitr(dir.path(), &["show-ref", "--verify", "refs/heads/feature-x"]);
    assert_eq!(out.exit_code, 0);
    assert!(!out.stdout.trim().is_empty(), "feature-x branch should exist");
}

#[test]
fn branch_delete() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Create then delete
    git(dir.path(), &["branch", "to-delete"]);
    let result = gitr(dir.path(), &["branch", "-d", "to-delete"]);
    assert_eq!(result.exit_code, 0);

    // Verify the branch is gone
    let out = gitr(dir.path(), &["show-ref", "--verify", "refs/heads/to-delete"]);
    assert_ne!(out.exit_code, 0, "deleted branch should not exist");
}

// ============== switch tests ==============

#[test]
fn switch_to_existing_branch() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    git(dir.path(), &["branch", "other"]);
    let result = gitr(dir.path(), &["switch", "other"]);
    assert_eq!(result.exit_code, 0);

    // Verify HEAD points to the new branch
    let head_ref = git(dir.path(), &["symbolic-ref", "HEAD"]).stdout.trim().to_string();
    assert_eq!(head_ref, "refs/heads/other");
}

#[test]
fn switch_create_new_branch() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["switch", "--create", "new-branch"]);
    assert_eq!(result.exit_code, 0);

    let head_ref = git(dir.path(), &["symbolic-ref", "HEAD"]).stdout.trim().to_string();
    assert_eq!(head_ref, "refs/heads/new-branch");
}

// ============== tag tests ==============

#[test]
fn tag_lightweight() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["tag", "v1.0"]);
    assert_eq!(result.exit_code, 0);

    // Verify tag exists
    let out = gitr(dir.path(), &["show-ref", "--verify", "refs/tags/v1.0"]);
    assert_eq!(out.exit_code, 0);
    assert!(!out.stdout.trim().is_empty(), "v1.0 tag should exist");
}

#[test]
fn tag_list() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    git(dir.path(), &["tag", "v1.0"]);
    git(dir.path(), &["tag", "v2.0"]);

    let out = gitr(dir.path(), &["tag"]);
    assert_eq!(out.exit_code, 0);
    assert!(out.stdout.contains("v1.0"), "should list v1.0");
    assert!(out.stdout.contains("v2.0"), "should list v2.0");
}

#[test]
fn tag_delete() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    git(dir.path(), &["tag", "v1.0"]);
    let result = gitr(dir.path(), &["tag", "-d", "v1.0"]);
    assert_eq!(result.exit_code, 0);

    let out = gitr(dir.path(), &["show-ref", "--verify", "refs/tags/v1.0"]);
    assert_ne!(out.exit_code, 0, "deleted tag should not exist");
}

// ============== reset tests ==============

#[test]
fn reset_soft_keeps_changes() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Make a second commit
    std::fs::write(dir.path().join("hello.txt"), "updated\n").unwrap();
    git(dir.path(), &["add", "hello.txt"]);
    git(dir.path(), &["commit", "-m", "second commit"]);

    let head_before = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let first_commit = git(dir.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();

    let result = gitr(dir.path(), &["reset", "--soft", "HEAD~1"]);
    assert_eq!(result.exit_code, 0);

    let head_after = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    assert_eq!(head_after, first_commit, "HEAD should point to first commit");
    assert_ne!(head_after, head_before);

    // Working tree should still have the updated content
    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(content, "updated\n");
}

#[test]
fn reset_hard_discards_changes() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Make a second commit
    std::fs::write(dir.path().join("hello.txt"), "updated\n").unwrap();
    git(dir.path(), &["add", "hello.txt"]);
    git(dir.path(), &["commit", "-m", "second commit"]);

    let result = gitr(dir.path(), &["reset", "--hard", "HEAD~1"]);
    assert_eq!(result.exit_code, 0);

    // Working tree should be restored to original
    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(content, "hello world\n");
}

// ============== mv tests ==============

#[test]
fn mv_renames_file() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["mv", "foo.txt", "bar.txt"]);
    assert_eq!(result.exit_code, 0);

    assert!(!dir.path().join("foo.txt").exists(), "old file should be gone");
    assert!(dir.path().join("bar.txt").exists(), "new file should exist");

    let out = gitr(dir.path(), &["ls-files"]);
    assert!(out.stdout.contains("bar.txt"), "index should contain bar.txt");
    assert!(!out.stdout.contains("foo.txt"), "index should not contain foo.txt");
}

// ============== clean tests ==============

#[test]
fn clean_removes_untracked() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    std::fs::write(dir.path().join("untracked.txt"), "junk\n").unwrap();
    assert!(dir.path().join("untracked.txt").exists());

    let result = gitr(dir.path(), &["clean", "-f"]);
    assert_eq!(result.exit_code, 0);

    assert!(
        !dir.path().join("untracked.txt").exists(),
        "untracked file should be removed"
    );
}

#[test]
fn clean_dry_run_does_not_remove() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    std::fs::write(dir.path().join("untracked.txt"), "junk\n").unwrap();

    let result = gitr(dir.path(), &["clean", "-n"]);
    assert_eq!(result.exit_code, 0);

    assert!(
        dir.path().join("untracked.txt").exists(),
        "dry run should not remove files"
    );
}

// ============== restore tests ==============

#[test]
fn restore_staged_unstages_file() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Modify and stage
    std::fs::write(dir.path().join("hello.txt"), "modified\n").unwrap();
    git(dir.path(), &["add", "hello.txt"]);

    // Unstage with restore --staged
    let result = gitr(dir.path(), &["restore", "--staged", "hello.txt"]);
    assert_eq!(result.exit_code, 0);

    // The file should still be modified in the working tree
    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(content, "modified\n");
}

// ============== write-tree + commit-tree round-trip ==============

#[test]
fn write_tree_and_commit_tree_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // write-tree with gitr
    let tree_result = gitr(dir.path(), &["write-tree"]);
    assert_eq!(tree_result.exit_code, 0);
    let tree_oid = tree_result.stdout.trim();

    // compare with C git
    let expected_tree = git(dir.path(), &["write-tree"]);
    assert_eq!(tree_oid, expected_tree.stdout.trim());

    // commit-tree with gitr
    let head = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let commit_result = gitr(
        dir.path(),
        &["commit-tree", tree_oid, "-p", &head, "-m", "test commit"],
    );
    assert_eq!(commit_result.exit_code, 0);
    assert!(!commit_result.stdout.trim().is_empty(), "should produce a commit OID");
}

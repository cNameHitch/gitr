//! Integration tests for plumbing commands.
//!
//! These tests create temporary git repositories using C git, then run our
//! `gitr` binary against them and verify the output matches C git's output.

mod common;
use common::*;

use std::process::Command;

/// Create a test repo with some content.
fn setup_test_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test Author"]);
    git(dir, &["config", "user.email", "test@example.com"]);

    std::fs::write(dir.join("hello.txt"), "hello world\n").unwrap();
    std::fs::write(dir.join("foo.txt"), "foo content\n").unwrap();

    git(dir, &["add", "hello.txt", "foo.txt"]);
    git(dir, &["commit", "-m", "initial commit"]);
}

// ============== hash-object tests ==============

#[test]
fn hash_object_stdin() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let c_git_output = Command::new("git")
        .args(["hash-object", "--stdin"])
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"hello\n").unwrap();
            child.wait_with_output()
        })
        .unwrap();
    let expected = String::from_utf8_lossy(&c_git_output.stdout).trim().to_string();

    let gitr_output = Command::new(gitr_bin())
        .args(["hash-object", "--stdin"])
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(b"hello\n").unwrap();
            child.wait_with_output()
        })
        .unwrap();
    let actual = String::from_utf8_lossy(&gitr_output.stdout).trim().to_string();

    assert_eq!(actual, expected, "hash-object --stdin mismatch");
}

#[test]
fn hash_object_file() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["hash-object", "hello.txt"]);
    let result = gitr(dir.path(), &["hash-object", "hello.txt"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "hash-object file mismatch");
}

// ============== cat-file tests ==============

#[test]
fn cat_file_type() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let oid = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    let expected = git(dir.path(), &["cat-file", "-t", &oid]);
    let result = gitr(dir.path(), &["cat-file", "-t", &oid]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "cat-file -t mismatch");
}

#[test]
fn cat_file_size() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let oid = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    let expected = git(dir.path(), &["cat-file", "-s", &oid]);
    let result = gitr(dir.path(), &["cat-file", "-s", &oid]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "cat-file -s mismatch");
}

#[test]
fn cat_file_pretty_blob() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let oid = git(dir.path(), &["hash-object", "hello.txt"]).stdout.trim().to_string();

    let expected = git(dir.path(), &["cat-file", "-p", &oid]);
    let result = gitr(dir.path(), &["cat-file", "-p", &oid]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, expected.stdout, "cat-file -p blob mismatch");
}

// ============== rev-parse tests ==============

#[test]
fn rev_parse_head() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["rev-parse", "HEAD"]);
    let result = gitr(dir.path(), &["rev-parse", "HEAD"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "rev-parse HEAD mismatch");
}

#[test]
fn rev_parse_git_dir() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["rev-parse", "--git-dir"]);
    let result = gitr(dir.path(), &["rev-parse", "--git-dir"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "rev-parse --git-dir mismatch");
}

#[test]
fn rev_parse_is_bare() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["rev-parse", "--is-bare-repository"]);
    let result = gitr(dir.path(), &["rev-parse", "--is-bare-repository"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "rev-parse --is-bare-repository mismatch");
}

// ============== show-ref tests ==============

#[test]
fn show_ref_basic() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["show-ref"]);
    let result = gitr(dir.path(), &["show-ref"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "show-ref mismatch");
}

// ============== symbolic-ref tests ==============

#[test]
fn symbolic_ref_read_head() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["symbolic-ref", "HEAD"]);
    let result = gitr(dir.path(), &["symbolic-ref", "HEAD"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "symbolic-ref HEAD mismatch");
}

// ============== ls-files tests ==============

#[test]
fn ls_files_cached() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["ls-files"]);
    let result = gitr(dir.path(), &["ls-files"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "ls-files mismatch");
}

#[test]
fn ls_files_stage() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["ls-files", "--stage"]);
    let result = gitr(dir.path(), &["ls-files", "--stage"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "ls-files --stage mismatch");
}

// ============== ls-tree tests ==============

#[test]
fn ls_tree_head() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["ls-tree", "HEAD"]);
    let result = gitr(dir.path(), &["ls-tree", "HEAD"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "ls-tree HEAD mismatch");
}

#[test]
fn ls_tree_recursive() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    // Add a subdirectory
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/bar.txt"), "bar content\n").unwrap();
    git(dir.path(), &["add", "sub/bar.txt"]);
    git(dir.path(), &["commit", "-m", "add subdir"]);

    let expected = git(dir.path(), &["ls-tree", "-r", "HEAD"]);
    let result = gitr(dir.path(), &["ls-tree", "-r", "HEAD"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "ls-tree -r HEAD mismatch");
}

#[test]
fn ls_tree_name_only() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["ls-tree", "--name-only", "HEAD"]);
    let result = gitr(dir.path(), &["ls-tree", "--name-only", "HEAD"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "ls-tree --name-only mismatch");
}

// ============== check-ref-format tests ==============

#[test]
fn check_ref_format_valid() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["check-ref-format", "refs/heads/main"]);
    assert_eq!(result.exit_code, 0, "valid ref format should exit 0");
}

#[test]
fn check_ref_format_invalid() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["check-ref-format", "invalid..name"]);
    assert_eq!(result.exit_code, 1, "invalid ref format should exit 1");
}

// ============== var tests ==============

#[test]
fn var_editor() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let result = gitr(dir.path(), &["var", "GIT_EDITOR"]);
    assert_eq!(result.exit_code, 0);
    assert!(!result.stdout.trim().is_empty(), "GIT_EDITOR should not be empty");
}

// ============== write-tree tests ==============

#[test]
fn write_tree_matches_git() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let expected = git(dir.path(), &["write-tree"]);
    let result = gitr(dir.path(), &["write-tree"]);

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), expected.stdout.trim(), "write-tree mismatch");
}

// ============== commit-tree tests ==============

#[test]
fn commit_tree_basic() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let tree = git(dir.path(), &["write-tree"]).stdout.trim().to_string();

    // Create a commit with gitr
    let result = gitr(dir.path(), &["commit-tree", &tree, "-m", "test commit"]);
    assert_eq!(result.exit_code, 0);

    // Verify the created object exists
    let oid = result.stdout.trim();
    let cat_result = gitr(dir.path(), &["cat-file", "-t", oid]);
    assert_eq!(cat_result.exit_code, 0);
    assert_eq!(cat_result.stdout.trim(), "commit");
}

// ============== update-ref / show-ref round-trip ==============

#[test]
fn update_ref_create_and_show() {
    let dir = tempfile::tempdir().unwrap();
    setup_test_repo(dir.path());

    let oid = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // Create a new ref with gitr
    let result = gitr(dir.path(), &["update-ref", "refs/heads/test-branch", &oid]);
    assert_eq!(result.exit_code, 0);

    // Verify it exists
    let out = gitr(dir.path(), &["show-ref", "--verify", "refs/heads/test-branch"]);
    assert_eq!(out.exit_code, 0);
    assert!(out.stdout.contains(&oid), "show-ref should contain the OID");
}

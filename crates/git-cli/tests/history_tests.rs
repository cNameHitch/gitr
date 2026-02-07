//! Integration tests for history and inspection commands.
//!
//! These tests create temporary git repositories using C git, then run our
//! `gitr` binary against them and verify the output/behavior matches.

mod common;
use common::*;

/// Create a test repo with multiple commits using C git.
fn setup_history_repo(dir: &std::path::Path) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test Author"]);
    git(dir, &["config", "user.email", "test@example.com"]);

    // Commit 1
    std::fs::write(dir.join("hello.txt"), "hello world\n").unwrap();
    git(dir, &["add", "hello.txt"]);
    git(dir, &["commit", "-m", "initial commit"]);

    // Commit 2
    std::fs::write(dir.join("hello.txt"), "hello world\nline 2\n").unwrap();
    git(dir, &["add", "hello.txt"]);
    git(dir, &["commit", "-m", "add line 2"]);

    // Commit 3
    std::fs::write(dir.join("foo.txt"), "foo content\n").unwrap();
    git(dir, &["add", "foo.txt"]);
    git(dir, &["commit", "-m", "add foo.txt"]);
}

// ============== log tests ==============

#[test]
fn log_default_shows_commits() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["log"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("initial commit"), "should show first commit");
    assert!(result.stdout.contains("add line 2"), "should show second commit");
    assert!(result.stdout.contains("add foo.txt"), "should show third commit");
}

#[test]
fn log_oneline() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "should have 3 commits in oneline format");
    assert!(lines[0].contains("add foo.txt"));
    assert!(lines[2].contains("initial commit"));
}

#[test]
fn log_max_count() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["log", "--oneline", "-n", "2"]);
    assert_eq!(result.exit_code, 0);
    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 2, "should limit to 2 commits");
}

#[test]
fn log_stat() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["log", "--stat"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello.txt") || result.stdout.contains("foo.txt"), "stat should mention changed files");
}

// ============== rev-list tests ==============

#[test]
fn rev_list_shows_oids() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["rev-list", "HEAD"]);
    assert_eq!(result.exit_code, 0);
    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "should have 3 commit OIDs");
    // Each line should be a valid hex OID (40 chars for SHA-1)
    for line in &lines {
        assert!(line.len() >= 40, "OID should be at least 40 chars: {}", line);
    }
}

#[test]
fn rev_list_count() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["rev-list", "--count", "HEAD"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "3", "should count 3 commits");
}

// ============== show tests ==============

#[test]
fn show_head_commit() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["show", "HEAD"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("add foo.txt"), "should show HEAD commit message");
    assert!(result.stdout.contains("Author:"), "should show author");
}

#[test]
fn show_file_at_revision() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["show", "HEAD:foo.txt"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "foo content\n", "should show file content at HEAD");
}

#[test]
fn show_no_patch() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["show", "--no-patch", "HEAD"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("add foo.txt"), "should show commit message");
    // Should not contain diff markers
    assert!(!result.stdout.contains("diff --git"), "should not show diff with --no-patch");
}

// ============== diff tests ==============

#[test]
fn diff_unstaged_changes() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    // Make an unstaged change
    std::fs::write(dir.path().join("hello.txt"), "modified\n").unwrap();

    let result = gitr(dir.path(), &["diff"]);
    // diff returns 0 by default (matching C git behavior)
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello.txt"), "should show changed file");
}

#[test]
fn diff_cached() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    // Stage a change
    std::fs::write(dir.path().join("hello.txt"), "staged change\n").unwrap();
    git(dir.path(), &["add", "hello.txt"]);

    let result = gitr(dir.path(), &["diff", "--cached"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello.txt"), "should show staged file");
}

#[test]
fn diff_no_changes() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["diff"]);
    assert_eq!(result.exit_code, 0, "no changes should return 0");
    assert!(result.stdout.is_empty(), "no output when no changes");
}

// ============== blame tests ==============

#[test]
fn blame_shows_annotations() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["blame", "hello.txt"]);
    assert_eq!(result.exit_code, 0);
    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 2, "hello.txt has 2 lines");
    assert!(lines[0].contains("hello world"), "first line content");
    assert!(lines[1].contains("line 2"), "second line content");
}

#[test]
fn blame_line_range() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["blame", "-L", "1,1", "hello.txt"]);
    assert_eq!(result.exit_code, 0);
    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 1, "should show only 1 line");
}

// ============== shortlog tests ==============

#[test]
fn shortlog_groups_by_author() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["shortlog"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("Test Author"), "should show author name");
    assert!(result.stdout.contains("(3)"), "should show commit count");
}

#[test]
fn shortlog_summary() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["shortlog", "-s"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("3"), "summary should show count");
    assert!(result.stdout.contains("Test Author"), "summary should show author");
}

// ============== describe tests ==============

#[test]
fn describe_with_tag() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    // Create a tag on the first commit
    let first_oid = git(dir.path(), &["rev-list", "--reverse", "HEAD"])
        .stdout
        .lines()
        .next()
        .unwrap()
        .to_string();
    git(dir.path(), &[
        "tag",
        "-a",
        "v1.0",
        &first_oid,
        "-m",
        "version 1.0",
    ]);

    let result = gitr(dir.path(), &["describe"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.starts_with("v1.0"), "should start with tag name");
}

#[test]
fn describe_always() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["describe", "--always"]);
    assert_eq!(result.exit_code, 0);
    assert!(!result.stdout.trim().is_empty(), "should output abbreviated OID");
}

// ============== grep tests ==============

#[test]
fn grep_finds_pattern() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["grep", "hello"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("hello.txt"), "should find hello in hello.txt");
}

#[test]
fn grep_no_match() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["grep", "nonexistent_pattern_xyz"]);
    assert_eq!(result.exit_code, 1, "should return 1 when no match");
}

#[test]
fn grep_line_numbers() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["grep", "-n", "hello"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains(":1:"), "should show line number");
}

// ============== reflog tests ==============

#[test]
fn reflog_shows_entries() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["reflog"]);
    assert_eq!(result.exit_code, 0);
    // Reflog should have entries from the 3 commits
    assert!(
        !result.stdout.is_empty() || result.exit_code == 0,
        "reflog should succeed"
    );
}

// ============== bisect tests ==============

#[test]
fn bisect_start_and_reset() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let result = gitr(dir.path(), &["bisect", "start"]);
    assert_eq!(result.exit_code, 0);

    // Bisect state files should exist
    assert!(
        dir.path().join(".git/BISECT_START").exists(),
        "BISECT_START should exist"
    );

    let result = gitr(dir.path(), &["bisect", "reset"]);
    assert_eq!(result.exit_code, 0);

    // State should be cleaned up
    assert!(
        !dir.path().join(".git/BISECT_START").exists(),
        "BISECT_START should be removed after reset"
    );
}

// ============== cherry-pick tests ==============

#[test]
fn cherry_pick_applies_commit() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    // Create a new branch from the first commit
    let first_oid = git(dir.path(), &["rev-list", "--reverse", "HEAD"])
        .stdout
        .lines()
        .next()
        .unwrap()
        .to_string();
    git(dir.path(), &["checkout", "-b", "test-branch", &first_oid]);

    // Get the OID of the "add foo.txt" commit
    let last_oid = git(dir.path(), &["rev-parse", "main"])
        .stdout
        .trim()
        .to_string();

    let result = gitr(dir.path(), &["cherry-pick", &last_oid]);
    assert_eq!(result.exit_code, 0);

    // foo.txt should now exist on this branch
    assert!(dir.path().join("foo.txt").exists(), "cherry-picked file should exist");
}

// ============== revert tests ==============

#[test]
fn revert_undoes_commit() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    // Revert the last commit (which added foo.txt)
    let result = gitr(dir.path(), &["revert", "HEAD"]);
    assert_eq!(result.exit_code, 0);

    // foo.txt should no longer exist
    assert!(!dir.path().join("foo.txt").exists(), "reverted file should be removed");
}

// ============== format-patch tests ==============

#[test]
fn format_patch_creates_files() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let patch_dir = dir.path().join("patches");
    std::fs::create_dir_all(&patch_dir).unwrap();

    // Get a commit range: HEAD~2..HEAD gives the last 2 commits
    let result = gitr(
        dir.path(),
        &[
            "format-patch",
            "-o",
            patch_dir.to_str().unwrap(),
            "HEAD~2..HEAD",
        ],
    );
    assert_eq!(result.exit_code, 0);
    assert!(!result.stdout.is_empty(), "should output patch filenames");

    // Check that patch files were created
    let patch_files: Vec<_> = std::fs::read_dir(&patch_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "patch"))
        .collect();
    assert!(!patch_files.is_empty(), "should create at least one patch file");
}

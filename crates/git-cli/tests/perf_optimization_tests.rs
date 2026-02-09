//! Integration tests for 022-perf-optimization: commit-graph accelerated history.
//!
//! Verifies that `gitr log` and `gitr rev-list --all` produce byte-identical
//! output to C git on repos with commit-graph files.

mod common;

use common::{assert_output_eq, assert_stdout_eq, git, git_with_date, gitr, next_date, setup_empty_repo};
use tempfile::TempDir;

/// Build a repo with 500+ commits (for commit-graph acceleration testing).
fn setup_large_history(dir: &std::path::Path, num_commits: usize) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    for i in 0..num_commits {
        let filename = format!("file_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        git_with_date(dir, &["commit", "-m", &format!("commit {}", i)], &date);
    }
}

#[test]
fn log_with_commit_graph_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 100);

    // Generate commit-graph with C git
    git(dir.path(), &["commit-graph", "write"]);

    let git_result = git(dir.path(), &["log"]);
    let gitr_result = gitr(dir.path(), &["log"]);
    assert_output_eq(&git_result, &gitr_result);
}

#[test]
fn rev_list_all_with_commit_graph_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 100);

    // Generate commit-graph with C git
    git(dir.path(), &["commit-graph", "write"]);

    let git_result = git(dir.path(), &["rev-list", "--all"]);
    let gitr_result = gitr(dir.path(), &["rev-list", "--all"]);
    assert_output_eq(&git_result, &gitr_result);
}

#[test]
fn log_without_commit_graph_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 100);

    // No commit-graph — verifies graceful fallback
    let git_result = git(dir.path(), &["log"]);
    let gitr_result = gitr(dir.path(), &["log"]);
    assert_output_eq(&git_result, &gitr_result);
}

#[test]
fn rev_list_count_with_commit_graph_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 100);

    // Generate commit-graph with C git
    git(dir.path(), &["commit-graph", "write"]);

    let git_result = git(dir.path(), &["rev-list", "--count", "HEAD"]);
    let gitr_result = gitr(dir.path(), &["rev-list", "--count", "HEAD"]);
    assert_output_eq(&git_result, &gitr_result);
}

#[test]
fn log_oneline_with_commit_graph_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 100);

    // Generate commit-graph with C git
    git(dir.path(), &["commit-graph", "write"]);

    let git_result = git(dir.path(), &["log", "--oneline"]);
    let gitr_result = gitr(dir.path(), &["log", "--oneline"]);
    assert_output_eq(&git_result, &gitr_result);
}

#[test]
fn log_with_branches_and_commit_graph_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_empty_repo(dir.path());
    let mut counter = 0u64;

    // Create 50 commits on main
    for i in 0..50 {
        let filename = format!("file_{}.txt", i);
        std::fs::write(dir.path().join(&filename), format!("content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir.path(), &["add", &filename], &date);
        git_with_date(
            dir.path(),
            &["commit", "-m", &format!("commit {}", i)],
            &date,
        );
    }

    // Create a branch from an earlier point
    git(dir.path(), &["branch", "feature", "HEAD~25"]);

    // Generate commit-graph with C git
    git(dir.path(), &["commit-graph", "write"]);

    // Test log with range exclusion (uses mark_hidden via A..B range)
    let git_result = git(dir.path(), &["log", "feature..main"]);
    let gitr_result = gitr(dir.path(), &["log", "feature..main"]);
    assert_output_eq(&git_result, &gitr_result);
}

// ──────────────────────────── Commit-graph write/verify tests (US4) ────────────────

#[test]
fn gitr_commit_graph_write_verified_by_git() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 50);

    // Write commit-graph with gitr
    gitr(dir.path(), &["commit-graph", "write"]);

    // Verify the commit-graph file exists
    let graph_path = dir.path().join(".git/objects/info/commit-graph");
    assert!(graph_path.exists(), "commit-graph file should exist after gitr commit-graph write");

    // Verify with C git
    let verify_result = git(dir.path(), &["commit-graph", "verify"]);
    assert_eq!(
        verify_result.exit_code, 0,
        "C git commit-graph verify should succeed on gitr-written graph: stderr={}",
        verify_result.stderr
    );
}

#[test]
fn gitr_commit_graph_verify_on_git_written_graph() {
    let dir = TempDir::new().unwrap();
    setup_large_history(dir.path(), 50);

    // Write commit-graph with C git (--reachable to ensure file is actually written)
    git(dir.path(), &["commit-graph", "write", "--reachable"]);

    // Verify with gitr
    let verify_result = gitr(dir.path(), &["commit-graph", "verify"]);
    assert_eq!(
        verify_result.exit_code, 0,
        "gitr commit-graph verify should succeed on C git-written graph: stderr={}",
        verify_result.stderr
    );
}

// ──────────────────────────── Status parity tests (US2) ────────────────────────────

#[test]
fn status_with_many_files_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_empty_repo(dir.path());
    let mut counter = 0u64;

    // Create 100+ files in initial commit
    for i in 0..120 {
        let subdir = format!("dir_{}", i % 10);
        std::fs::create_dir_all(dir.path().join(&subdir)).unwrap();
        let filename = format!("{}/file_{}.txt", subdir, i);
        std::fs::write(dir.path().join(&filename), format!("content {}\n", i)).unwrap();
    }
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "."], &date);
    git_with_date(dir.path(), &["commit", "-m", "initial files"], &date);

    // Modify a subset of files
    for i in [3, 17, 42, 78, 99] {
        let filename = format!("dir_{}/file_{}.txt", i % 10, i);
        std::fs::write(
            dir.path().join(&filename),
            format!("modified content {}\n", i),
        )
        .unwrap();
    }

    // Delete some files
    for i in [5, 50] {
        let filename = format!("dir_{}/file_{}.txt", i % 10, i);
        std::fs::remove_file(dir.path().join(&filename)).unwrap();
    }

    // Add untracked files
    std::fs::write(dir.path().join("untracked_1.txt"), "new file\n").unwrap();
    std::fs::write(dir.path().join("untracked_2.txt"), "another new file\n").unwrap();

    // Use --short format for reliable comparison (long format has whitespace variations)
    let git_result = git(dir.path(), &["status", "--short"]);
    let gitr_result = gitr(dir.path(), &["status", "--short"]);
    assert_stdout_eq(&git_result, &gitr_result);
}

// ──────────────────────────── Blame parity tests (US3) ────────────────────────────

#[test]
fn blame_single_commit_file_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_empty_repo(dir.path());
    let mut counter = 0u64;

    // Create a file with multiple lines in initial commit
    std::fs::write(
        dir.path().join("readme.txt"),
        "line 1\nline 2\nline 3\nline 4\nline 5\n",
    )
    .unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "readme.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "initial"], &date);

    // Blame should attribute all lines to the root commit
    let git_result = git(dir.path(), &["blame", "readme.txt"]);
    let gitr_result = gitr(dir.path(), &["blame", "readme.txt"]);
    assert_stdout_eq(&git_result, &gitr_result);
}

#[test]
fn blame_appended_lines_matches_git() {
    let dir = TempDir::new().unwrap();
    setup_empty_repo(dir.path());
    let mut counter = 0u64;

    // Create initial file
    std::fs::write(dir.path().join("log.txt"), "line 1\nline 2\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "log.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "initial"], &date);

    // Append more lines in subsequent commits
    for i in 3..8 {
        let mut content = std::fs::read_to_string(dir.path().join("log.txt")).unwrap();
        content.push_str(&format!("line {}\n", i));
        std::fs::write(dir.path().join("log.txt"), content).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir.path(), &["add", "log.txt"], &date);
        git_with_date(
            dir.path(),
            &["commit", "-m", &format!("add line {}", i)],
            &date,
        );
    }

    let git_result = git(dir.path(), &["blame", "log.txt"]);
    let gitr_result = gitr(dir.path(), &["blame", "log.txt"]);
    assert_stdout_eq(&git_result, &gitr_result);
}

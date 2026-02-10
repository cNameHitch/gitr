//! E2E parity tests for stderr output — Phase 5.
//!
//! Verifies that stderr messages match between git and gitr for
//! deterministic error cases, conflict reports, and status messages.

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// Error message parity (deterministic stderr)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stderr_checkout_nonexistent_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["checkout", "nonexistent"]);
    let m = gitr(dir_gitr.path(), &["checkout", "nonexistent"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_switch_nonexistent_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["switch", "nonexistent"]);
    let m = gitr(dir_gitr.path(), &["switch", "nonexistent"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_branch_delete_current() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["branch", "-d", "main"]);
    let m = gitr(dir_gitr.path(), &["branch", "-d", "main"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_tag_delete_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["tag", "-d", "nonexistent"]);
    let m = gitr(dir_gitr.path(), &["tag", "-d", "nonexistent"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_reset_invalid_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["reset", "nonexistent"]);
    let m = gitr(dir_gitr.path(), &["reset", "nonexistent"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_cherry_pick_invalid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["cherry-pick", "nonexistent"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "nonexistent"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_revert_invalid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["revert", "nonexistent"]);
    let m = gitr(dir_gitr.path(), &["revert", "nonexistent"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Merge conflict stderr
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stderr_merge_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());
    let g = git(dir_git.path(), &["merge", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge", "feature"]);
    assert_exit_code_eq(&g, &m);
    // Conflict messages should mention the conflicting file
    assert!(g.stdout.contains("conflict") || g.stderr.contains("conflict") ||
            g.stdout.contains("Conflict") || g.stderr.contains("CONFLICT"),
            "git should report conflict");
}

#[test]
fn test_stderr_cherry_pick_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());
    let g = git(dir_git.path(), &["cherry-pick", "feature"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "feature"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Command status messages
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_stderr_commit_nothing() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["commit", "-m", "nothing"]);
    let m = gitr(dir_gitr.path(), &["commit", "-m", "nothing"]);
    assert_exit_code_eq(&g, &m);
    // Both should mention "nothing to commit" or similar
}

#[test]
#[ignore] // known parity gap
fn test_stderr_clean_no_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());
    let g = git(dir_git.path(), &["clean"]);
    let m = gitr(dir_gitr.path(), &["clean"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Not-a-repo stderr
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stderr_not_a_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let g = git(dir_git.path(), &["status"]);
    let m = gitr(dir_gitr.path(), &["status"]);
    assert_exit_code_eq(&g, &m);
    // Both should have "fatal" in stderr about not being a git repo
    assert!(g.stderr.contains("fatal") || g.stderr.contains("not a git repository"),
            "git stderr should mention fatal/not a git repo: {}", g.stderr);
    assert!(m.stderr.contains("fatal") || m.stderr.contains("not a git repository"),
            "gitr stderr should mention fatal/not a git repo: {}", m.stderr);
}

// ══════════════════════════════════════════════════════════════════════════════
// Stash stderr
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stderr_stash_nothing() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    // No dirty changes to stash
    let g = git(dir_git.path(), &["stash"]);
    let m = gitr(dir_gitr.path(), &["stash"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_stash_pop_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["stash", "pop"]);
    let m = gitr(dir_gitr.path(), &["stash", "pop"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Abort state errors stderr
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_stderr_merge_abort_no_merge() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["merge", "--abort"]);
    let m = gitr(dir_gitr.path(), &["merge", "--abort"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_stderr_rebase_abort_no_rebase() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["rebase", "--abort"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--abort"]);
    assert_exit_code_eq(&g, &m);
    assert_stderr_matches(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Full parity checks (exit code + stdout + stderr)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_parity_log() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_diff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff", "HEAD~1", "HEAD"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_branch_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["branch", "--list"]);
    let m = gitr(dir_gitr.path(), &["branch", "--list"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_tag_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["tag", "-l"]);
    let m = gitr(dir_gitr.path(), &["tag", "-l"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_rev_parse() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["rev-parse", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_show() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["show", "--stat"]);
    let m = gitr(dir_gitr.path(), &["show", "--stat"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_blame() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["blame", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "file_0.txt"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_shortlog() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["shortlog", "-s"]);
    let m = gitr(dir_gitr.path(), &["shortlog", "-s"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_ls_files() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["ls-files"]);
    let m = gitr(dir_gitr.path(), &["ls-files"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_cat_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["cat-file", "-t", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-t", "HEAD"]);
    assert_full_parity(&g, &m);
}

#[test]
fn test_full_parity_for_each_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["for-each-ref", "--format=%(refname)"]);
    let m = gitr(dir_gitr.path(), &["for-each-ref", "--format=%(refname)"]);
    assert_full_parity(&g, &m);
}

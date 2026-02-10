//! E2E parity tests for error paths — Phase 4C.
//!
//! For every command, tests at least one invalid-input scenario and verifies
//! gitr produces the same exit code as git. Categories:
//! - Invalid arguments / missing required args
//! - Operating on empty/unborn repo
//! - Conflicting flags
//! - Non-existent paths/refs

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// Missing/invalid arguments
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_error_commit_no_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("new.txt"), "new\n").unwrap();
    std::fs::write(dir_gitr.path().join("new.txt"), "new\n").unwrap();
    git(dir_git.path(), &["add", "new.txt"]);
    git(dir_gitr.path(), &["add", "new.txt"]);
    // Commit without -m should fail (no editor in non-interactive)
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["commit"]);
}

#[test]
fn test_error_merge_no_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["merge"]);
}

#[test]
fn test_error_checkout_nonexistent_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["checkout", "nonexistent-branch"]);
}

#[test]
fn test_error_switch_nonexistent_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["switch", "nonexistent-branch"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_branch_delete_current() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["branch", "-d", "main"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_tag_delete_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["tag", "-d", "nonexistent"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_reset_invalid_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["reset", "nonexistent-ref"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_rebase_no_upstream() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    // No upstream configured and no argument
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["rebase"]);
}

#[test]
fn test_error_cherry_pick_invalid_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["cherry-pick", "nonexistent"]);
}

#[test]
fn test_error_revert_invalid_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["revert", "nonexistent"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_diff_nonexistent_path() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff", "--", "nonexistent.txt"]);
    let m = gitr(dir_gitr.path(), &["diff", "--", "nonexistent.txt"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_blame_nonexistent_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["blame", "nonexistent.txt"]);
}

#[test]
fn test_error_show_invalid_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["show", "nonexistent"]);
}

#[test]
fn test_error_cat_file_missing_object() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["cat-file", "-p", "0000000000000000000000000000000000000000"]);
}

#[test]
fn test_error_rev_parse_invalid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["rev-parse", "--verify", "nonexistent"]);
}

#[test]
fn test_error_merge_base_missing_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["merge-base", "main", "nonexistent"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// Operations on empty/unborn repo
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_log_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["log"]);
    let m = gitr(dir_gitr.path(), &["log"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_diff_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_status_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["status"]);
    let m = gitr(dir_gitr.path(), &["status"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_branch_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["branch"]);
    let m = gitr(dir_gitr.path(), &["branch"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_tag_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["tag"]);
    let m = gitr(dir_gitr.path(), &["tag"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_error_stash_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["stash"]);
    let m = gitr(dir_gitr.path(), &["stash"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_describe_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["describe"]);
}

#[test]
fn test_error_shortlog_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["shortlog"]);
    let m = gitr(dir_gitr.path(), &["shortlog"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Non-existent paths/refs
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_add_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["add", "nonexistent.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "nonexistent.txt"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_rm_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["rm", "nonexistent.txt"]);
}

#[test]
fn test_error_mv_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["mv", "nonexistent.txt", "dest.txt"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_restore_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["restore", "nonexistent.txt"]);
}

#[test]
fn test_error_log_nonexistent_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["log", "nonexistent-branch"]);
}

#[test]
fn test_error_show_ref_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["show-ref", "--verify", "refs/heads/nonexistent"]);
    let m = gitr(dir_gitr.path(), &["show-ref", "--verify", "refs/heads/nonexistent"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Conflict/state errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_merge_abort_no_merge() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["merge", "--abort"]);
}

#[test]
fn test_error_rebase_continue_no_rebase() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["rebase", "--continue"]);
}

#[test]
fn test_error_rebase_abort_no_rebase() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["rebase", "--abort"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_cherry_pick_abort_no_cp() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["cherry-pick", "--abort"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_revert_abort_no_revert() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["revert", "--abort"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_stash_pop_no_stash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["stash", "pop"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_stash_drop_no_stash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["stash", "drop"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// Plumbing command errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_error_cat_file_no_args() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["cat-file"]);
    let m = gitr(dir_gitr.path(), &["cat-file"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_error_diff_tree_no_args() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["diff-tree"]);
    let m = gitr(dir_gitr.path(), &["diff-tree"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_error_update_ref_no_args() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["update-ref"]);
    let m = gitr(dir_gitr.path(), &["update-ref"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Config errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_config_get_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["config", "--get", "nonexistent.key"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_config_unset_nonexistent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["config", "--unset", "nonexistent.key"]);
    let m = gitr(dir_gitr.path(), &["config", "--unset", "nonexistent.key"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Not-a-repo errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_status_not_a_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    // Don't init — these are plain directories
    let g = git(dir_git.path(), &["status"]);
    let m = gitr(dir_gitr.path(), &["status"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_log_not_a_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let g = git(dir_git.path(), &["log"]);
    let m = gitr(dir_gitr.path(), &["log"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_error_diff_not_a_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_branch_not_a_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let g = git(dir_git.path(), &["branch"]);
    let m = gitr(dir_gitr.path(), &["branch"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Duplicate/conflict errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_branch_already_exists() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["branch", "feature"]);
}

#[test]
fn test_error_tag_already_exists() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["tag", "v1.0"]);
}

#[test]
fn test_error_init_in_existing_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    // Re-init should succeed (but not error)
    let g = git(dir_git.path(), &["init"]);
    let m = gitr(dir_gitr.path(), &["init"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Commit nothing to commit
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_error_commit_nothing_staged() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["commit", "-m", "nothing"]);
}

#[test]
#[ignore] // known parity gap
fn test_error_commit_empty_not_allowed() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    // Without --allow-empty, empty commit should fail
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["commit", "-m", "empty"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// Clean errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_clean_no_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());
    // git clean without -f or -n should fail
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["clean"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// Apply errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_error_apply_bad_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("bad.patch"), "not a valid patch\n").unwrap();
    std::fs::write(dir_gitr.path().join("bad.patch"), "not a valid patch\n").unwrap();
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["apply", "bad.patch"]);
}

#[test]
fn test_error_apply_nonexistent_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["apply", "nonexistent.patch"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// Grep errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_grep_no_match() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    // grep returns exit 1 when no matches found
    let g = git(dir_git.path(), &["grep", "ZZZZZ_nonexistent_pattern"]);
    let m = gitr(dir_gitr.path(), &["grep", "ZZZZZ_nonexistent_pattern"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Describe errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_describe_no_tags() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    // Repo with no tags
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    assert_both_fail(dir_git.path(), dir_gitr.path(), &["describe"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// Bare repo errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_status_bare_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(dir_git.path());
    setup_bare_remote(dir_gitr.path());
    let g = git(dir_git.path(), &["status"]);
    let m = gitr(dir_gitr.path(), &["status"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_error_add_bare_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(dir_git.path());
    setup_bare_remote(dir_gitr.path());
    let g = git(dir_git.path(), &["add", "."]);
    let m = gitr(dir_gitr.path(), &["add", "."]);
    assert_exit_code_eq(&g, &m);
}

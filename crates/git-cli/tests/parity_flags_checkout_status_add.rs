//! E2E parity tests for checkout, switch, status, add, restore, clean, init, mv, and rm.
//!
//! Every test creates two identical repos (one for C git, one for gitr), runs
//! the same command on both, and asserts matching output. Mutating commands also
//! verify index and worktree equivalence via `git ls-files -s` and
//! `git status --porcelain`.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// CHECKOUT
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn checkout_switch_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let g = git(dir_git.path(), &["checkout", "feature"]);
    let m = gitr(dir_gitr.path(), &["checkout", "feature"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_create_branch_b() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["checkout", "-b", "new-branch"]);
    let m = gitr(dir_gitr.path(), &["checkout", "-b", "new-branch"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_force_create_branch_big_b() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // First create the branch, then force-recreate it at a different point
    git(dir_git.path(), &["branch", "existing-branch", "HEAD~1"]);
    git(dir_gitr.path(), &["branch", "existing-branch", "HEAD~1"]);

    let g = git(dir_git.path(), &["checkout", "-B", "existing-branch"]);
    let m = gitr(dir_gitr.path(), &["checkout", "-B", "existing-branch"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_detach_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["checkout", "--detach", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["checkout", "--detach", "HEAD~1"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_force_with_uncommitted_changes() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // Create uncommitted changes
    std::fs::write(dir_git.path().join("main_0.txt"), "dirty\n").unwrap();
    std::fs::write(dir_gitr.path().join("main_0.txt"), "dirty\n").unwrap();

    let g = git(dir_git.path(), &["checkout", "-f", "feature"]);
    let m = gitr(dir_gitr.path(), &["checkout", "-f", "feature"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let g = git(dir_git.path(), &["checkout", "-q", "feature"]);
    let m = gitr(dir_gitr.path(), &["checkout", "-q", "feature"]);

    assert_exit_code_eq(&g, &m);
    // Quiet mode: stdout should be empty or minimal
    assert_output_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_orphan() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    let g = git(dir_git.path(), &["checkout", "--orphan", "new-root"]);
    let m = gitr(dir_gitr.path(), &["checkout", "--orphan", "new-root"]);

    assert_exit_code_eq(&g, &m);
    // After --orphan, all files should be staged (index populated, but no parent commit)
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_file_restore() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Modify a tracked file
    std::fs::write(dir_git.path().join("file_0.txt"), "dirty\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "dirty\n").unwrap();

    // Restore single file from index
    let g = git(dir_git.path(), &["checkout", "--", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["checkout", "--", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // The file content should be restored to committed version
    let content_git = std::fs::read_to_string(dir_git.path().join("file_0.txt")).unwrap();
    let content_gitr = std::fs::read_to_string(dir_gitr.path().join("file_0.txt")).unwrap();
    assert_eq!(content_git, content_gitr, "restored file content should match");
}

#[test]
fn checkout_ours_during_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());

    // Trigger conflict
    git(dir_git.path(), &["merge", "feature"]);
    gitr(dir_gitr.path(), &["merge", "feature"]);

    // Resolve with --ours
    let g = git(dir_git.path(), &["checkout", "--ours", "conflict.txt"]);
    let m = gitr(dir_gitr.path(), &["checkout", "--ours", "conflict.txt"]);

    assert_exit_code_eq(&g, &m);

    let content_git = std::fs::read_to_string(dir_git.path().join("conflict.txt")).unwrap();
    let content_gitr = std::fs::read_to_string(dir_gitr.path().join("conflict.txt")).unwrap();
    assert_eq!(content_git, content_gitr, "--ours content should match");
}

#[test]
fn checkout_theirs_during_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());

    // Trigger conflict
    git(dir_git.path(), &["merge", "feature"]);
    gitr(dir_gitr.path(), &["merge", "feature"]);

    // Resolve with --theirs
    let g = git(dir_git.path(), &["checkout", "--theirs", "conflict.txt"]);
    let m = gitr(dir_gitr.path(), &["checkout", "--theirs", "conflict.txt"]);

    assert_exit_code_eq(&g, &m);

    let content_git = std::fs::read_to_string(dir_git.path().join("conflict.txt")).unwrap();
    let content_gitr = std::fs::read_to_string(dir_gitr.path().join("conflict.txt")).unwrap();
    assert_eq!(content_git, content_gitr, "--theirs content should match");
}

// ════════════════════════════════════════════════════════════════════════════
// SWITCH
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn switch_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let g = git(dir_git.path(), &["switch", "feature"]);
    let m = gitr(dir_gitr.path(), &["switch", "feature"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn switch_create_branch_c() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let g = git(dir_git.path(), &["switch", "-c", "new-branch"]);
    let m = gitr(dir_gitr.path(), &["switch", "-c", "new-branch"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn switch_force_create_big_c() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // Create the branch first, then force-recreate it
    git(dir_git.path(), &["branch", "existing", "HEAD~1"]);
    git(dir_gitr.path(), &["branch", "existing", "HEAD~1"]);

    let g = git(dir_git.path(), &["switch", "-C", "existing"]);
    let m = gitr(dir_gitr.path(), &["switch", "-C", "existing"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn switch_detach() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let g = git(dir_git.path(), &["switch", "--detach", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["switch", "--detach", "HEAD~1"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn switch_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let g = git(dir_git.path(), &["switch", "-q", "feature"]);
    let m = gitr(dir_gitr.path(), &["switch", "-q", "feature"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// STATUS
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn status_long_format() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status"]);
    let m = gitr(dir_gitr.path(), &["status"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_short() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "-s"]);
    let m = gitr(dir_gitr.path(), &["status", "-s"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "-b"]);
    let m = gitr(dir_gitr.path(), &["status", "-b"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_porcelain() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_porcelain_v2() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain=v2"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain=v2"]);

    assert_exit_code_eq(&g, &m);
    // porcelain=v2 format may differ slightly in implementation details;
    // verify that both succeed and produce output
    if g.exit_code == 0 {
        assert_output_eq(&g, &m);
    }
}

#[test]
fn status_untracked_files_no() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "-u", "no"]);
    let m = gitr(dir_gitr.path(), &["status", "-u", "no"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_ignored() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--ignored"]);
    let m = gitr(dir_gitr.path(), &["status", "--ignored"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_nul_terminated() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "-z"]);
    let m = gitr(dir_gitr.path(), &["status", "-z"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_ahead_behind() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--ahead-behind"]);
    let m = gitr(dir_gitr.path(), &["status", "--ahead-behind"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
}

#[test]
fn status_no_ahead_behind() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--no-ahead-behind"]);
    let m = gitr(dir_gitr.path(), &["status", "--no-ahead-behind"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
}

#[test]
fn status_short_branch_combined() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "-sb"]);
    let m = gitr(dir_gitr.path(), &["status", "-sb"]);

    assert_output_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// ADD
// ════════════════════════════════════════════════════════════════════════════

/// Helper: set up a repo with 1 commit, then create untracked + modified files.
fn setup_add_scenario(dir: &std::path::Path) {
    setup_linear_history(dir, 1);
    // Create untracked files
    std::fs::write(dir.join("new_file.txt"), "new content\n").unwrap();
    std::fs::write(dir.join("another_new.txt"), "another new\n").unwrap();
    // Modify a tracked file
    std::fs::write(dir.join("file_0.txt"), "modified content\n").unwrap();
    // Create .gitignore and an ignored file
    std::fs::write(dir.join(".gitignore"), "*.ignored\n").unwrap();
    std::fs::write(dir.join("secret.ignored"), "ignored content\n").unwrap();
}

#[test]
fn add_single_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "new_file.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "new_file.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "-A"]);
    let m = gitr(dir_gitr.path(), &["add", "-A"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_update() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "-u"]);
    let m = gitr(dir_gitr.path(), &["add", "-u"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_dry_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "-n", "new_file.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "-n", "new_file.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
    // Dry-run should not modify the index
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_dry_run_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "--dry-run", "new_file.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "--dry-run", "new_file.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
}

#[test]
fn add_force_ignored_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "-f", "secret.ignored"]);
    let m = gitr(dir_gitr.path(), &["add", "-f", "secret.ignored"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_force_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "--force", "secret.ignored"]);
    let m = gitr(dir_gitr.path(), &["add", "--force", "secret.ignored"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_verbose() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "-v", "new_file.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "-v", "new_file.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_verbose_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "--verbose", "new_file.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "--verbose", "new_file.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// RESTORE
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn restore_file_from_index() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Modify a tracked file (unstaged)
    std::fs::write(dir_git.path().join("file_0.txt"), "dirty\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "dirty\n").unwrap();

    let g = git(dir_git.path(), &["restore", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    let content_git = std::fs::read_to_string(dir_git.path().join("file_0.txt")).unwrap();
    let content_gitr = std::fs::read_to_string(dir_gitr.path().join("file_0.txt")).unwrap();
    assert_eq!(content_git, content_gitr, "restored content should match");
}

#[test]
fn restore_staged_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Stage a modification
    std::fs::write(dir_git.path().join("file_0.txt"), "staged\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "staged\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);

    // Unstage with --staged
    let g = git(dir_git.path(), &["restore", "--staged", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "--staged", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn restore_source_head_tilde() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Restore file_0.txt from HEAD~1
    let g = git(dir_git.path(), &["restore", "--source=HEAD~1", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "--source=HEAD~1", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    let content_git = std::fs::read_to_string(dir_git.path().join("file_0.txt")).unwrap();
    let content_gitr = std::fs::read_to_string(dir_gitr.path().join("file_0.txt")).unwrap();
    assert_eq!(content_git, content_gitr, "restore --source content should match");
}

#[test]
fn restore_worktree_explicit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Modify a tracked file
    std::fs::write(dir_git.path().join("file_0.txt"), "dirty\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "dirty\n").unwrap();

    let g = git(dir_git.path(), &["restore", "--worktree", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "--worktree", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    let content_git = std::fs::read_to_string(dir_git.path().join("file_0.txt")).unwrap();
    let content_gitr = std::fs::read_to_string(dir_gitr.path().join("file_0.txt")).unwrap();
    assert_eq!(content_git, content_gitr, "restore --worktree content should match");
}

// ════════════════════════════════════════════════════════════════════════════
// CLEAN
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clean_dry_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "-n"]);
    let m = gitr(dir_gitr.path(), &["clean", "-n"]);

    assert_output_eq(&g, &m);
    // Dry-run should not remove anything
    assert!(dir_git.path().join("untracked_a.txt").exists());
    assert!(dir_gitr.path().join("untracked_a.txt").exists());
}

#[test]
fn clean_dry_run_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "--dry-run"]);
    let m = gitr(dir_gitr.path(), &["clean", "--dry-run"]);

    assert_output_eq(&g, &m);
}

#[test]
fn clean_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "-f"]);
    let m = gitr(dir_gitr.path(), &["clean", "-f"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Untracked files should be removed, tracked should remain
    assert!(!dir_git.path().join("untracked_a.txt").exists());
    assert!(!dir_gitr.path().join("untracked_a.txt").exists());
    assert!(dir_git.path().join("tracked_a.txt").exists());
    assert!(dir_gitr.path().join("tracked_a.txt").exists());
}

#[test]
fn clean_force_directories() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "-fd"]);
    let m = gitr(dir_gitr.path(), &["clean", "-fd"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Untracked directory should also be removed
    assert!(!dir_git.path().join("untracked_dir").exists());
    assert!(!dir_gitr.path().join("untracked_dir").exists());
}

#[test]
fn clean_include_ignored() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "-fx"]);
    let m = gitr(dir_gitr.path(), &["clean", "-fx"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Ignored file should also be removed
    assert!(!dir_git.path().join("ignored.log").exists());
    assert!(!dir_gitr.path().join("ignored.log").exists());
}

#[test]
fn clean_only_ignored() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "-fX"]);
    let m = gitr(dir_gitr.path(), &["clean", "-fX"]);

    assert_exit_code_eq(&g, &m);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Ignored file should be removed, untracked files should remain
    assert!(!dir_git.path().join("ignored.log").exists());
    assert!(!dir_gitr.path().join("ignored.log").exists());
    assert!(dir_git.path().join("untracked_a.txt").exists());
    assert!(dir_gitr.path().join("untracked_a.txt").exists());
}

// ════════════════════════════════════════════════════════════════════════════
// INIT
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn init_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let g = git(dir_git.path(), &["init", "-b", "main"]);
    let m = gitr(dir_gitr.path(), &["init", "-b", "main"]);

    assert_exit_code_eq(&g, &m);

    // Both should create a .git directory
    assert!(dir_git.path().join(".git").exists());
    assert!(dir_gitr.path().join(".git").exists());

    // HEAD should point to main
    let head_git = std::fs::read_to_string(dir_git.path().join(".git/HEAD")).unwrap();
    let head_gitr = std::fs::read_to_string(dir_gitr.path().join(".git/HEAD")).unwrap();
    assert_eq!(head_git.trim(), head_gitr.trim());
}

#[test]
fn init_bare() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let git_bare = dir_git.path().join("repo.git");
    let gitr_bare = dir_gitr.path().join("repo.git");
    std::fs::create_dir_all(&git_bare).unwrap();
    std::fs::create_dir_all(&gitr_bare).unwrap();

    let g = git(&git_bare, &["init", "--bare", "-b", "main"]);
    let m = gitr(&gitr_bare, &["init", "--bare", "-b", "main"]);

    assert_exit_code_eq(&g, &m);

    // Bare repo: HEAD, refs/, objects/ should exist at top level, no .git wrapper
    assert!(git_bare.join("HEAD").exists());
    assert!(gitr_bare.join("HEAD").exists());
    assert!(git_bare.join("refs").exists());
    assert!(gitr_bare.join("refs").exists());
    assert!(git_bare.join("objects").exists());
    assert!(gitr_bare.join("objects").exists());
    assert!(!git_bare.join(".git").exists());
    assert!(!gitr_bare.join(".git").exists());
}

#[test]
fn init_custom_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let g = git(dir_git.path(), &["init", "-b", "custom-branch"]);
    let m = gitr(dir_gitr.path(), &["init", "-b", "custom-branch"]);

    assert_exit_code_eq(&g, &m);

    let head_git = std::fs::read_to_string(dir_git.path().join(".git/HEAD")).unwrap();
    let head_gitr = std::fs::read_to_string(dir_gitr.path().join(".git/HEAD")).unwrap();
    assert_eq!(head_git.trim(), "ref: refs/heads/custom-branch");
    assert_eq!(head_gitr.trim(), "ref: refs/heads/custom-branch");
}

// ════════════════════════════════════════════════════════════════════════════
// MV
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn mv_basic_rename() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["mv", "file_0.txt", "newname.txt"]);
    let m = gitr(dir_gitr.path(), &["mv", "file_0.txt", "newname.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Old file should not exist, new file should exist
    assert!(!dir_git.path().join("file_0.txt").exists());
    assert!(!dir_gitr.path().join("file_0.txt").exists());
    assert!(dir_git.path().join("newname.txt").exists());
    assert!(dir_gitr.path().join("newname.txt").exists());
}

#[test]
fn mv_dry_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["mv", "-n", "file_0.txt", "newname.txt"]);
    let m = gitr(dir_gitr.path(), &["mv", "-n", "file_0.txt", "newname.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);

    // Dry-run: file should NOT be moved
    assert!(dir_git.path().join("file_0.txt").exists());
    assert!(dir_gitr.path().join("file_0.txt").exists());
    assert!(!dir_git.path().join("newname.txt").exists());
    assert!(!dir_gitr.path().join("newname.txt").exists());
}

#[test]
fn mv_dry_run_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["mv", "--dry-run", "file_0.txt", "newname.txt"]);
    let m = gitr(dir_gitr.path(), &["mv", "--dry-run", "file_0.txt", "newname.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
}

#[test]
fn mv_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    // Create the destination so that -f is needed to overwrite
    std::fs::write(dir_git.path().join("newname.txt"), "existing\n").unwrap();
    std::fs::write(dir_gitr.path().join("newname.txt"), "existing\n").unwrap();
    git(dir_git.path(), &["add", "newname.txt"]);
    git(dir_gitr.path(), &["add", "newname.txt"]);

    let g = git(dir_git.path(), &["mv", "-f", "file_0.txt", "newname.txt"]);
    let m = gitr(dir_gitr.path(), &["mv", "-f", "file_0.txt", "newname.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// RM
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rm_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["rm", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // File should be removed from disk
    assert!(!dir_git.path().join("file_0.txt").exists());
    assert!(!dir_gitr.path().join("file_0.txt").exists());
}

#[test]
fn rm_cached() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["rm", "--cached", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "--cached", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // File should still exist on disk (only removed from index)
    assert!(dir_git.path().join("file_0.txt").exists());
    assert!(dir_gitr.path().join("file_0.txt").exists());
}

#[test]
fn rm_dry_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["rm", "-n", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "-n", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);

    // Dry-run: file should still exist
    assert!(dir_git.path().join("file_0.txt").exists());
    assert!(dir_gitr.path().join("file_0.txt").exists());
}

#[test]
fn rm_dry_run_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["rm", "--dry-run", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "--dry-run", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_output_eq(&g, &m);
}

#[test]
fn rm_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    // Modify the file so that rm would normally refuse without -f
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);

    let g = git(dir_git.path(), &["rm", "-f", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "-f", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    assert!(!dir_git.path().join("file_0.txt").exists());
    assert!(!dir_gitr.path().join("file_0.txt").exists());
}

#[test]
fn rm_force_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);

    let g = git(dir_git.path(), &["rm", "--force", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "--force", "file_0.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// Additional status edge cases
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn status_clean_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["status"]);
    let m = gitr(dir_gitr.path(), &["status"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_porcelain_clean() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);

    assert_output_eq(&g, &m);
    assert!(g.stdout.is_empty(), "clean repo should have empty porcelain output");
}

#[test]
fn status_with_staged_and_modified() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Stage a change to file_0
    std::fs::write(dir_git.path().join("file_0.txt"), "staged\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "staged\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);

    // Also modify file_1 without staging
    std::fs::write(dir_git.path().join("file_1.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_1.txt"), "modified\n").unwrap();

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);

    assert_output_eq(&g, &m);
}

#[test]
fn status_short_with_staged_and_modified() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    std::fs::write(dir_git.path().join("file_0.txt"), "staged\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "staged\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);

    std::fs::write(dir_git.path().join("file_1.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_1.txt"), "modified\n").unwrap();

    let g = git(dir_git.path(), &["status", "-s"]);
    let m = gitr(dir_gitr.path(), &["status", "-s"]);

    assert_output_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// Additional checkout edge cases
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn checkout_create_branch_at_specific_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["checkout", "-b", "from-old", "HEAD~2"]);
    let m = gitr(dir_gitr.path(), &["checkout", "-b", "from-old", "HEAD~2"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn checkout_back_to_main() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // Switch to feature then back to main
    git(dir_git.path(), &["checkout", "feature"]);
    gitr(dir_gitr.path(), &["checkout", "feature"]);

    let g = git(dir_git.path(), &["checkout", "main"]);
    let m = gitr(dir_gitr.path(), &["checkout", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// Additional clean edge cases
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clean_dry_run_include_ignored() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());

    let g = git(dir_git.path(), &["clean", "-ndx"]);
    let m = gitr(dir_gitr.path(), &["clean", "-ndx"]);

    assert_output_eq(&g, &m);

    // Dry-run: nothing should be removed
    assert!(dir_git.path().join("ignored.log").exists());
    assert!(dir_gitr.path().join("ignored.log").exists());
}

// ════════════════════════════════════════════════════════════════════════════
// Additional add edge cases
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn add_multiple_files() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "new_file.txt", "another_new.txt"]);
    let m = gitr(dir_gitr.path(), &["add", "new_file.txt", "another_new.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn add_dot() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_add_scenario(dir_git.path());
    setup_add_scenario(dir_gitr.path());

    let g = git(dir_git.path(), &["add", "."]);
    let m = gitr(dir_gitr.path(), &["add", "."]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// Additional rm edge case
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rm_multiple_files() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["rm", "file_0.txt", "file_1.txt"]);
    let m = gitr(dir_gitr.path(), &["rm", "file_0.txt", "file_1.txt"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    assert!(!dir_git.path().join("file_0.txt").exists());
    assert!(!dir_gitr.path().join("file_0.txt").exists());
    assert!(!dir_git.path().join("file_1.txt").exists());
    assert!(!dir_gitr.path().join("file_1.txt").exists());
}

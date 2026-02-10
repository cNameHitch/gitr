//! E2E parity tests for Tier 1 command flags that have gaps in existing coverage.
//!
//! Fills specific flag gaps identified across all parity_flags_* test files.

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// LOG — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_log_walk_reflogs() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["log", "-g", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "-g", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_log_walk_reflogs_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["log", "--walk-reflogs", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--walk-reflogs", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_log_simplify_by_decoration() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    // Add tags so there are decorations
    git(dir_git.path(), &["tag", "v1.0", "HEAD~1"]);
    git(dir_gitr.path(), &["tag", "v1.0", "HEAD~1"]);
    let g = git(dir_git.path(), &["log", "--simplify-by-decoration", "--oneline", "--all"]);
    let m = gitr(dir_gitr.path(), &["log", "--simplify-by-decoration", "--oneline", "--all"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_log_find_copies() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        let mut counter = 0u64;
        std::fs::write(dir.join("original.txt"), "some shared content\nline 2\nline 3\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "original.txt"], &date);
        git_with_date(dir, &["commit", "-m", "add original"], &date);

        std::fs::write(dir.join("copy.txt"), "some shared content\nline 2\nline 3\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "copy.txt"], &date);
        git_with_date(dir, &["commit", "-m", "add copy"], &date);
    };

    setup(dir_git.path());
    setup(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "-C", "--name-status", "-1"]);
    let m = gitr(dir_gitr.path(), &["log", "-C", "--name-status", "-1"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_log_since_until() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["log", "--since=1234567892", "--until=1234567895", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--since=1234567892", "--until=1234567895", "--oneline"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// DIFF — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_diff_find_renames_bare() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_renamed_files(dir_git.path());
    setup_renamed_files(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "-M", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff", "-M", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_find_copies() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_renamed_files(dir_git.path());
    setup_renamed_files(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "-C", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff", "-C", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_word_diff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified content for commit 0\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified content for commit 0\n").unwrap();
    let g = git(dir_git.path(), &["diff", "--word-diff"]);
    let m = gitr(dir_gitr.path(), &["diff", "--word-diff"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_color_words() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified content\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified content\n").unwrap();
    let g = git(dir_git.path(), &["diff", "--color-words", "--color=never"]);
    let m = gitr(dir_gitr.path(), &["diff", "--color-words", "--color=never"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_diff_pickaxe_regex() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["diff", "-G", "commit [0-2]", "HEAD~2", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff", "-G", "commit [0-2]", "HEAD~2", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_diff_diff_filter_added() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["diff", "--diff-filter=A", "--name-only", "HEAD~2", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff", "--diff-filter=A", "--name-only", "HEAD~2", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_diff_diff_filter_deleted() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 2);
        git(dir, &["rm", "file_0.txt"]);
        git(dir, &["commit", "-m", "remove file"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--diff-filter=D", "--name-only", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff", "--diff-filter=D", "--name-only", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// COMMIT — missing flags
// ══════════════════════════════════════════════════════════════════════════════

fn setup_commit_workdir(dir: &std::path::Path) {
    setup_linear_history(dir, 2);
    std::fs::write(dir.join("new_file.txt"), "new content\n").unwrap();
    git(dir, &["add", "new_file.txt"]);
}

#[test]
#[ignore] // known parity gap
fn test_commit_dry_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["commit", "--dry-run", "-m", "test"]);
    let m = gitr(dir_gitr.path(), &["commit", "--dry-run", "-m", "test"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_verbose() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    // -v with -m still works, just includes diff in the commit msg editor (which we skip)
    let g = git(dir_git.path(), &["commit", "-v", "-m", "verbose commit"]);
    let m = gitr(dir_gitr.path(), &["commit", "-v", "-m", "verbose commit"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_no_verify() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["commit", "--no-verify", "-m", "no verify"]);
    let m = gitr(dir_gitr.path(), &["commit", "--no-verify", "-m", "no verify"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_commit_date_override() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["commit", "--date=2020-01-01T00:00:00+0000", "-m", "dated"]);
    let m = gitr(dir_gitr.path(), &["commit", "--date=2020-01-01T00:00:00+0000", "-m", "dated"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_fixup() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["commit", "--fixup=HEAD"]);
    let m = gitr(dir_gitr.path(), &["commit", "--fixup=HEAD"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_squash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    // --squash requires --no-edit or it opens editor
    let g = git(dir_git.path(), &["commit", "--squash=HEAD", "--no-edit"]);
    let m = gitr(dir_gitr.path(), &["commit", "--squash=HEAD", "--no-edit"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_file_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    std::fs::write(dir_git.path().join("msg.txt"), "commit from file\n").unwrap();
    std::fs::write(dir_gitr.path().join("msg.txt"), "commit from file\n").unwrap();
    let g = git(dir_git.path(), &["commit", "-F", "msg.txt"]);
    let m = gitr(dir_gitr.path(), &["commit", "-F", "msg.txt"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_reuse_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["commit", "-C", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["commit", "-C", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_commit_trailer() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_commit_workdir(dir_git.path());
    setup_commit_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["commit", "-m", "msg", "--trailer", "Signed-off-by: Test <test@test.com>"]);
    let m = gitr(dir_gitr.path(), &["commit", "-m", "msg", "--trailer", "Signed-off-by: Test <test@test.com>"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// BRANCH — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_branch_force_delete() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["branch", "-D", "feature"]);
    let m = gitr(dir_gitr.path(), &["branch", "-D", "feature"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_branch_force_move() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["branch", "-M", "feature", "new-feature"]);
    let m = gitr(dir_gitr.path(), &["branch", "-M", "feature", "new-feature"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_branch_copy() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["branch", "-c", "feature", "feature-copy"]);
    let m = gitr(dir_gitr.path(), &["branch", "-c", "feature", "feature-copy"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_branch_no_track() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["branch", "--no-track", "new-branch", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["branch", "--no-track", "new-branch", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// MERGE — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_merge_no_edit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge", "--no-edit", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge", "--no-edit", "feature"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_merge_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge", "-q", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge", "-q", "feature"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_merge_signoff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge", "--signoff", "--no-edit", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge", "--signoff", "--no-edit", "feature"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_merge_continue() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());
    // Start merge (will conflict)
    git(dir_git.path(), &["merge", "feature"]);
    gitr(dir_gitr.path(), &["merge", "feature"]);
    // Resolve conflict
    std::fs::write(dir_git.path().join("conflict.txt"), "resolved\n").unwrap();
    std::fs::write(dir_gitr.path().join("conflict.txt"), "resolved\n").unwrap();
    git(dir_git.path(), &["add", "conflict.txt"]);
    git(dir_gitr.path(), &["add", "conflict.txt"]);
    // Continue
    let g = git(dir_git.path(), &["merge", "--continue"]);
    let m = gitr(dir_gitr.path(), &["merge", "--continue"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_merge_allow_unrelated_histories() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        let mut counter = 0u64;
        std::fs::write(dir.join("a.txt"), "main content\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "a.txt"], &date);
        git_with_date(dir, &["commit", "-m", "main"], &date);

        // Create orphan branch with unrelated history
        git(dir, &["checkout", "--orphan", "unrelated"]);
        git(dir, &["rm", "-rf", "."]);
        std::fs::write(dir.join("b.txt"), "unrelated content\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "b.txt"], &date);
        git_with_date(dir, &["commit", "-m", "unrelated"], &date);
        git(dir, &["checkout", "main"]);
    };

    setup(dir_git.path());
    setup(dir_gitr.path());
    let g = git(dir_git.path(), &["merge", "--allow-unrelated-histories", "--no-edit", "unrelated"]);
    let m = gitr(dir_gitr.path(), &["merge", "--allow-unrelated-histories", "--no-edit", "unrelated"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// REBASE — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rebase_verbose() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["rebase", "-v", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "-v", "main"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_rebase_signoff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["rebase", "--signoff", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--signoff", "main"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_rebase_keep_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["rebase", "--keep-empty", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--keep-empty", "main"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_rebase_autostash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());
    // Create dirty working tree
    std::fs::write(dir_git.path().join("dirty.txt"), "dirty\n").unwrap();
    std::fs::write(dir_gitr.path().join("dirty.txt"), "dirty\n").unwrap();
    let g = git(dir_git.path(), &["rebase", "--autostash", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--autostash", "main"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// CHERRY-PICK — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_edit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    // --no-edit to avoid editor
    let g = git(dir_git.path(), &["cherry-pick", "--no-edit", "feature~1"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--no-edit", "feature~1"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_cherry_pick_skip() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());
    // Cherry-pick the feature commit that conflicts
    git(dir_git.path(), &["cherry-pick", "feature"]);
    gitr(dir_gitr.path(), &["cherry-pick", "feature"]);
    // Skip it
    let g = git(dir_git.path(), &["cherry-pick", "--skip"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--skip"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_cherry_pick_allow_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    // Create an empty commit
    git(dir_git.path(), &["commit", "--allow-empty", "-m", "empty"]);
    git(dir_gitr.path(), &["commit", "--allow-empty", "-m", "empty"]);
    // Cherry-pick the empty commit
    let g = git(dir_git.path(), &["cherry-pick", "--allow-empty", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--allow-empty", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// REVERT — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_revert_no_edit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["revert", "--no-edit", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "--no-edit", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_revert_skip() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        let mut counter = 0u64;
        std::fs::write(dir.join("file.txt"), "line 1\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "initial"], &date);
        std::fs::write(dir.join("file.txt"), "line 1\nline 2\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "add line 2"], &date);
        std::fs::write(dir.join("file.txt"), "line 1\nline 2 modified\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "modify line 2"], &date);
    };

    setup(dir_git.path());
    setup(dir_gitr.path());
    // Revert commit that will conflict
    git(dir_git.path(), &["revert", "--no-edit", "HEAD~1"]);
    gitr(dir_gitr.path(), &["revert", "--no-edit", "HEAD~1"]);
    // Skip
    let g = git(dir_git.path(), &["revert", "--skip"]);
    let m = gitr(dir_gitr.path(), &["revert", "--skip"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// SWITCH — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_switch_create_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    // Force-create over existing branch
    let g = git(dir_git.path(), &["switch", "-C", "feature"]);
    let m = gitr(dir_gitr.path(), &["switch", "-C", "feature"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_switch_orphan() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["switch", "--orphan", "orphan-branch"]);
    let m = gitr(dir_gitr.path(), &["switch", "--orphan", "orphan-branch"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_switch_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    // Create dirty changes
    std::fs::write(dir_git.path().join("dirty.txt"), "dirty\n").unwrap();
    std::fs::write(dir_gitr.path().join("dirty.txt"), "dirty\n").unwrap();
    let g = git(dir_git.path(), &["switch", "--force", "feature"]);
    let m = gitr(dir_gitr.path(), &["switch", "--force", "feature"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// RESTORE — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_restore_overlay() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    std::fs::write(dir_git.path().join("file_0.txt"), "changed\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "changed\n").unwrap();
    let g = git(dir_git.path(), &["restore", "--worktree", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "--worktree", "file_0.txt"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_restore_staged_and_worktree() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    std::fs::write(dir_git.path().join("file_0.txt"), "changed\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "changed\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);
    let g = git(dir_git.path(), &["restore", "--staged", "--worktree", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "--staged", "--worktree", "file_0.txt"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_restore_source() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["restore", "--source=HEAD~2", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["restore", "--source=HEAD~2", "file_0.txt"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// PUSH — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_push_delete_branch() {
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |remote_dir: &std::path::Path, work_dir: &std::path::Path| {
        setup_bare_remote(remote_dir);
        let url = format!("file://{}", remote_dir.display());
        git(work_dir, &["clone", &url, "."]);
        git(work_dir, &["config", "user.name", "Test Author"]);
        git(work_dir, &["config", "user.email", "test@example.com"]);
        git(work_dir, &["checkout", "-b", "to-delete"]);
        std::fs::write(work_dir.join("x.txt"), "x\n").unwrap();
        git(work_dir, &["add", "x.txt"]);
        git(work_dir, &["commit", "-m", "x"]);
        git(work_dir, &["push", "origin", "to-delete"]);
        git(work_dir, &["checkout", "main"]);
    };

    // Use separate remotes for git and gitr to avoid cross-contamination
    let remote_gitr = tempfile::tempdir().unwrap();
    setup(remote.path(), dir_git.path());
    setup(remote_gitr.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["push", "origin", "--delete", "to-delete"]);
    let m = gitr(dir_gitr.path(), &["push", "origin", "--delete", "to-delete"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_push_dry_run() {
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());
    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(dir_git.path().join("x.txt"), "x\n").unwrap();
    git(dir_git.path(), &["add", "x.txt"]);
    git(dir_git.path(), &["commit", "-m", "x"]);
    let g = git(dir_git.path(), &["push", "--dry-run", "origin", "main"]);
    let m = gitr(dir_git.path(), &["push", "--dry-run", "origin", "main"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// FETCH — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_fetch_verbose() {
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());
    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    let g = git(dir_git.path(), &["fetch", "-v"]);
    let m = gitr(dir_git.path(), &["fetch", "-v"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_fetch_depth() {
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());
    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    let g = git(dir_git.path(), &["fetch", "--depth=1"]);
    let m = gitr(dir_git.path(), &["fetch", "--depth=1"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// CLONE — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_clone_single_branch() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let g = git(dir_git.path(), &["clone", "--single-branch", &url, "."]);
    let m = gitr(dir_gitr.path(), &["clone", "--single-branch", &url, "."]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_clone_no_tags() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let g = git(dir_git.path(), &["clone", "--no-tags", &url, "."]);
    let m = gitr(dir_gitr.path(), &["clone", "--no-tags", &url, "."]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_clone_mirror() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let clone_git = dir_git.path().join("repo");
    let clone_gitr = dir_gitr.path().join("repo");
    let g = git(dir_git.path(), &["clone", "--mirror", &url, clone_git.to_str().unwrap()]);
    let m = gitr(dir_gitr.path(), &["clone", "--mirror", &url, clone_gitr.to_str().unwrap()]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// PULL — missing flags
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_pull_verbose() {
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());
    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    let g = git(dir_git.path(), &["pull", "-v"]);
    let m = gitr(dir_git.path(), &["pull", "-v"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_pull_no_stat() {
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());
    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    let g = git(dir_git.path(), &["pull", "--no-stat"]);
    let m = gitr(dir_git.path(), &["pull", "--no-stat"]);
    assert_exit_code_eq(&g, &m);
}

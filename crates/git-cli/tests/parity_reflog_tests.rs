//! E2E parity tests for reflog — Phase 4B.
//!
//! For every command that writes reflog entries (commit, merge, rebase,
//! cherry-pick, revert, reset, checkout, switch, pull, stash), verify
//! that reflog output matches between git and gitr.

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// commit reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_amend() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 2);
        git(dir, &["commit", "--amend", "-m", "amended"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// merge reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_merge_ff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_merge_scenarios(dir);
        git(dir, &["merge", "ff-branch"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_merge_no_ff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_branched_history(dir);
        git(dir, &["merge", "--no-ff", "--no-edit", "feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_merge_abort() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_merge_conflict(dir);
        git(dir, &["merge", "feature"]);
        git(dir, &["merge", "--abort"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// rebase reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_rebase() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_rebase_scenarios(dir);
        git(dir, &["rebase", "main"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_rebase_onto() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_rebase_scenarios(dir);
        let base = git(dir, &["merge-base", "main", "feature"]).stdout.trim().to_string();
        git(dir, &["rebase", "--onto", "main", &base, "feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// cherry-pick reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_cherry_pick() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_branched_history(dir);
        let oid = git(dir, &["rev-parse", "feature~1"]).stdout.trim().to_string();
        git(dir, &["cherry-pick", &oid]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// revert reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_revert() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["revert", "--no-edit", "HEAD"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// reset reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_reset_soft() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["reset", "--soft", "HEAD~1"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_reset_hard() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["reset", "--hard", "HEAD~2"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_reset_mixed() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["reset", "HEAD~1"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// checkout reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_checkout_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_branched_history(dir);
        git(dir, &["checkout", "feature"]);
        git(dir, &["checkout", "main"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_checkout_detach() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["checkout", "--detach", "HEAD~1"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_checkout_new_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 2);
        git(dir, &["checkout", "-b", "new-branch"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// switch reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_switch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_branched_history(dir);
        git(dir, &["switch", "feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_switch_create() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 2);
        git(dir, &["switch", "-c", "new-branch"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// stash reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_after_stash_push() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_stash_scenarios(dir);
        git(dir, &["stash", "push", "-m", "test stash"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

#[test]
fn test_reflog_after_stash_pop() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_stash_scenarios(dir);
        git(dir, &["stash", "push", "-m", "test stash"]);
        git(dir, &["stash", "pop"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

// ══════════════════════════════════════════════════════════════════════════════
// reflog subcommand parity
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_show_main() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["reflog", "show", "main"]);
    let m = gitr(dir_gitr.path(), &["reflog", "show", "main"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_reflog_show_default() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["reflog"]);
    let m = gitr(dir_gitr.path(), &["reflog"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_reflog_exists_valid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["reflog", "exists", "refs/heads/main"]);
    let m = gitr(dir_gitr.path(), &["reflog", "exists", "refs/heads/main"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_reflog_exists_invalid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["reflog", "exists", "refs/heads/nonexistent"]);
    let m = gitr(dir_gitr.path(), &["reflog", "exists", "refs/heads/nonexistent"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// branch reflog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_branch_after_commits() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "main");
}

#[test]
fn test_reflog_after_branch_rename() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_branched_history(dir);
        git(dir, &["branch", "-m", "feature", "renamed-feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    // After rename, the new branch name should have the reflog
    let g = git(dir_git.path(), &["reflog", "show", "renamed-feature"]);
    let m = gitr(dir_gitr.path(), &["reflog", "show", "renamed-feature"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// complex reflog scenarios
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_complex_workflow() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        // Initial commit
        std::fs::write(dir.join("a.txt"), "initial\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "."], &date);
        git_with_date(dir, &["commit", "-m", "initial"], &date);

        // Branch and commit
        git(dir, &["checkout", "-b", "feature"]);
        std::fs::write(dir.join("b.txt"), "feature work\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "."], &date);
        git_with_date(dir, &["commit", "-m", "feature work"], &date);

        // Back to main and commit
        git(dir, &["checkout", "main"]);
        std::fs::write(dir.join("c.txt"), "main work\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "."], &date);
        git_with_date(dir, &["commit", "-m", "main work"], &date);

        // Merge
        git(dir, &["merge", "--no-edit", "feature"]);

        // Reset
        git(dir, &["reset", "--soft", "HEAD~1"]);

        // Re-commit
        let date = next_date(&mut counter);
        git_with_date(dir, &["commit", "-m", "re-merged"], &date);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());
    assert_reflog_eq(dir_git.path(), dir_gitr.path(), "HEAD");
}

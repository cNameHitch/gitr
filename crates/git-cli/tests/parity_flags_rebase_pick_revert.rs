//! E2E parity tests for rebase, cherry-pick, revert, pull, push, fetch,
//! and clone commands.
//!
//! Every test follows the dual-repo pattern: set up identical repos for
//! C git and gitr, run the same command, and compare results.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// REBASE
// ════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_rebase_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Verify same commit count and messages after rebase
    let g_log = git(dir_git.path(), &["log", "--format=%s"]);
    let m_log = gitr(dir_gitr.path(), &["log", "--format=%s"]);
    assert_output_eq(&g_log, &m_log);
}

#[test]
#[ignore] // known parity gap
fn test_rebase_onto() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    // --onto main HEAD~1 means: take the last 1 commit on feature, replay onto main
    let g = git(dir_git.path(), &["rebase", "--onto", "main", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--onto", "main", "HEAD~1"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    let g_log = git(dir_git.path(), &["log", "--format=%s"]);
    let m_log = gitr(dir_gitr.path(), &["log", "--format=%s"]);
    assert_output_eq(&g_log, &m_log);
}

#[test]
fn test_rebase_abort_after_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Set up rebase scenario then create a conflict on the same file
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        // Base commit
        std::fs::write(dir.join("shared.txt"), "original\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "base"], &date);

        // Main: modify shared.txt
        std::fs::write(dir.join("shared.txt"), "main change\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "main modify"], &date);

        // Feature from base: modify same file
        git(dir, &["checkout", "-b", "feature", "HEAD~1"]);
        std::fs::write(dir.join("shared.txt"), "feature change\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "feature modify"], &date);
    }

    // Record HEAD before rebase
    let g_head_before = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_before = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // Attempt rebase (should conflict)
    let g = git(dir_git.path(), &["rebase", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "main"]);
    assert_ne!(g.exit_code, 0, "git rebase should conflict");
    assert_ne!(m.exit_code, 0, "gitr rebase should conflict");

    // Abort
    let g_abort = git(dir_git.path(), &["rebase", "--abort"]);
    let m_abort = gitr(dir_gitr.path(), &["rebase", "--abort"]);
    assert_eq!(g_abort.exit_code, 0, "git rebase --abort failed: {}", g_abort.stderr);
    assert_eq!(m_abort.exit_code, 0, "gitr rebase --abort failed: {}", m_abort.stderr);

    // HEAD should be restored
    let g_head_after = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_after = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    assert_eq!(g_head_before, g_head_after, "git HEAD not restored after rebase --abort");
    assert_eq!(m_head_before, m_head_after, "gitr HEAD not restored after rebase --abort");

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_rebase_continue_after_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        std::fs::write(dir.join("shared.txt"), "original\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "base"], &date);

        std::fs::write(dir.join("shared.txt"), "main change\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "main modify"], &date);

        git(dir, &["checkout", "-b", "feature", "HEAD~1"]);
        std::fs::write(dir.join("shared.txt"), "feature change\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "shared.txt"], &date);
        git_with_date(dir, &["commit", "-m", "feature modify"], &date);
    }

    // Attempt rebase (will conflict)
    git(dir_git.path(), &["rebase", "main"]);
    gitr(dir_gitr.path(), &["rebase", "main"]);

    // Resolve conflict in both repos the same way
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("shared.txt"), "resolved content\n").unwrap();
        git(dir, &["add", "shared.txt"]);
    }

    // Continue rebase
    let g = git(dir_git.path(), &["rebase", "--continue"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--continue"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_rebase_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "--stat", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--stat", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_rebase_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "-q", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "-q", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_rebase_no_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "--no-stat", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--no-stat", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_rebase_signoff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "--signoff", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--signoff", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Verify Signed-off-by line is present in the rebased commits
    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%B"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%B"]);
    assert!(
        g_msg.stdout.contains("Signed-off-by:"),
        "git rebase --signoff should add Signed-off-by"
    );
    assert!(
        m_msg.stdout.contains("Signed-off-by:"),
        "gitr rebase --signoff should add Signed-off-by"
    );
}

#[test]
#[ignore] // known parity gap
fn test_rebase_keep_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Set up a scenario with an empty commit on the feature branch
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        std::fs::write(dir.join("base.txt"), "base\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "base.txt"], &date);
        git_with_date(dir, &["commit", "-m", "base"], &date);

        std::fs::write(dir.join("main.txt"), "main\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "main.txt"], &date);
        git_with_date(dir, &["commit", "-m", "main work"], &date);

        git(dir, &["checkout", "-b", "feature", "HEAD~1"]);

        // Create an empty commit
        let date = next_date(&mut counter);
        git_with_date(dir, &["commit", "--allow-empty", "-m", "empty commit"], &date);

        std::fs::write(dir.join("feat.txt"), "feat\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "feat.txt"], &date);
        git_with_date(dir, &["commit", "-m", "feature work"], &date);
    }

    let g = git(dir_git.path(), &["rebase", "--keep-empty", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--keep-empty", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// CHERRY-PICK
// ════════════════════════════════════════════════════════════════════════════

/// Helper to get the feature branch tip OID from a branched-history repo.
fn feature_tip(dir: &std::path::Path) -> String {
    git(dir, &["rev-parse", "feature"]).stdout.trim().to_string()
}

/// Helper to get the feature branch commits (oldest to newest).
fn feature_commits(dir: &std::path::Path) -> Vec<String> {
    let result = git(
        dir,
        &["log", "--reverse", "--format=%H", "feature", "--not", "main~1"],
    );
    result
        .stdout
        .trim()
        .lines()
        .map(|l| l.to_string())
        .collect()
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let oid_g = feature_tip(dir_git.path());
    let oid_m = feature_tip(dir_gitr.path());

    let g = git(dir_git.path(), &["cherry-pick", &oid_g]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", &oid_m]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Verify same commit message was preserved
    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%s"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%s"]);
    assert_output_eq(&g_msg, &m_msg);
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_no_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let oid_g = feature_tip(dir_git.path());
    let oid_m = feature_tip(dir_gitr.path());

    let g = git(dir_git.path(), &["cherry-pick", "-n", &oid_g]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "-n", &oid_m]);

    assert_exit_code_eq(&g, &m);
    // -n stages but does not commit; HEAD should remain unchanged
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_x_flag() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let oid_g = feature_tip(dir_git.path());
    let oid_m = feature_tip(dir_gitr.path());

    let g = git(dir_git.path(), &["cherry-pick", "-x", &oid_g]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "-x", &oid_m]);

    assert_exit_code_eq(&g, &m);

    // -x appends "(cherry picked from commit ...)" line
    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%B"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%B"]);
    assert!(
        g_msg.stdout.contains("cherry picked from commit"),
        "git -x should add cherry-picked-from line"
    );
    assert!(
        m_msg.stdout.contains("cherry picked from commit"),
        "gitr -x should add cherry-picked-from line"
    );
}

#[test]
fn test_cherry_pick_abort() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());

    // Both are on main. Cherry-pick the feature commit that conflicts.
    let oid_g = git(dir_git.path(), &["rev-parse", "feature"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "feature"]).stdout.trim().to_string();

    let g_head_before = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_before = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // Cherry-pick (should conflict)
    let g = git(dir_git.path(), &["cherry-pick", &oid_g]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", &oid_m]);
    assert_ne!(g.exit_code, 0, "git cherry-pick should conflict");
    assert_ne!(m.exit_code, 0, "gitr cherry-pick should conflict");

    // Abort
    let g_abort = git(dir_git.path(), &["cherry-pick", "--abort"]);
    let m_abort = gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);
    assert_eq!(g_abort.exit_code, 0, "git cherry-pick --abort failed: {}", g_abort.stderr);
    assert_eq!(m_abort.exit_code, 0, "gitr cherry-pick --abort failed: {}", m_abort.stderr);

    // HEAD restored
    let g_head_after = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_after = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    assert_eq!(g_head_before, g_head_after);
    assert_eq!(m_head_before, m_head_after);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_cherry_pick_continue() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());

    let oid_g = git(dir_git.path(), &["rev-parse", "feature"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "feature"]).stdout.trim().to_string();

    // Cherry-pick (conflict)
    git(dir_git.path(), &["cherry-pick", &oid_g]);
    gitr(dir_gitr.path(), &["cherry-pick", &oid_m]);

    // Resolve identically in both
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("conflict.txt"), "resolved\n").unwrap();
        git(dir, &["add", "conflict.txt"]);
    }

    // Continue
    let g = git(dir_git.path(), &["cherry-pick", "--continue"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--continue"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_signoff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let oid_g = feature_tip(dir_git.path());
    let oid_m = feature_tip(dir_gitr.path());

    let g = git(dir_git.path(), &["cherry-pick", "--signoff", &oid_g]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--signoff", &oid_m]);

    assert_exit_code_eq(&g, &m);

    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%B"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%B"]);
    assert!(g_msg.stdout.contains("Signed-off-by:"), "git --signoff should add trailer");
    assert!(m_msg.stdout.contains("Signed-off-by:"), "gitr --signoff should add trailer");
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_range() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // Cherry-pick the feature branch commits as a range from main
    let g = git(dir_git.path(), &["cherry-pick", "feature~2..feature"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "feature~2..feature"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// REVERT
// ════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_revert_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["revert", "--no-edit", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "--no-edit", "HEAD"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Verify revert commit message matches
    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%s"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%s"]);
    assert_output_eq(&g_msg, &m_msg);
}

#[test]
#[ignore] // known parity gap
fn test_revert_no_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["revert", "-n", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "-n", "HEAD"]);

    assert_exit_code_eq(&g, &m);
    // -n does not create a commit; HEAD stays unchanged
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_revert_abort() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Create a conflict scenario for revert
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        std::fs::write(dir.join("file.txt"), "original\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "initial"], &date);

        std::fs::write(dir.join("file.txt"), "modified in c2\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "commit 2"], &date);

        std::fs::write(dir.join("file.txt"), "further modified in c3\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "commit 3"], &date);
    }

    let g_head_before = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_before = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();

    // Revert commit 2 (HEAD~1) -- conflicts with commit 3
    let g = git(dir_git.path(), &["revert", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["revert", "HEAD~1"]);
    assert_ne!(g.exit_code, 0, "git revert should conflict");
    assert_ne!(m.exit_code, 0, "gitr revert should conflict");

    // Abort
    let g_abort = git(dir_git.path(), &["revert", "--abort"]);
    let m_abort = gitr(dir_gitr.path(), &["revert", "--abort"]);
    assert_eq!(g_abort.exit_code, 0, "git revert --abort failed: {}", g_abort.stderr);
    assert_eq!(m_abort.exit_code, 0, "gitr revert --abort failed: {}", m_abort.stderr);

    let g_head_after = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let m_head_after = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    assert_eq!(g_head_before, g_head_after);
    assert_eq!(m_head_before, m_head_after);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_revert_signoff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["revert", "--signoff", "--no-edit", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "--signoff", "--no-edit", "HEAD"]);

    assert_exit_code_eq(&g, &m);

    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%B"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%B"]);
    assert!(g_msg.stdout.contains("Signed-off-by:"), "git --signoff should add trailer");
    assert!(m_msg.stdout.contains("Signed-off-by:"), "gitr --signoff should add trailer");
}

#[test]
#[ignore] // known parity gap
fn test_revert_range() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 4);
    setup_linear_history(dir_gitr.path(), 4);

    // Revert the last 2 commits (HEAD~2..HEAD means HEAD~1 and HEAD)
    let g = git(dir_git.path(), &["revert", "--no-edit", "HEAD~2..HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "--no-edit", "HEAD~2..HEAD"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_revert_merge_m1() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Create a merge commit to revert
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        std::fs::write(dir.join("base.txt"), "base\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "base.txt"], &date);
        git_with_date(dir, &["commit", "-m", "base"], &date);

        // Feature branch with its own file
        git(dir, &["checkout", "-b", "feature"]);
        std::fs::write(dir.join("feat.txt"), "feature\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "feat.txt"], &date);
        git_with_date(dir, &["commit", "-m", "feature work"], &date);

        git(dir, &["checkout", "main"]);
        std::fs::write(dir.join("main.txt"), "main\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "main.txt"], &date);
        git_with_date(dir, &["commit", "-m", "main work"], &date);

        // Merge feature
        git(dir, &["merge", "feature", "--no-edit"]);
    }

    // Revert the merge commit with -m 1 (revert to first parent = main)
    let g = git(dir_git.path(), &["revert", "-m", "1", "--no-edit", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "-m", "1", "--no-edit", "HEAD"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // feat.txt should be removed by the revert
    let g_ls = git(dir_git.path(), &["ls-files"]);
    let m_ls = gitr(dir_gitr.path(), &["ls-files"]);
    assert_output_eq(&g_ls, &m_ls);
}

// ════════════════════════════════════════════════════════════════════════════
// PULL (remote operations over file:// transport)
// ════════════════════════════════════════════════════════════════════════════

/// Set up a pair of cloned repos and push new commits to the remote so both
/// can pull. Returns (remote_dir, pusher_dir) temps that must be kept alive.
fn setup_pull_pair(
    dir_git: &std::path::Path,
    dir_gitr: &std::path::Path,
) -> (tempfile::TempDir, tempfile::TempDir) {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    // Clone into both dirs
    git(dir_git, &["clone", &url, "."]);
    git(dir_git, &["config", "user.name", "Test Author"]);
    git(dir_git, &["config", "user.email", "test@example.com"]);

    gitr(dir_gitr, &["clone", &url, "."]);
    git(dir_gitr, &["config", "user.name", "Test Author"]);
    git(dir_gitr, &["config", "user.email", "test@example.com"]);

    // Push 2 new commits to remote from a third temp
    let pusher = tempfile::tempdir().unwrap();
    git(pusher.path(), &["clone", &url, "."]);
    git(pusher.path(), &["config", "user.name", "Test Author"]);
    git(pusher.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 10u64;
    for i in 0..2 {
        let filename = format!("pushed_{}.txt", i);
        let date = next_date(&mut counter);
        std::fs::write(pusher.path().join(&filename), format!("pushed {}\n", i)).unwrap();
        git_with_date(pusher.path(), &["add", &filename], &date);
        git_with_date(
            pusher.path(),
            &["commit", "-m", &format!("pushed commit {}", i)],
            &date,
        );
    }
    git(pusher.path(), &["push", "origin", "main"]);

    (remote, pusher)
}

#[test]
fn test_pull_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_pull_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["pull"]);
    let m = gitr(dir_gitr.path(), &["pull"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_pull_ff_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_pull_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["pull", "--ff-only"]);
    let m = gitr(dir_gitr.path(), &["pull", "--ff-only"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_pull_no_ff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_pull_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["pull", "--no-ff"]);
    let m = gitr(dir_gitr.path(), &["pull", "--no-ff"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_pull_rebase() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_pull_pair(dir_git.path(), dir_gitr.path());

    // Create a local commit in both so rebase has something to do
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("local.txt"), "local\n").unwrap();
        git(dir, &["add", "local.txt"]);
        git(dir, &["commit", "-m", "local commit"]);
    }

    let g = git(dir_git.path(), &["pull", "--rebase"]);
    let m = gitr(dir_gitr.path(), &["pull", "--rebase"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_pull_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_pull_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["pull", "-q"]);
    let m = gitr(dir_gitr.path(), &["pull", "-q"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_pull_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_pull_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["pull", "--stat"]);
    let m = gitr(dir_gitr.path(), &["pull", "--stat"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// PUSH (remote operations over file:// transport)
// ════════════════════════════════════════════════════════════════════════════

/// Set up a cloned repo pair ready for push tests. Both repos are clones of
/// the same bare remote. Returns the remote tempdir (must be kept alive).
fn setup_push_pair(
    dir_git: &std::path::Path,
    dir_gitr: &std::path::Path,
) -> (tempfile::TempDir, tempfile::TempDir) {
    let remote_git = tempfile::tempdir().unwrap();
    let remote_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_git.path());
    setup_bare_remote(remote_gitr.path());

    let url_git = format!("file://{}", remote_git.path().display());
    let url_gitr = format!("file://{}", remote_gitr.path().display());

    git(dir_git, &["clone", &url_git, "."]);
    git(dir_git, &["config", "user.name", "Test Author"]);
    git(dir_git, &["config", "user.email", "test@example.com"]);

    gitr(dir_gitr, &["clone", &url_gitr, "."]);
    git(dir_gitr, &["config", "user.name", "Test Author"]);
    git(dir_gitr, &["config", "user.email", "test@example.com"]);

    (remote_git, remote_gitr)
}

#[test]
fn test_push_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_rg, _rm) = setup_push_pair(dir_git.path(), dir_gitr.path());

    // Create identical commits in both
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("new.txt"), "new content\n").unwrap();
        git(dir, &["add", "new.txt"]);
        git(dir, &["commit", "-m", "new commit"]);
    }

    let g = git(dir_git.path(), &["push"]);
    let m = gitr(dir_gitr.path(), &["push"]);

    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_push_set_upstream() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_rg, _rm) = setup_push_pair(dir_git.path(), dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("new.txt"), "content\n").unwrap();
        git(dir, &["add", "new.txt"]);
        git(dir, &["commit", "-m", "commit"]);
    }

    let g = git(dir_git.path(), &["push", "-u", "origin", "main"]);
    let m = gitr(dir_gitr.path(), &["push", "-u", "origin", "main"]);

    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // hangs: gitr push blocks on subprocess
fn test_push_tags() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_rg, _rm) = setup_push_pair(dir_git.path(), dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["tag", "v1.0"]);
    }

    let g = git(dir_git.path(), &["push", "--tags"]);
    let m = gitr(dir_gitr.path(), &["push", "--tags"]);

    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_push_dry_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_rg, _rm) = setup_push_pair(dir_git.path(), dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("new.txt"), "content\n").unwrap();
        git(dir, &["add", "new.txt"]);
        git(dir, &["commit", "-m", "commit"]);
    }

    let g = git(dir_git.path(), &["push", "--dry-run"]);
    let m = gitr(dir_gitr.path(), &["push", "--dry-run"]);

    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_push_force() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_rg, _rm) = setup_push_pair(dir_git.path(), dir_gitr.path());

    // Create and push a commit
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("new.txt"), "content\n").unwrap();
        git(dir, &["add", "new.txt"]);
        git(dir, &["commit", "-m", "commit to push"]);
        git(dir, &["push"]);
    }

    // Reset to parent, create a different commit, then force push
    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["reset", "--hard", "HEAD~1"]);
        std::fs::write(dir.join("alt.txt"), "alternative\n").unwrap();
        git(dir, &["add", "alt.txt"]);
        git(dir, &["commit", "-m", "alternative commit"]);
    }

    let g = git(dir_git.path(), &["push", "--force"]);
    let m = gitr(dir_gitr.path(), &["push", "--force"]);

    assert_exit_code_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// FETCH (remote operations over file:// transport)
// ════════════════════════════════════════════════════════════════════════════

/// Set up a clone pair with new commits pushed to the remote.
/// Returns (remote, pusher) temps that must be kept alive.
fn setup_fetch_pair(
    dir_git: &std::path::Path,
    dir_gitr: &std::path::Path,
) -> (tempfile::TempDir, tempfile::TempDir) {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    git(dir_git, &["clone", &url, "."]);
    git(dir_git, &["config", "user.name", "Test Author"]);
    git(dir_git, &["config", "user.email", "test@example.com"]);

    gitr(dir_gitr, &["clone", &url, "."]);
    git(dir_gitr, &["config", "user.name", "Test Author"]);
    git(dir_gitr, &["config", "user.email", "test@example.com"]);

    // Push new commits
    let pusher = tempfile::tempdir().unwrap();
    git(pusher.path(), &["clone", &url, "."]);
    git(pusher.path(), &["config", "user.name", "Test Author"]);
    git(pusher.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 10u64;
    for i in 0..2 {
        let filename = format!("fetched_{}.txt", i);
        let date = next_date(&mut counter);
        std::fs::write(pusher.path().join(&filename), format!("fetched {}\n", i)).unwrap();
        git_with_date(pusher.path(), &["add", &filename], &date);
        git_with_date(
            pusher.path(),
            &["commit", "-m", &format!("fetch commit {}", i)],
            &date,
        );
    }
    // Also push a tag
    git(pusher.path(), &["tag", "v2.0"]);
    // Also push a new branch
    git(pusher.path(), &["checkout", "-b", "develop"]);
    std::fs::write(pusher.path().join("dev.txt"), "dev\n").unwrap();
    git(pusher.path(), &["add", "dev.txt"]);
    git(pusher.path(), &["commit", "-m", "develop commit"]);
    git(pusher.path(), &["push", "origin", "main"]);
    git(pusher.path(), &["push", "origin", "develop"]);
    git(pusher.path(), &["push", "origin", "v2.0"]);

    (remote, pusher)
}

#[test]
fn test_fetch_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["fetch"]);
    let m = gitr(dir_gitr.path(), &["fetch"]);

    assert_exit_code_eq(&g, &m);

    // After fetch, origin/main should point to the same commit
    let g_rev = git(dir_git.path(), &["rev-parse", "origin/main"]);
    let m_rev = gitr(dir_gitr.path(), &["rev-parse", "origin/main"]);
    assert_output_eq(&g_rev, &m_rev);
}

#[test]
fn test_fetch_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["fetch", "--all"]);
    let m = gitr(dir_gitr.path(), &["fetch", "--all"]);

    assert_exit_code_eq(&g, &m);

    let g_rev = git(dir_git.path(), &["rev-parse", "origin/main"]);
    let m_rev = gitr(dir_gitr.path(), &["rev-parse", "origin/main"]);
    assert_output_eq(&g_rev, &m_rev);
}

#[test]
fn test_fetch_tags() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["fetch", "--tags"]);
    let m = gitr(dir_gitr.path(), &["fetch", "--tags"]);

    assert_exit_code_eq(&g, &m);

    // Verify the tag was fetched
    let g_tag = git(dir_git.path(), &["tag", "-l"]);
    let m_tag = gitr(dir_gitr.path(), &["tag", "-l"]);
    assert_output_eq(&g_tag, &m_tag);
}

#[test]
fn test_fetch_prune() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    // First fetch to get all refs
    git(dir_git.path(), &["fetch", "--all"]);
    gitr(dir_gitr.path(), &["fetch", "--all"]);

    // Create stale tracking refs by manually writing a ref that no longer exists on remote
    for dir in [dir_git.path(), dir_gitr.path()] {
        let oid = git(dir, &["rev-parse", "HEAD"]).stdout.trim().to_string();
        let stale_ref = dir.join(".git/refs/remotes/origin/stale-branch");
        std::fs::write(&stale_ref, format!("{}\n", oid)).unwrap();
    }

    let g = git(dir_git.path(), &["fetch", "--prune"]);
    let m = gitr(dir_gitr.path(), &["fetch", "--prune"]);

    assert_exit_code_eq(&g, &m);

    // The stale ref should be pruned
    assert!(
        !dir_git.path().join(".git/refs/remotes/origin/stale-branch").exists(),
        "git fetch --prune should remove stale ref"
    );
    assert!(
        !dir_gitr.path().join(".git/refs/remotes/origin/stale-branch").exists(),
        "gitr fetch --prune should remove stale ref"
    );
}

#[test]
fn test_fetch_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["fetch", "-q"]);
    let m = gitr(dir_gitr.path(), &["fetch", "-q"]);

    assert_exit_code_eq(&g, &m);

    let g_rev = git(dir_git.path(), &["rev-parse", "origin/main"]);
    let m_rev = gitr(dir_gitr.path(), &["rev-parse", "origin/main"]);
    assert_output_eq(&g_rev, &m_rev);
}

// ════════════════════════════════════════════════════════════════════════════
// CLONE
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_clone_basic() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // Compare log
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    // Compare show-ref
    let g = git(dir_git.path(), &["show-ref"]);
    let m = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g, &m);

    // Compare working tree
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_clone_bare() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", "--bare", &url, "."]);
    gitr(dir_gitr.path(), &["clone", "--bare", &url, "."]);

    // Compare show-ref
    let g = git(dir_git.path(), &["show-ref"]);
    let m = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g, &m);

    // Verify bare structure
    assert!(dir_git.path().join("HEAD").exists());
    assert!(dir_gitr.path().join("HEAD").exists());
    assert!(dir_git.path().join("objects").exists());
    assert!(dir_gitr.path().join("objects").exists());
}

#[test]
#[ignore] // known parity gap
fn test_clone_depth_1() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let g = git(dir_git.path(), &["clone", "--depth", "1", &url, "."]);
    let m = gitr(dir_gitr.path(), &["clone", "--depth", "1", &url, "."]);

    assert_exit_code_eq(&g, &m);

    if g.exit_code == 0 {
        // Shallow clone should have exactly 1 commit
        let g_count = git(dir_git.path(), &["rev-list", "--count", "HEAD"]);
        let m_count = gitr(dir_gitr.path(), &["rev-list", "--count", "HEAD"]);
        assert_output_eq(&g_count, &m_count);
        assert_eq!(g_count.stdout.trim(), "1", "shallow clone should have 1 commit");
    }
}

#[test]
fn test_clone_branch() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let g = git(dir_git.path(), &["clone", "-b", "main", &url, "."]);
    let m = gitr(dir_gitr.path(), &["clone", "-b", "main", &url, "."]);

    assert_exit_code_eq(&g, &m);

    if g.exit_code == 0 {
        let g_log = git(dir_git.path(), &["log", "--oneline"]);
        let m_log = gitr(dir_gitr.path(), &["log", "--oneline"]);
        assert_output_eq(&g_log, &m_log);
    }
}

#[test]
fn test_clone_quiet() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let g = git(dir_git.path(), &["clone", "-q", &url, "."]);
    let m = gitr(dir_gitr.path(), &["clone", "-q", &url, "."]);

    assert_exit_code_eq(&g, &m);

    if g.exit_code == 0 {
        let g_log = git(dir_git.path(), &["log", "--oneline"]);
        let m_log = gitr(dir_gitr.path(), &["log", "--oneline"]);
        assert_output_eq(&g_log, &m_log);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Additional edge-case tests
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rebase_already_up_to_date() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Linear history: feature IS main, so rebase is a no-op
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_linear_history(dir, 3);
        git(dir, &["checkout", "-b", "feature"]);
    }

    let g = git(dir_git.path(), &["rebase", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_no_commit_flag_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let oid_g = feature_tip(dir_git.path());
    let oid_m = feature_tip(dir_gitr.path());

    // Use the long form --no-commit
    let g = git(dir_git.path(), &["cherry-pick", "--no-commit", &oid_g]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--no-commit", &oid_m]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_revert_no_commit_long_flag() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["revert", "--no-commit", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["revert", "--no-commit", "HEAD"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_rebase_abort_no_rebase_in_progress() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["rebase", "--abort"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--abort"]);

    // Both should fail
    assert_ne!(g.exit_code, 0, "git should error when no rebase in progress");
    assert_ne!(m.exit_code, 0, "gitr should error when no rebase in progress");
}

#[test]
fn test_cherry_pick_abort_no_pick_in_progress() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["cherry-pick", "--abort"]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", "--abort"]);

    assert_ne!(g.exit_code, 0, "git should error when no cherry-pick in progress");
    assert_ne!(m.exit_code, 0, "gitr should error when no cherry-pick in progress");
}

#[test]
fn test_revert_abort_no_revert_in_progress() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["revert", "--abort"]);
    let m = gitr(dir_gitr.path(), &["revert", "--abort"]);

    assert_ne!(g.exit_code, 0, "git should error when no revert in progress");
    assert_ne!(m.exit_code, 0, "gitr should error when no revert in progress");
}

#[test]
fn test_clone_fsck_clean() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    assert_fsck_clean(dir_git.path());
    assert_fsck_clean(dir_gitr.path());
}

#[test]
fn test_push_creates_valid_remote_objects() {
    let remote_git = tempfile::tempdir().unwrap();
    let remote_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_git.path());
    setup_bare_remote(remote_gitr.path());

    let url_git = format!("file://{}", remote_git.path().display());
    let url_gitr = format!("file://{}", remote_gitr.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url_git, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);

    gitr(dir_gitr.path(), &["clone", &url_gitr, "."]);
    git(dir_gitr.path(), &["config", "user.name", "Test Author"]);
    git(dir_gitr.path(), &["config", "user.email", "test@example.com"]);

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("new.txt"), "content\n").unwrap();
        git(dir, &["add", "new.txt"]);
        git(dir, &["commit", "-m", "new commit"]);
    }

    git(dir_git.path(), &["push"]);
    gitr(dir_gitr.path(), &["push"]);

    // Both bare remotes should pass fsck
    assert_fsck_clean(remote_git.path());
    assert_fsck_clean(remote_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_fetch_new_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    // Fetch all to get the new develop branch
    git(dir_git.path(), &["fetch", "--all"]);
    gitr(dir_gitr.path(), &["fetch", "--all"]);

    // Both should now have origin/develop
    let g_show = git(dir_git.path(), &["show-ref"]);
    let m_show = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g_show, &m_show);

    assert!(
        g_show.stdout.contains("refs/remotes/origin/develop"),
        "git should have origin/develop after fetch"
    );
    assert!(
        m_show.stdout.contains("refs/remotes/origin/develop"),
        "gitr should have origin/develop after fetch"
    );
}

#[test]
fn test_pull_already_up_to_date() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);

    gitr(dir_gitr.path(), &["clone", &url, "."]);
    git(dir_gitr.path(), &["config", "user.name", "Test Author"]);
    git(dir_gitr.path(), &["config", "user.email", "test@example.com"]);

    // Pull when already up to date
    let g = git(dir_git.path(), &["pull"]);
    let m = gitr(dir_gitr.path(), &["pull"]);

    assert_exit_code_eq(&g, &m);
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_rebase_verifies_worktree() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "main"]);

    assert_exit_code_eq(&g, &m);

    // After rebase, feature files should exist
    for f in ["feat_0.txt", "feat_1.txt"] {
        assert!(
            dir_git.path().join(f).exists(),
            "git repo should have {} after rebase",
            f
        );
        assert!(
            dir_gitr.path().join(f).exists(),
            "gitr repo should have {} after rebase",
            f
        );
    }

    // Main files should also exist
    for f in ["main_0.txt", "main_1.txt", "main_2.txt"] {
        assert!(
            dir_git.path().join(f).exists(),
            "git repo should have {} after rebase",
            f
        );
        assert!(
            dir_gitr.path().join(f).exists(),
            "gitr repo should have {} after rebase",
            f
        );
    }
}

#[test]
#[ignore] // known parity gap
fn test_rebase_quiet_long_flag() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["rebase", "--quiet", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "--quiet", "main"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_cherry_pick_preserves_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    let oid_g = feature_tip(dir_git.path());
    let oid_m = feature_tip(dir_gitr.path());

    git(dir_git.path(), &["cherry-pick", &oid_g]);
    gitr(dir_gitr.path(), &["cherry-pick", &oid_m]);

    // The commit message should be preserved
    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%s"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%s"]);
    assert_output_eq(&g_msg, &m_msg);
}

#[test]
fn test_revert_creates_revert_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    git(dir_git.path(), &["revert", "--no-edit", "HEAD"]);
    gitr(dir_gitr.path(), &["revert", "--no-edit", "HEAD"]);

    // The commit message should start with "Revert"
    let g_msg = git(dir_git.path(), &["log", "-1", "--format=%s"]);
    let m_msg = gitr(dir_gitr.path(), &["log", "-1", "--format=%s"]);
    assert_output_eq(&g_msg, &m_msg);
    assert!(
        g_msg.stdout.contains("Revert"),
        "revert commit message should contain 'Revert'"
    );
}

#[test]
fn test_clone_remote_config_matches() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    let g_url = git(dir_git.path(), &["config", "--get", "remote.origin.url"]);
    let m_url = gitr(dir_gitr.path(), &["config", "--get", "remote.origin.url"]);
    assert_output_eq(&g_url, &m_url);

    let g_fetch = git(dir_git.path(), &["config", "--get", "remote.origin.fetch"]);
    let m_fetch = gitr(dir_gitr.path(), &["config", "--get", "remote.origin.fetch"]);
    assert_output_eq(&g_fetch, &m_fetch);
}

#[test]
fn test_push_new_branch_and_fetch() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    // gitr clones, creates a branch, pushes
    let dir_gitr = tempfile::tempdir().unwrap();
    gitr(dir_gitr.path(), &["clone", &url, "."]);
    git(dir_gitr.path(), &["config", "user.name", "Test Author"]);
    git(dir_gitr.path(), &["config", "user.email", "test@example.com"]);

    gitr(dir_gitr.path(), &["checkout", "-b", "new-feature"]);
    std::fs::write(dir_gitr.path().join("feature.txt"), "feature\n").unwrap();
    gitr(dir_gitr.path(), &["add", "feature.txt"]);
    gitr(dir_gitr.path(), &["commit", "-m", "feature commit"]);
    let push_result = gitr(dir_gitr.path(), &["push", "-u", "origin", "new-feature"]);
    assert_eq!(push_result.exit_code, 0, "gitr push should succeed: {}", push_result.stderr);

    // git clones fresh and verifies the new branch
    let dir_git = tempfile::tempdir().unwrap();
    git(dir_git.path(), &["clone", &url, "."]);
    let refs = git(dir_git.path(), &["show-ref"]);
    assert!(
        refs.stdout.contains("refs/remotes/origin/new-feature"),
        "git should see the branch pushed by gitr"
    );
}

#[test]
fn test_fetch_and_merge_matches_pull() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    // Set up clone
    let dir_pull = tempfile::tempdir().unwrap();
    let dir_fetch = tempfile::tempdir().unwrap();

    git(dir_pull.path(), &["clone", &url, "."]);
    git(dir_pull.path(), &["config", "user.name", "Test Author"]);
    git(dir_pull.path(), &["config", "user.email", "test@example.com"]);

    gitr(dir_fetch.path(), &["clone", &url, "."]);
    git(dir_fetch.path(), &["config", "user.name", "Test Author"]);
    git(dir_fetch.path(), &["config", "user.email", "test@example.com"]);

    // Push a new commit to remote
    let pusher = tempfile::tempdir().unwrap();
    git(pusher.path(), &["clone", &url, "."]);
    git(pusher.path(), &["config", "user.name", "Test Author"]);
    git(pusher.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(pusher.path().join("new.txt"), "new\n").unwrap();
    git(pusher.path(), &["add", "new.txt"]);
    git(pusher.path(), &["commit", "-m", "new commit"]);
    git(pusher.path(), &["push", "origin", "main"]);

    // One does pull, other does fetch+merge
    git(dir_pull.path(), &["pull"]);
    gitr(dir_fetch.path(), &["fetch", "origin"]);
    gitr(dir_fetch.path(), &["merge", "origin/main"]);

    // Both should end up at the same state
    assert_head_eq(dir_pull.path(), dir_fetch.path());
    assert_index_eq(dir_pull.path(), dir_fetch.path());
    assert_worktree_eq(dir_pull.path(), dir_fetch.path());
}

#[test]
fn test_rebase_log_matches() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_rebase_scenarios(dir_git.path());
    setup_rebase_scenarios(dir_gitr.path());

    git(dir_git.path(), &["rebase", "main"]);
    gitr(dir_gitr.path(), &["rebase", "main"]);

    // Compare log output after rebase
    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g_log, &m_log);
}

#[test]
fn test_revert_continue_after_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        std::fs::write(dir.join("file.txt"), "original\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "initial"], &date);

        std::fs::write(dir.join("file.txt"), "modified in c2\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "commit 2"], &date);

        std::fs::write(dir.join("file.txt"), "further in c3\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "file.txt"], &date);
        git_with_date(dir, &["commit", "-m", "commit 3"], &date);
    }

    // Revert commit 2 (conflicts with commit 3)
    git(dir_git.path(), &["revert", "HEAD~1"]);
    gitr(dir_gitr.path(), &["revert", "HEAD~1"]);

    // Resolve identically
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("file.txt"), "resolved\n").unwrap();
        git(dir, &["add", "file.txt"]);
    }

    let g = git(dir_git.path(), &["revert", "--continue"]);
    let m = gitr(dir_gitr.path(), &["revert", "--continue"]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_cherry_pick_multiple_commits() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // Get both feature commits
    let commits_g = feature_commits(dir_git.path());
    let commits_m = feature_commits(dir_gitr.path());
    assert!(commits_g.len() >= 2, "Expected at least 2 feature commits");
    assert!(commits_m.len() >= 2, "Expected at least 2 feature commits");

    let g = git(dir_git.path(), &["cherry-pick", &commits_g[0], &commits_g[1]]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", &commits_m[0], &commits_m[1]]);

    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Verify same commit messages
    let g_log = git(dir_git.path(), &["log", "--format=%s"]);
    let m_log = gitr(dir_gitr.path(), &["log", "--format=%s"]);
    assert_output_eq(&g_log, &m_log);
}

#[test]
fn test_clone_head_matches() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // hangs: gitr push blocks on subprocess
fn test_push_nothing_to_push() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_rg, _rm) = setup_push_pair(dir_git.path(), dir_gitr.path());

    // Push without any local commits beyond what's on remote
    let g = git(dir_git.path(), &["push"]);
    let m = gitr(dir_gitr.path(), &["push"]);

    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_fetch_updates_remote_refs() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    let (_remote, _pusher) = setup_fetch_pair(dir_git.path(), dir_gitr.path());

    // Record before
    let g_before = git(dir_git.path(), &["rev-parse", "origin/main"]);
    let m_before = gitr(dir_gitr.path(), &["rev-parse", "origin/main"]);
    assert_output_eq(&g_before, &m_before);

    // Fetch
    git(dir_git.path(), &["fetch"]);
    gitr(dir_gitr.path(), &["fetch"]);

    // After: origin/main should be updated
    let g_after = git(dir_git.path(), &["rev-parse", "origin/main"]);
    let m_after = gitr(dir_gitr.path(), &["rev-parse", "origin/main"]);
    assert_output_eq(&g_after, &m_after);

    // Should have advanced past the initial state
    assert_ne!(
        g_before.stdout.trim(),
        g_after.stdout.trim(),
        "origin/main should have advanced after fetch"
    );
}

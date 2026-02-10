//! End-to-end interop tests for Git Parity Phase 4 — US2 stub subcommand completion.
//!
//! Tests the 9 subcommands that were implemented:
//! - remote get-url, set-head, prune, update, set-branches
//! - reflog expire, delete, exists
//! - maintenance start/stop

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// remote get-url
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_remote_get_url_returns_url() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let url = "https://example.com/repo.git";
    git(dir.path(), &["remote", "add", "origin", url]);

    let g = git(dir.path(), &["remote", "get-url", "origin"]);
    let m = gitr(dir.path(), &["remote", "get-url", "origin"]);

    assert_eq!(g.exit_code, 0, "git remote get-url failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr remote get-url failed: {}", m.stderr);
    assert_output_eq(&g, &m);
    assert_eq!(g.stdout.trim(), url);
}

#[test]
fn test_remote_get_url_nonexistent_remote() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let g = git(dir.path(), &["remote", "get-url", "nosuch"]);
    let m = gitr(dir.path(), &["remote", "get-url", "nosuch"]);

    // Both should fail with non-zero exit code
    assert_ne!(g.exit_code, 0, "git should fail for nonexistent remote");
    assert_ne!(m.exit_code, 0, "gitr should fail for nonexistent remote");
}

#[test]
fn test_remote_get_url_push_flag() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let url = "https://example.com/repo.git";
    git(dir.path(), &["remote", "add", "origin", url]);

    // With --push, should return the push URL (same as fetch URL when no pushurl set)
    let g = git(dir.path(), &["remote", "get-url", "--push", "origin"]);
    let m = gitr(dir.path(), &["remote", "get-url", "--push", "origin"]);

    assert_eq!(g.exit_code, 0, "git remote get-url --push failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr remote get-url --push failed: {}", m.stderr);
    assert_output_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// remote set-head
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_remote_set_head_explicit() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // Set HEAD explicitly
    let g = git(dir_git.path(), &["remote", "set-head", "origin", "main"]);
    let m = gitr(dir_gitr.path(), &["remote", "set-head", "origin", "main"]);

    assert_eq!(g.exit_code, 0, "git remote set-head failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr remote set-head failed: {}", m.stderr);

    // Verify that refs/remotes/origin/HEAD now exists
    let g_ref = git(dir_git.path(), &["symbolic-ref", "refs/remotes/origin/HEAD"]);
    let m_ref = gitr(dir_gitr.path(), &["symbolic-ref", "refs/remotes/origin/HEAD"]);

    assert_eq!(g_ref.exit_code, 0, "git symbolic-ref should succeed: {}", g_ref.stderr);
    assert_eq!(m_ref.exit_code, 0, "gitr symbolic-ref should succeed: {}", m_ref.stderr);
    assert_output_eq(&g_ref, &m_ref);
}

#[test]
fn test_remote_set_head_delete() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir = tempfile::tempdir().unwrap();
    gitr(dir.path(), &["clone", &url, "."]);

    // Set it, then delete it
    gitr(dir.path(), &["remote", "set-head", "origin", "main"]);
    let result = gitr(dir.path(), &["remote", "set-head", "--delete", "origin"]);
    assert_eq!(result.exit_code, 0, "gitr remote set-head --delete failed: {}", result.stderr);
}

// ════════════════════════════════════════════════════════════════════════════
// remote prune (dry-run only -- avoids needing network)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_remote_prune_dry_run_stale_ref() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["clone", &url, "."]);

    // Create a stale tracking ref by writing a ref that does not exist on the remote
    let stale_ref_dir = dir.path().join(".git/refs/remotes/origin");
    std::fs::create_dir_all(&stale_ref_dir).unwrap();

    // Get a valid OID to use for the stale ref
    let oid_result = git(dir.path(), &["rev-parse", "HEAD"]);
    let oid = oid_result.stdout.trim();
    std::fs::write(stale_ref_dir.join("stale-branch"), format!("{}\n", oid)).unwrap();

    // Run prune --dry-run with gitr
    let m = gitr(dir.path(), &["remote", "prune", "--dry-run", "origin"]);
    assert_eq!(m.exit_code, 0, "gitr remote prune --dry-run failed: {}", m.stderr);

    // The stale branch should be listed as "would prune"
    let combined = format!("{}{}", m.stdout, m.stderr);
    assert!(
        combined.contains("stale-branch") || combined.contains("would prune"),
        "gitr remote prune --dry-run should list stale tracking ref.\nstdout: {}\nstderr: {}",
        m.stdout,
        m.stderr
    );
}

// ════════════════════════════════════════════════════════════════════════════
// remote update
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_remote_update_completes() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // Push a new commit to the remote so there's something new to fetch
    let push_dir = tempfile::tempdir().unwrap();
    git(push_dir.path(), &["clone", &url, "."]);
    git(push_dir.path(), &["config", "user.name", "Test Author"]);
    git(push_dir.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(push_dir.path().join("update.txt"), "update content\n").unwrap();
    git(push_dir.path(), &["add", "update.txt"]);
    git(push_dir.path(), &["commit", "-m", "new commit for update"]);
    git(push_dir.path(), &["push", "origin", "main"]);

    let g = git(dir_git.path(), &["remote", "update"]);
    let m = gitr(dir_gitr.path(), &["remote", "update"]);

    assert_eq!(g.exit_code, 0, "git remote update failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr remote update failed: {}", m.stderr);

    // After update, both should see the new commit on origin/main
    let g_log = git(dir_git.path(), &["rev-list", "--count", "origin/main"]);
    let m_log = gitr(dir_gitr.path(), &["rev-list", "--count", "origin/main"]);
    assert_output_eq(&g_log, &m_log);
}

// ════════════════════════════════════════════════════════════════════════════
// remote set-branches
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_remote_set_branches_replaces_refspecs() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let url = "https://example.com/repo.git";
    git(dir.path(), &["remote", "add", "origin", url]);

    // Set branches to track only "main" and "dev"
    let m = gitr(dir.path(), &["remote", "set-branches", "origin", "main", "dev"]);
    assert_eq!(m.exit_code, 0, "gitr remote set-branches failed: {}", m.stderr);

    // Verify config was updated: fetch refspecs should reference main and dev
    let config_content = std::fs::read_to_string(dir.path().join(".git/config")).unwrap();
    assert!(
        config_content.contains("refs/heads/main:refs/remotes/origin/main"),
        "config should contain main refspec after set-branches.\nconfig:\n{}",
        config_content
    );
    assert!(
        config_content.contains("refs/heads/dev:refs/remotes/origin/dev"),
        "config should contain dev refspec after set-branches.\nconfig:\n{}",
        config_content
    );
}

#[test]
fn test_remote_set_branches_nonexistent_remote() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let m = gitr(dir.path(), &["remote", "set-branches", "nosuch", "main"]);
    assert_ne!(m.exit_code, 0, "gitr remote set-branches should fail for nonexistent remote");
}

// ════════════════════════════════════════════════════════════════════════════
// reflog expire
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_expire_now() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Create repos with multiple commits to populate the reflog
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);

    let g = git(dir_git.path(), &["reflog", "expire", "--expire=now", "--all"]);
    let m = gitr(dir_gitr.path(), &["reflog", "expire", "--expire=now", "--all"]);

    assert_eq!(g.exit_code, 0, "git reflog expire failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr reflog expire failed: {}", m.stderr);

    // Both repos should still be valid after expiring reflogs
    assert_fsck_clean(dir_git.path());
    assert_fsck_clean(dir_gitr.path());
}

#[test]
fn test_reflog_expire_preserves_repo_integrity() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    // Get HEAD before expire
    let before = gitr(dir.path(), &["rev-parse", "HEAD"]);
    assert_eq!(before.exit_code, 0);

    gitr(dir.path(), &["reflog", "expire", "--expire=now", "--all"]);

    // HEAD should still point to the same commit
    let after = gitr(dir.path(), &["rev-parse", "HEAD"]);
    assert_eq!(after.exit_code, 0);
    assert_eq!(before.stdout.trim(), after.stdout.trim(), "HEAD should be unchanged after reflog expire");

    assert_fsck_clean(dir.path());
}

// ════════════════════════════════════════════════════════════════════════════
// reflog delete
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_delete_entry() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Create repos with commits to populate the reflog
    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_empty_repo(dir);
        let mut counter = 0u64;
        for i in 0..3 {
            let filename = format!("file_{}.txt", i);
            let content = format!("content for commit {}\n", i);
            std::fs::write(dir.join(&filename), &content).unwrap();
            let date = next_date(&mut counter);
            git_with_date(dir, &["add", &filename], &date);
            let msg = format!("commit {}", i);
            git_with_date(dir, &["commit", "-m", &msg], &date);
        }
    }

    // Count reflog entries before
    let g_before = git(dir_git.path(), &["reflog"]);
    let m_before = gitr(dir_gitr.path(), &["reflog"]);
    let g_count_before = g_before.stdout.lines().count();
    let m_count_before = m_before.stdout.lines().count();
    assert!(g_count_before >= 3, "git should have at least 3 reflog entries, got {}", g_count_before);
    assert!(m_count_before >= 3, "gitr should have at least 3 reflog entries, got {}", m_count_before);

    // Delete the most recent entry
    let g = git(dir_git.path(), &["reflog", "delete", "HEAD@{0}"]);
    let m = gitr(dir_gitr.path(), &["reflog", "delete", "HEAD@{0}"]);

    assert_eq!(g.exit_code, 0, "git reflog delete failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr reflog delete failed: {}", m.stderr);

    // Count reflog entries after: should have one fewer
    let g_after = git(dir_git.path(), &["reflog"]);
    let m_after = gitr(dir_gitr.path(), &["reflog"]);
    let g_count_after = g_after.stdout.lines().count();
    let m_count_after = m_after.stdout.lines().count();

    assert_eq!(
        g_count_after,
        g_count_before - 1,
        "git reflog should have one fewer entry"
    );
    assert_eq!(
        m_count_after,
        m_count_before - 1,
        "gitr reflog should have one fewer entry"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// reflog exists
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_reflog_exists_head() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let g = git(dir.path(), &["reflog", "exists", "HEAD"]);
    let m = gitr(dir.path(), &["reflog", "exists", "HEAD"]);

    assert_eq!(g.exit_code, 0, "git reflog exists HEAD should return 0");
    assert_eq!(m.exit_code, 0, "gitr reflog exists HEAD should return 0");
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_reflog_exists_branch() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let g = git(dir.path(), &["reflog", "exists", "refs/heads/main"]);
    let m = gitr(dir.path(), &["reflog", "exists", "refs/heads/main"]);

    assert_eq!(g.exit_code, 0, "git reflog exists refs/heads/main should return 0");
    assert_eq!(m.exit_code, 0, "gitr reflog exists refs/heads/main should return 0");
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_reflog_exists_nonexistent() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let g = git(dir.path(), &["reflog", "exists", "refs/heads/no-such-branch"]);
    let m = gitr(dir.path(), &["reflog", "exists", "refs/heads/no-such-branch"]);

    // Both should return non-zero for a ref that does not have a reflog
    assert_ne!(g.exit_code, 0, "git reflog exists should return non-zero for nonexistent ref");
    assert_ne!(m.exit_code, 0, "gitr reflog exists should return non-zero for nonexistent ref");
    assert_exit_code_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// maintenance start / stop
// ════════════════════════════════════════════════════════════════════════════
//
// These commands interact with system schedulers (launchd on macOS,
// crontab on Linux), so we only verify that the subcommands are recognized
// and accepted by the CLI parser. They may fail due to permissions in CI,
// so we check for parser acceptance rather than exit code 0.

#[test]
fn test_maintenance_run_accepted() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // `maintenance run` should succeed (runs gc, commit-graph, etc.)
    let m = gitr(dir.path(), &["maintenance", "run", "--quiet"]);
    // Exit code 0 means it ran successfully
    // Any other code means a task may have failed, but the command was recognized
    // The key test is that it does NOT fail with "unknown command" or similar parse error
    assert!(
        !m.stderr.contains("unrecognized") && !m.stderr.contains("unknown subcommand"),
        "gitr should recognize 'maintenance run'.\nstderr: {}",
        m.stderr
    );
}

#[test]
fn test_maintenance_start_accepted() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // `maintenance start` may fail in CI due to launchd/crontab permissions,
    // but it should be recognized as a valid subcommand
    let m = gitr(dir.path(), &["maintenance", "start"]);

    // Check that the command is recognized (not a parser error)
    assert!(
        !m.stderr.contains("unrecognized") && !m.stderr.contains("unknown subcommand"),
        "gitr should recognize 'maintenance start'.\nstderr: {}",
        m.stderr
    );
}

#[test]
fn test_maintenance_stop_accepted() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // `maintenance stop` should be recognized
    let m = gitr(dir.path(), &["maintenance", "stop"]);

    assert!(
        !m.stderr.contains("unrecognized") && !m.stderr.contains("unknown subcommand"),
        "gitr should recognize 'maintenance stop'.\nstderr: {}",
        m.stderr
    );
}

#[test]
fn test_maintenance_register_unregister() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // Register
    let m = gitr(dir.path(), &["maintenance", "register"]);
    assert_eq!(m.exit_code, 0, "gitr maintenance register failed: {}", m.stderr);

    // Verify the marker file was created
    assert!(
        dir.path().join(".git/maintenance/registered").exists(),
        "maintenance register should create a marker file"
    );

    // Unregister
    let m = gitr(dir.path(), &["maintenance", "unregister"]);
    assert_eq!(m.exit_code, 0, "gitr maintenance unregister failed: {}", m.stderr);

    // Verify the marker file was removed
    assert!(
        !dir.path().join(".git/maintenance/registered").exists(),
        "maintenance unregister should remove the marker file"
    );
}

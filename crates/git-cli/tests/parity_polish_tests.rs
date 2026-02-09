mod common;
use common::*;

// ═══════════════════════════════════════════════════════════════════════
// User Story 1 — Core Command Correctness (P0)
// ═══════════════════════════════════════════════════════════════════════

// -- FR-001: diff pathspec disambiguation --

#[test]
fn p0_diff_bare_pathspec() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified content\n").unwrap();

    let g = git(dir, &["diff", "file_0.txt"]);
    let r = gitr(dir, &["diff", "file_0.txt"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p0_diff_double_dash_pathspec() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified content\n").unwrap();

    let g = git(dir, &["diff", "--", "file_0.txt"]);
    let r = gitr(dir, &["diff", "--", "file_0.txt"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p0_diff_rev_double_dash_pathspec() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified content\n").unwrap();

    let g = git(dir, &["diff", "HEAD", "--", "file_0.txt"]);
    let r = gitr(dir, &["diff", "HEAD", "--", "file_0.txt"]);
    assert_output_eq(&g, &r);
}

// -- FR-002: diff --cached hunk header --

#[test]
fn p0_diff_cached_new_file_hunk_header() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_empty_repo(dir);
    std::fs::write(dir.join("new.txt"), "line1\nline2\nline3\n").unwrap();
    git(dir, &["add", "new.txt"]);

    let g = git(dir, &["diff", "--cached"]);
    let r = gitr(dir, &["diff", "--cached"]);
    assert_output_eq(&g, &r);
}

// -- FR-003: reset bare pathspec --

#[test]
fn p0_reset_bare_pathspec() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified\n").unwrap();
    git(dir, &["add", "file_0.txt"]);

    let g = git(dir, &["reset", "file_0.txt"]);
    let r = gitr(dir, &["reset", "file_0.txt"]);
    assert_exit_code_eq(&g, &r);
}

#[test]
fn p0_reset_head_pathspec() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified\n").unwrap();
    git(dir, &["add", "file_0.txt"]);

    let g = git(dir, &["reset", "HEAD", "file_0.txt"]);
    let r = gitr(dir, &["reset", "HEAD", "file_0.txt"]);
    assert_exit_code_eq(&g, &r);
}

// -- FR-004: check-ignore directory patterns --

#[test]
fn p0_check_ignore_dir_pattern_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_empty_repo(dir);
    std::fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    git(dir, &["add", ".gitignore"]);
    git(dir, &["commit", "-m", "add gitignore"]);
    std::fs::create_dir_all(dir.join("build")).unwrap();
    std::fs::write(dir.join("build/output.o"), "binary\n").unwrap();

    let g = git(dir, &["check-ignore", "build/output.o"]);
    let r = gitr(dir, &["check-ignore", "build/output.o"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p0_check_ignore_dir_pattern_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_empty_repo(dir);
    std::fs::write(dir.join(".gitignore"), "build/\n").unwrap();
    git(dir, &["add", ".gitignore"]);
    git(dir, &["commit", "-m", "add gitignore"]);
    std::fs::create_dir_all(dir.join("build")).unwrap();

    let g = git(dir, &["check-ignore", "build"]);
    let r = gitr(dir, &["check-ignore", "build"]);
    assert_output_eq(&g, &r);
}

// -- FR-005: log --since/--until --

#[test]
fn p0_log_since_until() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_empty_repo(dir);

    let mut counter = 0u64;
    for i in 0..5 {
        let filename = format!("file_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        git_with_date(dir, &["commit", "-m", &format!("commit {}", i)], &date);
    }

    let g = git(dir, &["log", "--oneline", "--since=1234567892", "--until=1234567894"]);
    let r = gitr(dir, &["log", "--oneline", "--since=1234567892", "--until=1234567894"]);
    assert_output_eq(&g, &r);
}

// -- FR-006: init nested directory --

#[test]
fn p0_init_nested_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let nested = tmp.path().join("a/b/c");

    let g = git(tmp.path(), &["init", "-b", "main", nested.to_str().unwrap()]);
    assert_eq!(g.exit_code, 0, "git init nested failed: {}", g.stderr);
    assert!(nested.join(".git").exists());

    // Clean up and test gitr
    std::fs::remove_dir_all(&nested).ok();
    let nested2 = tmp.path().join("d/e/f");
    let r = gitr(tmp.path(), &["init", "-b", "main", nested2.to_str().unwrap()]);
    assert_eq!(r.exit_code, 0, "gitr init nested failed: {}", r.stderr);
    assert!(nested2.join(".git").exists());
}

// -- FR-007/008/010/011/012: show fixes --

#[test]
fn p0_show_stat() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 2);

    let g = git(dir, &["show", "--stat", "HEAD"]);
    let r = gitr(dir, &["show", "--stat", "HEAD"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p0_show_format_oneline() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["show", "--format=oneline", "--no-patch", "HEAD"]);
    let r = gitr(dir, &["show", "--format=oneline", "--no-patch", "HEAD"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p0_show_format_raw() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["show", "--format=raw", "--no-patch", "HEAD"]);
    let r = gitr(dir, &["show", "--format=raw", "--no-patch", "HEAD"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p0_show_custom_format() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["show", "--format=%H %s", "--no-patch", "HEAD"]);
    let r = gitr(dir, &["show", "--format=%H %s", "--no-patch", "HEAD"]);
    assert_output_eq(&g, &r);
}

// -- FR-009: commit-tree stdin --

#[test]
fn p0_commit_tree_stdin() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let tree_hash = git(dir, &["rev-parse", "HEAD^{tree}"]);
    let tree = tree_hash.stdout.trim();

    let g = git_stdin(dir, &["commit-tree", tree], b"test message from stdin\n");
    let r = gitr_stdin(dir, &["commit-tree", tree], b"test message from stdin\n");
    assert_eq!(g.exit_code, r.exit_code, "exit codes differ: git={}, gitr={}", g.exit_code, r.exit_code);
    // Both should produce a valid commit hash (40 hex chars)
    assert_eq!(g.stdout.trim().len(), 40, "git commit-tree output not 40 chars");
    assert_eq!(r.stdout.trim().len(), 40, "gitr commit-tree output not 40 chars");
}

// ═══════════════════════════════════════════════════════════════════════
// User Story 2 — Missing Flag Support (P1)
// ═══════════════════════════════════════════════════════════════════════

// -- FR-013: log -N --

#[test]
fn p1_log_dash_n() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 5);

    let g = git(dir, &["log", "--oneline", "-3"]);
    let r = gitr(dir, &["log", "--oneline", "-3"]);
    assert_output_eq(&g, &r);
}

// -- FR-014: log --decorate --

#[test]
fn p1_log_decorate() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 3);
    git(dir, &["branch", "feature"]);
    git(dir, &["tag", "v1.0"]);

    let g = git(dir, &["log", "--oneline", "--decorate", "-1"]);
    let r = gitr(dir, &["log", "--oneline", "--decorate", "-1"]);
    assert_output_eq(&g, &r);
}

// -- FR-015: branch -v --

#[test]
fn p1_branch_verbose() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_branched_history(dir);

    let g = git(dir, &["branch", "-v"]);
    let r = gitr(dir, &["branch", "-v"]);
    assert_output_eq(&g, &r);
}

// -- FR-016: tag -n --

#[test]
fn p1_tag_annotation() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);
    git(dir, &["tag", "-a", "v1.0", "-m", "Release 1.0"]);

    let g = git(dir, &["tag", "-n"]);
    let r = gitr(dir, &["tag", "-n"]);
    assert_output_eq(&g, &r);
}

// -- FR-017: ls-tree -l --

#[test]
fn p1_ls_tree_long() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["ls-tree", "-l", "HEAD"]);
    let r = gitr(dir, &["ls-tree", "-l", "HEAD"]);
    assert_output_eq(&g, &r);
}

// -- FR-018/019: rev-parse --abbrev-ref / --short --

#[test]
fn p1_rev_parse_abbrev_ref() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let r = gitr(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_output_eq(&g, &r);
}

#[test]
fn p1_rev_parse_short() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["rev-parse", "--short", "HEAD"]);
    let r = gitr(dir, &["rev-parse", "--short", "HEAD"]);
    assert_output_eq(&g, &r);
}

// -- FR-020: config --local --

#[test]
fn p1_config_local() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_empty_repo(dir);
    git(dir, &["config", "user.name", "Local User"]);

    let g = git(dir, &["config", "--local", "user.name"]);
    let r = gitr(dir, &["config", "--local", "user.name"]);
    assert_output_eq(&g, &r);
}

// -- FR-021: format-patch --stdout --

#[test]
fn p1_format_patch_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 2);

    let g = git(dir, &["format-patch", "--stdout", "HEAD~1"]);
    let r = gitr(dir, &["format-patch", "--stdout", "HEAD~1"]);
    // Normalize version trailer: git outputs its version, gitr outputs its own
    let g_norm = normalize_patch_version(&g.stdout);
    let r_norm = normalize_patch_version(&r.stdout);
    assert_eq!(g.exit_code, r.exit_code, "Exit code mismatch");
    assert_eq!(g_norm, r_norm,
        "Format-patch stdout mismatch:\n--- git ---\n{}\n--- gitr ---\n{}\n--- end ---",
        g.stdout, r.stdout);
}

/// Strip the version line after `-- ` in format-patch output for comparison.
fn normalize_patch_version(s: &str) -> String {
    let mut out = String::new();
    let mut prev_was_separator = false;
    for line in s.lines() {
        if prev_was_separator {
            // This is the version line — normalize it
            out.push_str("VERSION\n");
            prev_was_separator = false;
            continue;
        }
        if line == "-- " {
            out.push_str("-- \n");
            prev_was_separator = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

// -- FR-022: remote show --

#[test]
fn p1_remote_show() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());
    git(dir, &["clone", &url, "."]);
    git(dir, &["config", "user.name", "Test Author"]);
    git(dir, &["config", "user.email", "test@example.com"]);

    let g = git(dir, &["remote", "show", "origin"]);
    let r = gitr(dir, &["remote", "show", "origin"]);
    assert_output_eq(&g, &r);
}

// ═══════════════════════════════════════════════════════════════════════
// User Story 3 — Output Formatting Fidelity (P2)
// ═══════════════════════════════════════════════════════════════════════

// -- FR-023: date padding --

#[test]
fn p2_date_format_no_padding() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["log", "-1", "--format=%ad"]);
    let r = gitr(dir, &["log", "-1", "--format=%ad"]);
    assert_output_eq(&g, &r);
}

// -- FR-025: diff --stat alignment --

#[test]
fn p2_diff_stat_alignment() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 2);

    let g = git(dir, &["diff", "--stat", "HEAD~1", "HEAD"]);
    let r = gitr(dir, &["diff", "--stat", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &r);
}

// -- FR-026: log --stat blank line --

#[test]
fn p2_log_stat_blank_line() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 2);

    let g = git(dir, &["log", "--stat", "-1"]);
    let r = gitr(dir, &["log", "--stat", "-1"]);
    assert_output_eq(&g, &r);
}

// -- FR-028/029: clean -n sorting --

#[test]
fn p2_clean_sorted_output() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_untracked_files(dir);

    let g = git(dir, &["clean", "-n"]);
    let r = gitr(dir, &["clean", "-n"]);
    assert_output_eq(&g, &r);
}

// -- FR-034: shortlog ordering --

#[test]
fn p2_shortlog_ordering() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 5);

    let g = git(dir, &["shortlog", "HEAD"]);
    let r = gitr(dir, &["shortlog", "HEAD"]);
    assert_output_eq(&g, &r);
}

// ═══════════════════════════════════════════════════════════════════════
// User Story 4 — Exit Code Compatibility (P2)
// ═══════════════════════════════════════════════════════════════════════

// -- FR-035: show-ref --verify exit code --

#[test]
fn exit_code_show_ref_verify_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["show-ref", "--verify", "refs/heads/nonexistent"]);
    let r = gitr(dir, &["show-ref", "--verify", "refs/heads/nonexistent"]);
    assert_exit_code_eq(&g, &r);
}

// -- FR-036: branch -d exit code --

#[test]
fn exit_code_branch_delete_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["branch", "-d", "nonexistent"]);
    let r = gitr(dir, &["branch", "-d", "nonexistent"]);
    assert_exit_code_eq(&g, &r);
}

// -- FR-037: checkout exit code --

#[test]
fn exit_code_checkout_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["checkout", "nonexistent"]);
    let r = gitr(dir, &["checkout", "nonexistent"]);
    assert_exit_code_eq(&g, &r);
}

// -- FR-038: invalid CLI exit code --

#[test]
fn exit_code_invalid_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_linear_history(dir, 1);

    let g = git(dir, &["log", "--bogus-flag"]);
    let r = gitr(dir, &["log", "--bogus-flag"]);
    assert_exit_code_eq(&g, &r);
}

// ═══════════════════════════════════════════════════════════════════════
// User Story 5 — Config and Init Platform Parity (P3)
// ═══════════════════════════════════════════════════════════════════════

// -- FR-040: config --show-origin --

#[test]
fn p3_config_show_origin() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup_empty_repo(dir);
    git(dir, &["config", "user.name", "Origin Test"]);

    let g = git(dir, &["config", "--show-origin", "user.name"]);
    let r = gitr(dir, &["config", "--show-origin", "user.name"]);
    assert_output_eq(&g, &r);
}

// -- FR-041: macOS init config --

#[test]
#[cfg(target_os = "macos")]
fn p3_init_macos_config() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("test_repo");

    let g_dir = tmp.path().join("git_repo");
    git(tmp.path(), &["init", "-b", "main", g_dir.to_str().unwrap()]);
    gitr(tmp.path(), &["init", "-b", "main", dir.to_str().unwrap()]);

    let g_config = std::fs::read_to_string(g_dir.join(".git/config")).unwrap_or_default();
    let r_config = std::fs::read_to_string(dir.join(".git/config")).unwrap_or_default();

    // Both should have ignorecase and precomposeunicode on macOS
    assert!(g_config.contains("ignorecase"), "git config missing ignorecase");
    assert!(r_config.contains("ignorecase"), "gitr config missing ignorecase");
    assert!(g_config.contains("precomposeunicode"), "git config missing precomposeunicode");
    assert!(r_config.contains("precomposeunicode"), "gitr config missing precomposeunicode");
}

// -- FR-042: sample hooks --

#[test]
fn p3_init_sample_hooks() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("test_repo");

    let g_dir = tmp.path().join("git_repo");
    git(tmp.path(), &["init", "-b", "main", g_dir.to_str().unwrap()]);
    gitr(tmp.path(), &["init", "-b", "main", dir.to_str().unwrap()]);

    // Check that sample hooks exist in both
    let expected_hooks = [
        "pre-commit.sample",
        "commit-msg.sample",
        "pre-push.sample",
    ];

    for hook in &expected_hooks {
        let g_hook = g_dir.join(".git/hooks").join(hook);
        let r_hook = dir.join(".git/hooks").join(hook);
        assert!(g_hook.exists(), "git missing hook: {}", hook);
        assert!(r_hook.exists(), "gitr missing hook: {}", hook);
    }
}

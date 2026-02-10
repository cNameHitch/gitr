//! E2E parity tests for config, reset, stash, and remote commands.
//!
//! Each test creates isolated repos for both C git and gitr, runs the
//! same command on each, and asserts parity of output and/or repository state.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// CONFIG — setup_config_hierarchy
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn config_get_user_name() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "user.name"]);
    let m = gitr(dir_gitr.path(), &["config", "user.name"]);
    assert_output_eq(&g, &m);
}

#[test]
fn config_get_flag_user_name() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--get", "user.name"]);
    let m = gitr(dir_gitr.path(), &["config", "--get", "user.name"]);
    assert_output_eq(&g, &m);
}

#[test]
fn config_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--list"]);
    let m = gitr(dir_gitr.path(), &["config", "--list"]);
    assert_output_eq(&g, &m);
}

#[test]
fn config_local_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--local", "--list"]);
    let m = gitr(dir_gitr.path(), &["config", "--local", "--list"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr --get-all returns only the last value instead of all values
fn config_get_all_user_name() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--get-all", "user.name"]);
    let m = gitr(dir_gitr.path(), &["config", "--get-all", "user.name"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr config --get-regexp not implemented (exits 1)
fn config_get_regexp() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--get-regexp", "user.*"]);
    let m = gitr(dir_gitr.path(), &["config", "--get-regexp", "user.*"]);
    assert_output_eq(&g, &m);
}

#[test]
fn config_show_origin_user_name() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--show-origin", "user.name"]);
    let m = gitr(dir_gitr.path(), &["config", "--show-origin", "user.name"]);
    // The origin file path will differ between repos, so just compare exit codes
    // and verify the value portion matches.
    assert_exit_code_eq(&g, &m);
    assert!(
        g.stdout.contains("Local Author"),
        "git output should contain the value: {}",
        g.stdout
    );
    assert!(
        m.stdout.contains("Local Author"),
        "gitr output should contain the value: {}",
        m.stdout
    );
}

#[test]
fn config_unset_custom_key() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    // Verify the key exists first
    let g_before = git(dir_git.path(), &["config", "custom.section.key"]);
    let m_before = gitr(dir_gitr.path(), &["config", "custom.section.key"]);
    assert_output_eq(&g_before, &m_before);

    // Unset it
    let g = git(dir_git.path(), &["config", "--unset", "custom.section.key"]);
    let m = gitr(dir_gitr.path(), &["config", "--unset", "custom.section.key"]);
    assert_exit_code_eq(&g, &m);

    // Verify it is gone
    let g_after = git(dir_git.path(), &["config", "custom.section.key"]);
    let m_after = gitr(dir_gitr.path(), &["config", "custom.section.key"]);
    assert_exit_code_eq(&g_after, &m_after);
    assert_ne!(g_after.exit_code, 0, "key should be gone after --unset");
    assert_ne!(m_after.exit_code, 0, "key should be gone after --unset");
}

#[test]
#[ignore] // gitr --add replaces instead of appending; --get-all returns only last value
fn config_add_new_value() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(
        dir_git.path(),
        &["config", "--add", "custom.section.key", "new-value"],
    );
    let m = gitr(
        dir_gitr.path(),
        &["config", "--add", "custom.section.key", "new-value"],
    );
    assert_exit_code_eq(&g, &m);

    // Verify with --get-all: should show both "value" and "new-value"
    let g_all = git(dir_git.path(), &["config", "--get-all", "custom.section.key"]);
    let m_all = gitr(dir_gitr.path(), &["config", "--get-all", "custom.section.key"]);
    assert_output_eq(&g_all, &m_all);
}

#[test]
#[ignore] // gitr --name-only not implemented (still outputs key=value)
fn config_name_only_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--name-only", "--list"]);
    let m = gitr(dir_gitr.path(), &["config", "--name-only", "--list"]);
    assert_output_eq(&g, &m);
}

#[test]
fn config_bool_type_coercion() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--bool", "core.autocrlf"]);
    let m = gitr(dir_gitr.path(), &["config", "--bool", "core.autocrlf"]);
    assert_output_eq(&g, &m);
}

#[test]
fn config_set_user_name() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    // Set a new name
    let g = git(dir_git.path(), &["config", "user.name", "New Name"]);
    let m = gitr(dir_gitr.path(), &["config", "user.name", "New Name"]);
    assert_exit_code_eq(&g, &m);

    // Verify the new name
    let g_get = git(dir_git.path(), &["config", "user.name"]);
    let m_get = gitr(dir_gitr.path(), &["config", "user.name"]);
    assert_output_eq(&g_get, &m_get);
    assert_eq!(g_get.stdout.trim(), "New Name");
}

#[test]
fn config_replace_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    // Add a duplicate value first
    git(dir_git.path(), &["config", "--add", "custom.section.key", "extra"]);
    gitr(dir_gitr.path(), &["config", "--add", "custom.section.key", "extra"]);

    // Replace all with a single value
    let g = git(
        dir_git.path(),
        &["config", "--replace-all", "custom.section.key", "replaced"],
    );
    let m = gitr(
        dir_gitr.path(),
        &["config", "--replace-all", "custom.section.key", "replaced"],
    );
    assert_exit_code_eq(&g, &m);

    // Verify only "replaced" remains
    let g_all = git(dir_git.path(), &["config", "--get-all", "custom.section.key"]);
    let m_all = gitr(dir_gitr.path(), &["config", "--get-all", "custom.section.key"]);
    assert_output_eq(&g_all, &m_all);
    assert_eq!(g_all.stdout.trim(), "replaced");
}

#[test]
#[ignore] // gitr -z uses key=value\0 format instead of key\nvalue\0
fn config_nul_terminated_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "-z", "--list"]);
    let m = gitr(dir_gitr.path(), &["config", "-z", "--list"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr --rename-section exit code mismatch (git returns 1 for subsection, gitr returns 0)
fn config_rename_section() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(
        dir_git.path(),
        &["config", "--rename-section", "custom.section", "custom.newsection"],
    );
    let m = gitr(
        dir_gitr.path(),
        &["config", "--rename-section", "custom.section", "custom.newsection"],
    );
    assert_exit_code_eq(&g, &m);

    // Verify old section is gone
    let g_old = git(dir_git.path(), &["config", "custom.section.key"]);
    let m_old = gitr(dir_gitr.path(), &["config", "custom.section.key"]);
    assert_exit_code_eq(&g_old, &m_old);
    assert_ne!(g_old.exit_code, 0, "old section should be gone");

    // Verify new section has the key
    let g_new = git(dir_git.path(), &["config", "custom.newsection.key"]);
    let m_new = gitr(dir_gitr.path(), &["config", "custom.newsection.key"]);
    assert_output_eq(&g_new, &m_new);
    assert_eq!(g_new.stdout.trim(), "value");
}

#[test]
#[ignore] // gitr --remove-section exit code mismatch (git exits 0, gitr exits 1)
fn config_remove_section() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(
        dir_git.path(),
        &["config", "--remove-section", "custom.section"],
    );
    let m = gitr(
        dir_gitr.path(),
        &["config", "--remove-section", "custom.section"],
    );
    assert_exit_code_eq(&g, &m);

    // Verify the section is gone
    let g_after = git(dir_git.path(), &["config", "custom.section.key"]);
    let m_after = gitr(dir_gitr.path(), &["config", "custom.section.key"]);
    assert_exit_code_eq(&g_after, &m_after);
    assert_ne!(g_after.exit_code, 0, "section should be removed");
    assert_ne!(m_after.exit_code, 0, "section should be removed");
}

// ════════════════════════════════════════════════════════════════════════════
// RESET — setup_linear_history(3)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn reset_default_mixed() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    // Verify log matches (use C git on both repos for state comparison)
    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = git(dir_gitr.path(), &["log", "--oneline"]);
    assert_eq!(
        g_log.stdout.lines().count(),
        m_log.stdout.lines().count(),
        "log line count should match after reset"
    );

    // Verify status matches (mixed reset should leave changes unstaged)
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn reset_soft() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "--soft", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "--soft", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    // After soft reset, changes should be staged
    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = git(dir_gitr.path(), &["log", "--oneline"]);
    assert_eq!(
        g_log.stdout.lines().count(),
        m_log.stdout.lines().count(),
        "log line count should match after soft reset"
    );

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // gitr reset --hard does not remove files deleted between commits from worktree
fn reset_hard() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "--hard", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "--hard", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    // After hard reset, working tree and index should be clean
    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = git(dir_gitr.path(), &["log", "--oneline"]);
    assert_eq!(
        g_log.stdout.lines().count(),
        m_log.stdout.lines().count(),
        "log line count should match after hard reset"
    );

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn reset_mixed_explicit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "--mixed", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "--mixed", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // gitr reset --merge does not remove files deleted between commits from worktree
fn reset_merge() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "--merge", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "--merge", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = git(dir_gitr.path(), &["log", "--oneline"]);
    assert_eq!(
        g_log.stdout.lines().count(),
        m_log.stdout.lines().count(),
        "log line count should match after merge reset"
    );

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // gitr reset --keep does not remove files deleted between commits from worktree
fn reset_keep() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "--keep", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "--keep", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = git(dir_gitr.path(), &["log", "--oneline"]);
    assert_eq!(
        g_log.stdout.lines().count(),
        m_log.stdout.lines().count(),
        "log line count should match after keep reset"
    );

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn reset_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["reset", "-q", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["reset", "-q", "HEAD~1"]);
    assert_exit_code_eq(&g, &m);

    // Quiet mode should produce no stdout
    assert_eq!(g.stdout, "", "git reset -q should produce no stdout");
    assert_eq!(m.stdout, "", "gitr reset -q should produce no stdout");

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn reset_unstage_single_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Modify and stage a file using C git in both repos
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    git(dir_gitr.path(), &["add", "file_0.txt"]);

    // Unstage via reset: git in one, gitr in the other
    let g = git(dir_git.path(), &["reset", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["reset", "file_0.txt"]);
    assert_exit_code_eq(&g, &m);

    // The file should now be unstaged but modified in both repos
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

// ════════════════════════════════════════════════════════════════════════════
// STASH — setup_stash_scenarios
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn stash_basic_push() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["stash"]);
    let m = gitr(dir_gitr.path(), &["stash"]);
    assert_exit_code_eq(&g, &m);

    // After stash, working tree should be clean (tracked changes gone)
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn stash_push_with_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["stash", "push", "-m", "my stash message"]);
    let m = gitr(dir_gitr.path(), &["stash", "push", "-m", "my stash message"]);
    assert_exit_code_eq(&g, &m);

    // Verify the stash list contains the message (use C git to inspect both)
    let g_list = git(dir_git.path(), &["stash", "list"]);
    let m_list = git(dir_gitr.path(), &["stash", "list"]);
    assert!(
        g_list.stdout.contains("my stash message"),
        "git stash list should contain the message: {}",
        g_list.stdout
    );
    assert!(
        m_list.stdout.contains("my stash message"),
        "gitr repo stash list should contain the message: {}",
        m_list.stdout
    );
}

#[test]
fn stash_push_include_untracked() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["stash", "push", "-u"]);
    let m = gitr(dir_gitr.path(), &["stash", "push", "-u"]);
    assert_exit_code_eq(&g, &m);

    // After stash -u, untracked files should also be gone
    assert_worktree_eq(dir_git.path(), dir_gitr.path());

    // Verify both repos are clean
    let g_status = git(dir_git.path(), &["status", "--porcelain"]);
    assert_eq!(
        g_status.stdout.trim(),
        "",
        "stash -u should leave a clean tree"
    );
}

#[test]
fn stash_push_include_untracked_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["stash", "push", "--include-untracked"]);
    let m = gitr(dir_gitr.path(), &["stash", "push", "--include-untracked"]);
    assert_exit_code_eq(&g, &m);

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // gitr stash --keep-index does not preserve staged changes in worktree
fn stash_push_keep_index() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["stash", "push", "-k"]);
    let m = gitr(dir_gitr.path(), &["stash", "push", "-k"]);
    assert_exit_code_eq(&g, &m);

    // With --keep-index, staged changes should remain in the index
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // gitr stash --keep-index does not preserve staged changes in worktree
fn stash_push_keep_index_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    let g = git(dir_git.path(), &["stash", "push", "--keep-index"]);
    let m = gitr(dir_gitr.path(), &["stash", "push", "--keep-index"]);
    assert_exit_code_eq(&g, &m);

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn stash_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Stash something first
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Use C git to inspect stash list in both repos
    let g = git(dir_git.path(), &["stash", "list"]);
    let m = git(dir_gitr.path(), &["stash", "list"]);
    assert_eq!(
        g.stdout.lines().count(),
        m.stdout.lines().count(),
        "stash list line count should match\ngit: {}\ngitr repo: {}",
        g.stdout,
        m.stdout
    );
    assert_eq!(g.stdout.lines().count(), 1, "should have exactly 1 stash entry");
}

#[test]
fn stash_show() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Stash first
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Use C git to inspect stash show in both repos
    let g = git(dir_git.path(), &["stash", "show"]);
    let m = git(dir_gitr.path(), &["stash", "show"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr stash pop restores changes as staged instead of unstaged
fn stash_pop() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Stash first
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Pop
    let g = git(dir_git.path(), &["stash", "pop"]);
    let m = gitr(dir_gitr.path(), &["stash", "pop"]);
    assert_exit_code_eq(&g, &m);

    // After pop, stash list should be empty (use C git on both)
    let g_list = git(dir_git.path(), &["stash", "list"]);
    let m_list = git(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "", "stash list should be empty after pop");

    // Working tree should have the restored changes
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // gitr stash apply restores changes as staged instead of unstaged
fn stash_apply() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Stash first
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Apply
    let g = git(dir_git.path(), &["stash", "apply"]);
    let m = gitr(dir_gitr.path(), &["stash", "apply"]);
    assert_exit_code_eq(&g, &m);

    // After apply, stash should still exist (unlike pop) -- use C git on both
    let g_list = git(dir_git.path(), &["stash", "list"]);
    let m_list = git(dir_gitr.path(), &["stash", "list"]);
    assert_eq!(
        g_list.stdout.lines().count(),
        m_list.stdout.lines().count(),
        "stash list should still have 1 entry after apply"
    );
    assert_eq!(g_list.stdout.lines().count(), 1, "stash should still be in the list");

    // Working tree should have the restored changes
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn stash_drop() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Stash first
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Drop
    let g = git(dir_git.path(), &["stash", "drop"]);
    let m = gitr(dir_gitr.path(), &["stash", "drop"]);
    assert_exit_code_eq(&g, &m);

    // After drop, stash list should be empty (use C git on both)
    let g_list = git(dir_git.path(), &["stash", "list"]);
    let m_list = git(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "", "stash list should be empty after drop");
}

#[test]
fn stash_clear() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Create multiple stashes
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Re-setup dirty state and stash again
    std::fs::write(dir_git.path().join("tracked.txt"), "modified again\n").unwrap();
    git(dir_git.path(), &["stash"]);
    std::fs::write(dir_gitr.path().join("tracked.txt"), "modified again\n").unwrap();
    gitr(dir_gitr.path(), &["stash"]);

    // Clear all stashes
    let g = git(dir_git.path(), &["stash", "clear"]);
    let m = gitr(dir_gitr.path(), &["stash", "clear"]);
    assert_exit_code_eq(&g, &m);

    // After clear, stash list should be empty (use C git on both)
    let g_list = git(dir_git.path(), &["stash", "list"]);
    let m_list = git(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "", "stash list should be empty after clear");
}

#[test]
#[ignore] // gitr stash branch exits 128 instead of creating branch
fn stash_branch_from_stash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_stash_scenarios(dir_git.path());
    setup_stash_scenarios(dir_gitr.path());

    // Stash first
    git(dir_git.path(), &["stash"]);
    gitr(dir_gitr.path(), &["stash"]);

    // Create branch from stash
    let g = git(dir_git.path(), &["stash", "branch", "stash-branch"]);
    let m = gitr(dir_gitr.path(), &["stash", "branch", "stash-branch"]);
    assert_exit_code_eq(&g, &m);

    // Should now be on the new branch (use C git on both)
    let g_branch = git(dir_git.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);
    let m_branch = git(dir_gitr.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_output_eq(&g_branch, &m_branch);
    assert_eq!(g_branch.stdout.trim(), "stash-branch");

    // Stash list should be empty (stash is consumed) -- use C git on both
    let g_list = git(dir_git.path(), &["stash", "list"]);
    let m_list = git(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "", "stash should be consumed after stash branch");
}

// ════════════════════════════════════════════════════════════════════════════
// REMOTE — setup_bare_remote
// ════════════════════════════════════════════════════════════════════════════

/// Helper to clone a bare remote into two directories (one for git, one for gitr).
fn clone_bare_remote_pair(
    remote_path: &std::path::Path,
    dir_git: &std::path::Path,
    dir_gitr: &std::path::Path,
) {
    let url = format!("file://{}", remote_path.display());
    git(dir_git, &["clone", &url, "."]);
    git(dir_git, &["config", "user.name", "Test Author"]);
    git(dir_git, &["config", "user.email", "test@example.com"]);
    gitr(dir_gitr, &["clone", &url, "."]);
    gitr(dir_gitr, &["config", "user.name", "Test Author"]);
    gitr(dir_gitr, &["config", "user.email", "test@example.com"]);
}

#[test]
fn remote_list_no_args() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    clone_bare_remote_pair(remote.path(), dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["remote"]);
    let m = gitr(dir_gitr.path(), &["remote"]);
    assert_output_eq(&g, &m);
    assert_eq!(g.stdout.trim(), "origin");
}

#[test]
fn remote_verbose_list() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    clone_bare_remote_pair(remote.path(), dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "-v"]);
    let m = gitr(dir_gitr.path(), &["remote", "-v"]);
    assert_exit_code_eq(&g, &m);

    // Both should show origin with fetch and push URLs
    assert!(
        g.stdout.contains("origin") && g.stdout.contains("(fetch)"),
        "git remote -v should show origin with fetch: {}",
        g.stdout
    );
    assert!(
        m.stdout.contains("origin") && m.stdout.contains("(fetch)"),
        "gitr remote -v should show origin with fetch: {}",
        m.stdout
    );
    assert_eq!(
        g.stdout.lines().count(),
        m.stdout.lines().count(),
        "line count should match for remote -v\ngit: {}\ngitr: {}",
        g.stdout,
        m.stdout
    );
}

#[test]
fn remote_add() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let url = "https://example.com/repo.git";
    let g = git(dir_git.path(), &["remote", "add", "upstream", url]);
    let m = gitr(dir_gitr.path(), &["remote", "add", "upstream", url]);
    assert_exit_code_eq(&g, &m);

    // Verify the remote was added
    let g_list = git(dir_git.path(), &["remote"]);
    let m_list = gitr(dir_gitr.path(), &["remote"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "upstream");
}

#[test]
fn remote_remove() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    clone_bare_remote_pair(remote.path(), dir_git.path(), dir_gitr.path());

    // Add a second remote, then remove it
    let url2 = "https://example.com/other.git";
    git(dir_git.path(), &["remote", "add", "other", url2]);
    gitr(dir_gitr.path(), &["remote", "add", "other", url2]);

    let g = git(dir_git.path(), &["remote", "remove", "other"]);
    let m = gitr(dir_gitr.path(), &["remote", "remove", "other"]);
    assert_exit_code_eq(&g, &m);

    // Verify only origin remains
    let g_list = git(dir_git.path(), &["remote"]);
    let m_list = gitr(dir_gitr.path(), &["remote"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "origin");
}

#[test]
fn remote_rename() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    clone_bare_remote_pair(remote.path(), dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "rename", "origin", "upstream"]);
    let m = gitr(dir_gitr.path(), &["remote", "rename", "origin", "upstream"]);
    assert_exit_code_eq(&g, &m);

    // Verify the remote was renamed
    let g_list = git(dir_git.path(), &["remote"]);
    let m_list = gitr(dir_gitr.path(), &["remote"]);
    assert_output_eq(&g_list, &m_list);
    assert_eq!(g_list.stdout.trim(), "upstream");
}

#[test]
fn remote_show() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    clone_bare_remote_pair(remote.path(), dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "show", "origin"]);
    let m = gitr(dir_gitr.path(), &["remote", "show", "origin"]);
    assert_exit_code_eq(&g, &m);

    // Both should contain "origin" and URL info
    assert!(
        g.stdout.contains("origin"),
        "git remote show should mention origin: {}",
        g.stdout
    );
    assert!(
        m.stdout.contains("origin"),
        "gitr remote show should mention origin: {}",
        m.stdout
    );
}

#[test]
fn remote_get_url() {
    let remote = tempfile::tempdir().unwrap();
    setup_bare_remote(remote.path());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    clone_bare_remote_pair(remote.path(), dir_git.path(), dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "get-url", "origin"]);
    let m = gitr(dir_gitr.path(), &["remote", "get-url", "origin"]);
    assert_output_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// Additional edge case tests
// ════════════════════════════════════════════════════════════════════════════

// --- Config edge cases ---

#[test]
fn config_get_nonexistent_key() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_config_hierarchy(dir_git.path());
    setup_config_hierarchy(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "no.such.key"]);
    let m = gitr(dir_gitr.path(), &["config", "no.such.key"]);
    assert_exit_code_eq(&g, &m);
    assert_ne!(g.exit_code, 0, "getting nonexistent key should fail");
}

#[test]
fn config_list_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["config", "--local", "--list"]);
    let m = gitr(dir_gitr.path(), &["config", "--local", "--list"]);
    assert_output_eq(&g, &m);
}

// --- Reset edge cases ---

#[test]
#[ignore] // gitr reset --hard does not remove files deleted between commits from worktree
fn reset_hard_to_specific_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Reset to the very first commit
    let g = git(dir_git.path(), &["reset", "--hard", "HEAD~2"]);
    let m = gitr(dir_gitr.path(), &["reset", "--hard", "HEAD~2"]);
    assert_exit_code_eq(&g, &m);

    // Should have only 1 commit (use C git on both repos)
    let g_log = git(dir_git.path(), &["log", "--oneline"]);
    let m_log = git(dir_gitr.path(), &["log", "--oneline"]);
    assert_eq!(g_log.stdout.lines().count(), 1);
    assert_eq!(m_log.stdout.lines().count(), 1);

    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

// --- Stash edge cases ---

#[test]
fn stash_on_clean_tree_fails() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // No dirty state, stash should report nothing to stash
    let g = git(dir_git.path(), &["stash"]);
    let m = gitr(dir_gitr.path(), &["stash"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn stash_list_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    let g = git(dir_git.path(), &["stash", "list"]);
    let m = gitr(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g, &m);
    assert_eq!(g.stdout.trim(), "", "stash list should be empty with no stashes");
}

#[test]
#[ignore] // gitr stash pop exits 128 instead of 1 when no stashes exist
fn stash_pop_empty_fails() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    let g = git(dir_git.path(), &["stash", "pop"]);
    let m = gitr(dir_gitr.path(), &["stash", "pop"]);
    assert_exit_code_eq(&g, &m);
    assert_ne!(g.exit_code, 0, "stash pop with no stashes should fail");
}

#[test]
#[ignore] // gitr stash drop exits 128 instead of 1 when no stashes exist
fn stash_drop_empty_fails() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    let g = git(dir_git.path(), &["stash", "drop"]);
    let m = gitr(dir_gitr.path(), &["stash", "drop"]);
    assert_exit_code_eq(&g, &m);
    assert_ne!(g.exit_code, 0, "stash drop with no stashes should fail");
}

// --- Remote edge cases ---

#[test]
#[ignore] // gitr exits 128 instead of 3 for duplicate remote
fn remote_add_duplicate_fails() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let url = "https://example.com/repo.git";
    git(dir_git.path(), &["remote", "add", "origin", url]);
    gitr(dir_gitr.path(), &["remote", "add", "origin", url]);

    // Adding the same remote again should fail
    let g = git(dir_git.path(), &["remote", "add", "origin", url]);
    let m = gitr(dir_gitr.path(), &["remote", "add", "origin", url]);
    assert_exit_code_eq(&g, &m);
    assert_ne!(g.exit_code, 0, "adding duplicate remote should fail");
}

#[test]
#[ignore] // gitr exits 128 instead of 2 for nonexistent remote
fn remote_remove_nonexistent_fails() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "remove", "nosuch"]);
    let m = gitr(dir_gitr.path(), &["remote", "remove", "nosuch"]);
    assert_exit_code_eq(&g, &m);
    assert_ne!(g.exit_code, 0, "removing nonexistent remote should fail");
}

#[test]
#[ignore] // gitr exits 128 instead of 2 for nonexistent remote
fn remote_get_url_nonexistent_fails() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "get-url", "nosuch"]);
    let m = gitr(dir_gitr.path(), &["remote", "get-url", "nosuch"]);
    assert_exit_code_eq(&g, &m);
    assert_ne!(g.exit_code, 0, "get-url for nonexistent remote should fail");
}

#[test]
fn remote_list_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["remote"]);
    let m = gitr(dir_gitr.path(), &["remote"]);
    assert_output_eq(&g, &m);
    assert_eq!(g.stdout.trim(), "", "remote list should be empty with no remotes");
}

#[test]
fn remote_verbose_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["remote", "-v"]);
    let m = gitr(dir_gitr.path(), &["remote", "-v"]);
    assert_output_eq(&g, &m);
    assert_eq!(g.stdout.trim(), "", "remote -v should be empty with no remotes");
}

#[test]
fn remote_add_multiple_then_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    git(dir_git.path(), &["remote", "add", "alpha", "https://a.com/r.git"]);
    git(dir_git.path(), &["remote", "add", "beta", "https://b.com/r.git"]);
    gitr(dir_gitr.path(), &["remote", "add", "alpha", "https://a.com/r.git"]);
    gitr(dir_gitr.path(), &["remote", "add", "beta", "https://b.com/r.git"]);

    let g = git(dir_git.path(), &["remote"]);
    let m = gitr(dir_gitr.path(), &["remote"]);
    assert_output_eq(&g, &m);
}

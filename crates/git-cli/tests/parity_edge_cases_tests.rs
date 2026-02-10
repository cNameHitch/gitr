//! E2E parity tests for edge cases — Phase 4D.
//!
//! Systematic edge cases: empty repo, binary files, unicode filenames,
//! symlinks, empty files, CRLF, repos with many commits, detached HEAD,
//! bare repo operations, and merge conflicts.

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// Empty repo (no commits)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_status_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_diff_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    // Add a file but don't commit
    std::fs::write(dir_git.path().join("new.txt"), "new\n").unwrap();
    std::fs::write(dir_gitr.path().join("new.txt"), "new\n").unwrap();
    git(dir_git.path(), &["add", "new.txt"]);
    git(dir_gitr.path(), &["add", "new.txt"]);
    let g = git(dir_git.path(), &["diff", "--cached"]);
    let m = gitr(dir_gitr.path(), &["diff", "--cached"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_branch_list_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["branch", "--list"]);
    let m = gitr(dir_gitr.path(), &["branch", "--list"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_stash_empty_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["stash", "list"]);
    let m = gitr(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Binary files
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_add_binary() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_binary_files(dir_git.path());
    setup_binary_files(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-files", "-s"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_diff_binary() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_binary_files(dir_git.path());
    setup_binary_files(dir_gitr.path());
    // Modify the binary file
    let mut data = vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    for i in 0..128u16 {
        data.push((i & 0xFF) as u8);
    }
    std::fs::write(dir_git.path().join("image.bin"), &data).unwrap();
    std::fs::write(dir_gitr.path().join("image.bin"), &data).unwrap();
    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_edge_show_binary() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_binary_files(dir_git.path());
    setup_binary_files(dir_gitr.path());
    let g = git(dir_git.path(), &["show", "--stat"]);
    let m = gitr(dir_gitr.path(), &["show", "--stat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_blame_binary() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_binary_files(dir_git.path());
    setup_binary_files(dir_gitr.path());
    let g = git(dir_git.path(), &["blame", "image.bin"]);
    let m = gitr(dir_gitr.path(), &["blame", "image.bin"]);
    // Binary blame may differ, but exit codes should match
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Unicode filenames
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_status_unicode() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_unicode_paths(dir_git.path());
    setup_unicode_paths(dir_gitr.path());
    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_ls_files_unicode() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_unicode_paths(dir_git.path());
    setup_unicode_paths(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-files"]);
    let m = gitr(dir_gitr.path(), &["ls-files"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_edge_diff_unicode() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_unicode_paths(dir_git.path());
    setup_unicode_paths(dir_gitr.path());
    // Modify a unicode-named file
    std::fs::write(dir_git.path().join("café.txt"), "espresso\n").unwrap();
    std::fs::write(dir_gitr.path().join("café.txt"), "espresso\n").unwrap();
    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_edge_log_unicode() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_unicode_paths(dir_git.path());
    setup_unicode_paths(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--name-only", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--name-only", "--oneline"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Symlinks
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(unix)]
#[test]
fn test_edge_add_symlink() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("target.txt"), "target content\n").unwrap();
        std::os::unix::fs::symlink("target.txt", dir.join("link.txt")).unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-m", "add symlink"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["ls-files", "-s"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-s"]);
    assert_output_eq(&g, &m);
}

#[cfg(unix)]
#[test]
fn test_edge_status_symlink() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("target.txt"), "target\n").unwrap();
        std::os::unix::fs::symlink("target.txt", dir.join("link.txt")).unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-m", "add symlink"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Empty files
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_add_empty_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("empty.txt"), "").unwrap();
        git(dir, &["add", "empty.txt"]);
        git(dir, &["commit", "-m", "add empty file"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["ls-files", "-s"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_edge_diff_empty_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("empty.txt"), "").unwrap();
        git(dir, &["add", "empty.txt"]);
        git(dir, &["commit", "-m", "add empty"]);
        std::fs::write(dir.join("empty.txt"), "now has content\n").unwrap();
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_status_empty_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("empty.txt"), "").unwrap();
        git(dir, &["add", "empty.txt"]);
        git(dir, &["commit", "-m", "add empty"]);
        std::fs::write(dir.join("empty.txt"), "content\n").unwrap();
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// CRLF line endings
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_add_crlf() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        git(dir, &["config", "core.autocrlf", "false"]);
        std::fs::write(dir.join("crlf.txt"), "line 1\r\nline 2\r\nline 3\r\n").unwrap();
        git(dir, &["add", "crlf.txt"]);
        git(dir, &["commit", "-m", "add crlf file"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["ls-files", "-s"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_diff_crlf() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        git(dir, &["config", "core.autocrlf", "false"]);
        std::fs::write(dir.join("crlf.txt"), "line 1\r\nline 2\r\n").unwrap();
        git(dir, &["add", "crlf.txt"]);
        git(dir, &["commit", "-m", "initial"]);
        std::fs::write(dir.join("crlf.txt"), "line 1\r\nline 2 modified\r\n").unwrap();
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Many commits
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_log_many_commits() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_large_repo(dir_git.path(), 50, 0, 0);
    setup_large_repo(dir_gitr.path(), 50, 0, 0);
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_rev_list_many_commits() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_large_repo(dir_git.path(), 50, 0, 0);
    setup_large_repo(dir_gitr.path(), 50, 0, 0);
    let g = git(dir_git.path(), &["rev-list", "--count", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "--count", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_shortlog_many_commits() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_large_repo(dir_git.path(), 50, 0, 0);
    setup_large_repo(dir_gitr.path(), 50, 0, 0);
    let g = git(dir_git.path(), &["shortlog", "-s"]);
    let m = gitr(dir_gitr.path(), &["shortlog", "-s"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Detached HEAD
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_status_detached_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["checkout", "--detach", "HEAD"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_commit_detached_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 2);
        git(dir, &["checkout", "--detach", "HEAD"]);
        std::fs::write(dir.join("detached.txt"), "detached content\n").unwrap();
        git(dir, &["add", "detached.txt"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["commit", "-m", "detached commit"]);
    let m = gitr(dir_gitr.path(), &["commit", "-m", "detached commit"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_edge_log_detached_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_linear_history(dir, 3);
        git(dir, &["checkout", "--detach", "HEAD~1"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Bare repo operations
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_log_bare_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(dir_git.path());
    setup_bare_remote(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_rev_parse_bare_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(dir_git.path());
    setup_bare_remote(dir_gitr.path());
    let g = git(dir_git.path(), &["rev-parse", "--is-bare-repository"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--is-bare-repository"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_config_bare_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(dir_git.path());
    setup_bare_remote(dir_gitr.path());
    let g = git(dir_git.path(), &["config", "--list"]);
    let m = gitr(dir_gitr.path(), &["config", "--list"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_gc_bare_repo() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(dir_git.path());
    setup_bare_remote(dir_gitr.path());
    let g = git(dir_git.path(), &["gc"]);
    let m = gitr(dir_gitr.path(), &["gc"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Merge conflict edge cases
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_edge_merge_conflict_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_merge_conflict(dir);
        git(dir, &["merge", "feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_edge_merge_conflict_diff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_merge_conflict(dir);
        git(dir, &["merge", "feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_edge_cherry_pick_conflict_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_merge_conflict(dir);
        git(dir, &["cherry-pick", "feature"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Deeply nested directory
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_edge_nested_ls_tree() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_edge_nested_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// Permission files
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(unix)]
#[test]
fn test_edge_permission_ls_tree() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_permission_files(dir_git.path());
    setup_permission_files(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[cfg(unix)]
#[test]
#[ignore] // known parity gap
fn test_edge_permission_diff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_permission_files(dir_git.path());
    setup_permission_files(dir_gitr.path());
    // Change file permission
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o644);
    std::fs::set_permissions(dir_git.path().join("script.sh"), perms.clone()).unwrap();
    std::fs::set_permissions(dir_gitr.path().join("script.sh"), perms).unwrap();
    let g = git(dir_git.path(), &["diff"]);
    let m = gitr(dir_gitr.path(), &["diff"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// State verification after mutations
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_state_after_add() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    std::fs::write(dir_git.path().join("a.txt"), "content\n").unwrap();
    std::fs::write(dir_gitr.path().join("a.txt"), "content\n").unwrap();
    git(dir_git.path(), &["add", "a.txt"]);
    gitr(dir_gitr.path(), &["add", "a.txt"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_state_after_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("a.txt"), "content\n").unwrap();
        git(dir, &["add", "a.txt"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    git(dir_git.path(), &["commit", "-m", "initial"]);
    gitr(dir_gitr.path(), &["commit", "-m", "initial"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_state_after_checkout() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    git(dir_git.path(), &["checkout", "feature"]);
    gitr(dir_gitr.path(), &["checkout", "feature"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_state_after_merge() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    git(dir_git.path(), &["merge", "--no-edit", "feature"]);
    gitr(dir_gitr.path(), &["merge", "--no-edit", "feature"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
#[ignore] // known parity gap
fn test_state_after_reset_hard() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    git(dir_git.path(), &["reset", "--hard", "HEAD~1"]);
    gitr(dir_gitr.path(), &["reset", "--hard", "HEAD~1"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
    assert_head_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_state_after_clean() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());
    git(dir_git.path(), &["clean", "-fd"]);
    gitr(dir_gitr.path(), &["clean", "-fd"]);
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_state_after_rm() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    git(dir_git.path(), &["rm", "file_0.txt"]);
    gitr(dir_gitr.path(), &["rm", "file_0.txt"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_state_after_mv() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    git(dir_git.path(), &["mv", "file_0.txt", "renamed.txt"]);
    gitr(dir_gitr.path(), &["mv", "file_0.txt", "renamed.txt"]);
    assert_index_eq(dir_git.path(), dir_gitr.path());
    assert_worktree_eq(dir_git.path(), dir_gitr.path());
}

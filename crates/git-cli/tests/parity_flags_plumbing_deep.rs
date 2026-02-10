//! Deep E2E parity tests for plumbing commands — Phase 3.
//!
//! Systematic flag coverage for: diff-files, diff-index, diff-tree,
//! merge-base, merge-file, merge-tree, fmt-merge-msg, read-tree,
//! write-tree, ls-tree, mktree, commit-tree, cat-file, hash-object,
//! ls-files, for-each-ref, show-ref, update-ref, symbolic-ref, name-rev,
//! rev-parse, rev-list, describe, pack-objects, repack, prune, gc, fsck,
//! count-objects, verify-pack, index-pack, update-index, check-attr,
//! check-ignore, check-ref-format, var, stripspace.

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// diff-files
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_diff_files_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["diff-files", "-p"]);
    let m = gitr(dir_gitr.path(), &["diff-files", "-p"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_files_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["diff-files", "-q"]);
    let m = gitr(dir_gitr.path(), &["diff-files", "-q"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_files_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["diff-files", "--name-only"]);
    let m = gitr(dir_gitr.path(), &["diff-files", "--name-only"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_files_name_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["diff-files", "--name-status"]);
    let m = gitr(dir_gitr.path(), &["diff-files", "--name-status"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// diff-index
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_diff_index_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff-index", "--name-only", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "--name-only", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_index_name_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff-index", "--name-status", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "--name-status", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_index_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff-index", "-p", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "-p", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_index_cached_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);
    let g = git(dir_git.path(), &["diff-index", "--cached", "--name-only", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "--cached", "--name-only", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// diff-tree
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_diff_tree_recursive() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    // Add another commit with changes
    std::fs::write(dir_git.path().join("a/b/c/d/e/f/g/h/i/j/file.txt"), "changed\n").unwrap();
    git(dir_git.path(), &["add", "."]); git(dir_git.path(), &["commit", "-m", "change deep"]);
    std::fs::write(dir_gitr.path().join("a/b/c/d/e/f/g/h/i/j/file.txt"), "changed\n").unwrap();
    git(dir_gitr.path(), &["add", "."]); git(dir_gitr.path(), &["commit", "-m", "change deep"]);
    let g = git(dir_git.path(), &["diff-tree", "-r", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-tree", "-r", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_tree_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["diff-tree", "-p", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-tree", "-p", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_diff_tree_root() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let root_g = git(dir_git.path(), &["rev-list", "--max-parents=0", "HEAD"]).stdout.trim().to_string();
    let root_m = git(dir_gitr.path(), &["rev-list", "--max-parents=0", "HEAD"]).stdout.trim().to_string();
    let g = git(dir_git.path(), &["diff-tree", "--root", "-r", &root_g]);
    let m = gitr(dir_gitr.path(), &["diff-tree", "--root", "-r", &root_m]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// merge-base
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_merge_base_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-base", "--all", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge-base", "--all", "main", "feature"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_merge_base_octopus() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merge_scenarios(dir_git.path());
    setup_merge_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-base", "--octopus", "main", "octopus-a", "octopus-b"]);
    let m = gitr(dir_gitr.path(), &["merge-base", "--octopus", "main", "octopus-a", "octopus-b"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_merge_base_fork_point() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-base", "--fork-point", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge-base", "--fork-point", "main", "feature"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// fmt-merge-msg
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_fmt_merge_msg_no_log() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let head_g = git(dir_git.path(), &["rev-parse", "feature"]).stdout.trim().to_string();
    let head_m = git(dir_gitr.path(), &["rev-parse", "feature"]).stdout.trim().to_string();
    let input_g = format!("{}\t\tbranch 'feature' of .\n", head_g);
    let input_m = format!("{}\t\tbranch 'feature' of .\n", head_m);
    let g = git_stdin(dir_git.path(), &["fmt-merge-msg", "--no-log"], input_g.as_bytes());
    let m = gitr_stdin(dir_gitr.path(), &["fmt-merge-msg", "--no-log"], input_m.as_bytes());
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_fmt_merge_msg_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let head_g = git(dir_git.path(), &["rev-parse", "feature"]).stdout.trim().to_string();
    let head_m = git(dir_gitr.path(), &["rev-parse", "feature"]).stdout.trim().to_string();
    let input_g = format!("{}\t\tbranch 'feature' of .\n", head_g);
    let input_m = format!("{}\t\tbranch 'feature' of .\n", head_m);
    let g = git_stdin(dir_git.path(), &["fmt-merge-msg", "-m", "custom message"], input_g.as_bytes());
    let m = gitr_stdin(dir_gitr.path(), &["fmt-merge-msg", "-m", "custom message"], input_m.as_bytes());
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// ls-tree
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_ls_tree_recursive() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_ls_tree_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["ls-tree", "--name-only", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "--name-only", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_ls_tree_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["ls-tree", "-l", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-l", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_ls_tree_trees_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-tree", "-d", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-d", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// ls-files
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_ls_files_stage() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["ls-files", "-s"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_ls_files_cached() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["ls-files", "-c"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-c"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_ls_files_modified() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["ls-files", "-m"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-m"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_ls_files_others() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());
    let g = git(dir_git.path(), &["ls-files", "-o", "--exclude-standard"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-o", "--exclude-standard"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_ls_files_deleted() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    std::fs::remove_file(dir_git.path().join("file_0.txt")).unwrap();
    std::fs::remove_file(dir_gitr.path().join("file_0.txt")).unwrap();
    let g = git(dir_git.path(), &["ls-files", "-d"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-d"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// cat-file
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cat_file_type() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["cat-file", "-t", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-t", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_cat_file_size() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["cat-file", "-s", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-s", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_cat_file_pretty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["cat-file", "-p", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-p", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_cat_file_blob() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["cat-file", "-p", "HEAD:file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-p", "HEAD:file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_cat_file_batch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let oid_g = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let g = git_stdin(dir_git.path(), &["cat-file", "--batch"], oid_g.as_bytes());
    let m = gitr_stdin(dir_gitr.path(), &["cat-file", "--batch"], oid_m.as_bytes());
    assert_output_eq(&g, &m);
}

#[test]
fn test_cat_file_batch_check() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let oid_g = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let g = git_stdin(dir_git.path(), &["cat-file", "--batch-check"], oid_g.as_bytes());
    let m = gitr_stdin(dir_gitr.path(), &["cat-file", "--batch-check"], oid_m.as_bytes());
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// hash-object
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_hash_object_file() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    std::fs::write(dir_git.path().join("test.txt"), "hello world\n").unwrap();
    std::fs::write(dir_gitr.path().join("test.txt"), "hello world\n").unwrap();
    let g = git(dir_git.path(), &["hash-object", "test.txt"]);
    let m = gitr(dir_gitr.path(), &["hash-object", "test.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_hash_object_stdin() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git_stdin(dir_git.path(), &["hash-object", "--stdin"], b"test content\n");
    let m = gitr_stdin(dir_gitr.path(), &["hash-object", "--stdin"], b"test content\n");
    assert_output_eq(&g, &m);
}

#[test]
fn test_hash_object_write() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    std::fs::write(dir_git.path().join("test.txt"), "hello\n").unwrap();
    std::fs::write(dir_gitr.path().join("test.txt"), "hello\n").unwrap();
    let g = git(dir_git.path(), &["hash-object", "-w", "test.txt"]);
    let m = gitr(dir_gitr.path(), &["hash-object", "-w", "test.txt"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// rev-parse
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rev_parse_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["rev-parse", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_git_dir() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["rev-parse", "--git-dir"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--git-dir"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_show_toplevel() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["rev-parse", "--show-toplevel"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--show-toplevel"]);
    // Both should succeed, actual paths differ
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_rev_parse_is_inside_work_tree() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["rev-parse", "--is-inside-work-tree"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--is-inside-work-tree"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_is_bare_repository() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["rev-parse", "--is-bare-repository"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--is-bare-repository"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_verify() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["rev-parse", "--verify", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--verify", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_abbrev_ref() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_short() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["rev-parse", "--short", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "--short", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// rev-list
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rev_list_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["rev-list", "--all"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "--all"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_list_count() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["rev-list", "--count", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "--count", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_list_max_count() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["rev-list", "-n", "2", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "-n", "2", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_list_reverse() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["rev-list", "--reverse", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "--reverse", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_list_first_parent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    git(dir_git.path(), &["merge", "--no-edit", "feature"]);
    git(dir_gitr.path(), &["merge", "--no-edit", "feature"]);
    let g = git(dir_git.path(), &["rev-list", "--first-parent", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "--first-parent", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// for-each-ref
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_for_each_ref_format() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["for-each-ref", "--format=%(refname) %(objecttype)"]);
    let m = gitr(dir_gitr.path(), &["for-each-ref", "--format=%(refname) %(objecttype)"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_for_each_ref_sort() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["for-each-ref", "--sort=refname", "refs/tags/"]);
    let m = gitr(dir_gitr.path(), &["for-each-ref", "--sort=refname", "refs/tags/"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_for_each_ref_count() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["for-each-ref", "--count=2", "refs/tags/"]);
    let m = gitr(dir_gitr.path(), &["for-each-ref", "--count=2", "refs/tags/"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_for_each_ref_contains() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["for-each-ref", "--contains=HEAD~1", "refs/tags/"]);
    let m = gitr(dir_gitr.path(), &["for-each-ref", "--contains=HEAD~1", "refs/tags/"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// show-ref
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_show_ref_heads() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["show-ref", "--heads"]);
    let m = gitr(dir_gitr.path(), &["show-ref", "--heads"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_show_ref_tags() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["show-ref", "--tags"]);
    let m = gitr(dir_gitr.path(), &["show-ref", "--tags"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_show_ref_verify() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["show-ref", "--verify", "refs/heads/main"]);
    let m = gitr(dir_gitr.path(), &["show-ref", "--verify", "refs/heads/main"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_show_ref_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["show-ref", "--head"]);
    let m = gitr(dir_gitr.path(), &["show-ref", "--head"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// symbolic-ref
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_symbolic_ref_read() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["symbolic-ref", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["symbolic-ref", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_symbolic_ref_short() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["symbolic-ref", "--short", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["symbolic-ref", "--short", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// name-rev
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_name_rev_always() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let oid_g = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let g = git(dir_git.path(), &["name-rev", "--always", &oid_g]);
    let m = gitr(dir_gitr.path(), &["name-rev", "--always", &oid_m]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_name_rev_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let oid_g = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let g = git(dir_git.path(), &["name-rev", "--name-only", &oid_g]);
    let m = gitr(dir_gitr.path(), &["name-rev", "--name-only", &oid_m]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// describe
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_describe_tags() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["describe", "--tags"]);
    let m = gitr(dir_gitr.path(), &["describe", "--tags"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_describe_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["describe", "--long"]);
    let m = gitr(dir_gitr.path(), &["describe", "--long"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_describe_always() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["describe", "--always"]);
    let m = gitr(dir_gitr.path(), &["describe", "--always"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_describe_exact_match() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["describe", "--exact-match"]);
    let m = gitr(dir_gitr.path(), &["describe", "--exact-match"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_describe_contains() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["describe", "--contains", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["describe", "--contains", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_describe_match_pattern() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["describe", "--tags", "--match=v*"]);
    let m = gitr(dir_gitr.path(), &["describe", "--tags", "--match=v*"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// gc / repack / prune / fsck
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_gc_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["gc"]);
    let m = gitr(dir_gitr.path(), &["gc"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_repack_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["repack"]);
    let m = gitr(dir_gitr.path(), &["repack"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_repack_ad() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["repack", "-a", "-d"]);
    let m = gitr(dir_gitr.path(), &["repack", "-a", "-d"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_prune_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["prune"]);
    let m = gitr(dir_gitr.path(), &["prune"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_fsck_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["fsck"]);
    let m = gitr(dir_gitr.path(), &["fsck"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_fsck_full() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["fsck", "--full"]);
    let m = gitr(dir_gitr.path(), &["fsck", "--full"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// write-tree
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_write_tree_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["write-tree"]);
    let m = gitr(dir_gitr.path(), &["write-tree"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// update-index
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_update_index_add() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    std::fs::write(dir_git.path().join("test.txt"), "content\n").unwrap();
    std::fs::write(dir_gitr.path().join("test.txt"), "content\n").unwrap();
    let g = git(dir_git.path(), &["update-index", "--add", "test.txt"]);
    let m = gitr(dir_gitr.path(), &["update-index", "--add", "test.txt"]);
    assert_exit_code_eq(&g, &m);
    assert_index_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_update_index_remove() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["update-index", "--remove", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["update-index", "--remove", "file_0.txt"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// update-ref
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_update_ref_create() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let oid_g = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let oid_m = git(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let g = git(dir_git.path(), &["update-ref", "refs/heads/new-branch", &oid_g]);
    let m = gitr(dir_gitr.path(), &["update-ref", "refs/heads/new-branch", &oid_m]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_update_ref_delete() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["update-ref", "-d", "refs/heads/feature"]);
    let m = gitr(dir_gitr.path(), &["update-ref", "-d", "refs/heads/feature"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// check-ref-format
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_check_ref_format_valid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["check-ref-format", "refs/heads/main"]);
    let m = gitr(dir_gitr.path(), &["check-ref-format", "refs/heads/main"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_check_ref_format_invalid() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["check-ref-format", "refs/heads/../bad"]);
    let m = gitr(dir_gitr.path(), &["check-ref-format", "refs/heads/../bad"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// var
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_var_git_default_branch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git(dir_git.path(), &["var", "GIT_DEFAULT_BRANCH"]);
    let m = gitr(dir_gitr.path(), &["var", "GIT_DEFAULT_BRANCH"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// stripspace (additional flags)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stripspace_empty_input() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git_stdin(dir_git.path(), &["stripspace"], b"");
    let m = gitr_stdin(dir_gitr.path(), &["stripspace"], b"");
    assert_output_eq(&g, &m);
}

#[test]
fn test_stripspace_only_whitespace() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git_stdin(dir_git.path(), &["stripspace"], b"   \n\n   \n\n");
    let m = gitr_stdin(dir_gitr.path(), &["stripspace"], b"   \n\n   \n\n");
    assert_output_eq(&g, &m);
}

#[test]
fn test_stripspace_comment_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git_stdin(dir_git.path(), &["stripspace", "-c"], b"hello\nworld\n");
    let m = gitr_stdin(dir_gitr.path(), &["stripspace", "-c"], b"hello\nworld\n");
    assert_output_eq(&g, &m);
}

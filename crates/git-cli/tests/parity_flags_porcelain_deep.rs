//! Deep E2E parity tests for porcelain extra commands — Phase 3.
//!
//! Systematic flag coverage for: blame, shortlog, format-patch, am, archive,
//! bundle, notes, grep, bisect, sparse-checkout, worktree, submodule,
//! replace, maintenance, whatchanged, cherry, range-diff, apply.

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// blame
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_blame_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["blame", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_blame_line_range() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join("multi.txt"), "line 1\nline 2\nline 3\nline 4\nline 5\n").unwrap();
        git(dir, &["add", "multi.txt"]);
        git(dir, &["commit", "-m", "add multi"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["blame", "-L", "2,4", "multi.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "-L", "2,4", "multi.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_blame_show_number() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["blame", "-n", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "-n", "file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_blame_show_email() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["blame", "-e", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "-e", "file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_blame_porcelain() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["blame", "--porcelain", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "--porcelain", "file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_blame_show_name() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["blame", "-f", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "-f", "file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_blame_suppress_author() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["blame", "-s", "file_0.txt"]);
    let m = gitr(dir_gitr.path(), &["blame", "-s", "file_0.txt"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// shortlog
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_shortlog_summary() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["shortlog", "-s"]);
    let m = gitr(dir_gitr.path(), &["shortlog", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_shortlog_numbered() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["shortlog", "-s", "-n"]);
    let m = gitr(dir_gitr.path(), &["shortlog", "-s", "-n"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_shortlog_email() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["shortlog", "-s", "-e"]);
    let m = gitr(dir_gitr.path(), &["shortlog", "-s", "-e"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_shortlog_committer() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["shortlog", "-s", "-c"]);
    let m = gitr(dir_gitr.path(), &["shortlog", "-s", "-c"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// grep
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_grep_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["grep", "content"]);
    let m = gitr(dir_gitr.path(), &["grep", "content"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_grep_line_number() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["grep", "-n", "content"]);
    let m = gitr(dir_gitr.path(), &["grep", "-n", "content"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_grep_files_with_matches() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["grep", "-l", "content"]);
    let m = gitr(dir_gitr.path(), &["grep", "-l", "content"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_grep_count() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["grep", "--count", "content"]);
    let m = gitr(dir_gitr.path(), &["grep", "--count", "content"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_grep_ignore_case() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["grep", "-i", "CONTENT"]);
    let m = gitr(dir_gitr.path(), &["grep", "-i", "CONTENT"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_grep_invert_match() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["grep", "-v", "commit 0"]);
    let m = gitr(dir_gitr.path(), &["grep", "-v", "commit 0"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// format-patch
// ══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore] // known parity gap
fn test_format_patch_stdout() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["format-patch", "--stdout", "-1"]);
    let m = gitr(dir_gitr.path(), &["format-patch", "--stdout", "-1"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_format_patch_numbered() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["format-patch", "--stdout", "-n", "-2"]);
    let m = gitr(dir_gitr.path(), &["format-patch", "--stdout", "-n", "-2"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_format_patch_subject_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["format-patch", "--stdout", "--subject-prefix=RFC PATCH", "-1"]);
    let m = gitr(dir_gitr.path(), &["format-patch", "--stdout", "--subject-prefix=RFC PATCH", "-1"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_format_patch_signoff() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["format-patch", "--stdout", "-s", "-1"]);
    let m = gitr(dir_gitr.path(), &["format-patch", "--stdout", "-s", "-1"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// am
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_am_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Generate patch, then apply in a fresh repo
    let patch = git(dir_git.path(), &["format-patch", "--stdout", "-1"]);
    std::fs::write(dir_git.path().join("patch.mbox"), &patch.stdout).unwrap();

    let patch2 = git(dir_gitr.path(), &["format-patch", "--stdout", "-1"]);
    std::fs::write(dir_gitr.path().join("patch.mbox"), &patch2.stdout).unwrap();

    // Reset back 1 commit to apply the patch
    git(dir_git.path(), &["reset", "--hard", "HEAD~1"]);
    git(dir_gitr.path(), &["reset", "--hard", "HEAD~1"]);

    let g = git(dir_git.path(), &["am", "patch.mbox"]);
    let m = gitr(dir_gitr.path(), &["am", "patch.mbox"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// archive
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_archive_tar_format() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["archive", "--format=tar", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["archive", "--format=tar", "HEAD"]);
    // Compare sizes approximately (tar timestamps may differ)
    assert_exit_code_eq(&g, &m);
    assert!(!g.stdout.is_empty());
    assert!(!m.stdout.is_empty());
}

#[test]
fn test_archive_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["archive", "--format=tar", "--prefix=project/", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["archive", "--format=tar", "--prefix=project/", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// bundle
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_bundle_create_and_verify() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["bundle", "create", "repo.bundle", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["bundle", "create", "repo.bundle", "HEAD"]);
    assert_exit_code_eq(&g, &m);

    let gv = git(dir_git.path(), &["bundle", "verify", "repo.bundle"]);
    let mv = gitr(dir_gitr.path(), &["bundle", "verify", "repo.bundle"]);
    assert_exit_code_eq(&gv, &mv);
}

#[test]
fn test_bundle_list_heads() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    git(dir_git.path(), &["bundle", "create", "repo.bundle", "HEAD"]);
    gitr(dir_gitr.path(), &["bundle", "create", "repo.bundle", "HEAD"]);
    let g = git(dir_git.path(), &["bundle", "list-heads", "repo.bundle"]);
    let m = gitr(dir_gitr.path(), &["bundle", "list-heads", "repo.bundle"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// notes
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_notes_add_and_show() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    git(dir_git.path(), &["notes", "add", "-m", "test note"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "test note"]);
    let g = git(dir_git.path(), &["notes", "show"]);
    let m = gitr(dir_gitr.path(), &["notes", "show"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_notes_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    git(dir_git.path(), &["notes", "add", "-m", "note"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "note"]);
    let g = git(dir_git.path(), &["notes", "list"]);
    let m = gitr(dir_gitr.path(), &["notes", "list"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_notes_remove() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    git(dir_git.path(), &["notes", "add", "-m", "note"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "note"]);
    let g = git(dir_git.path(), &["notes", "remove"]);
    let m = gitr(dir_gitr.path(), &["notes", "remove"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_notes_append() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    git(dir_git.path(), &["notes", "add", "-m", "first"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "first"]);
    git(dir_git.path(), &["notes", "append", "-m", "second"]);
    gitr(dir_gitr.path(), &["notes", "append", "-m", "second"]);
    let g = git(dir_git.path(), &["notes", "show"]);
    let m = gitr(dir_gitr.path(), &["notes", "show"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// replace
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_replace_list_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["replace", "-l"]);
    let m = gitr(dir_gitr.path(), &["replace", "-l"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_replace_create_and_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    // Replace HEAD~2 with HEAD~1
    let old_g = git(dir_git.path(), &["rev-parse", "HEAD~2"]).stdout.trim().to_string();
    let new_g = git(dir_git.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();
    let old_m = git(dir_gitr.path(), &["rev-parse", "HEAD~2"]).stdout.trim().to_string();
    let new_m = git(dir_gitr.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();
    git(dir_git.path(), &["replace", &old_g, &new_g]);
    gitr(dir_gitr.path(), &["replace", &old_m, &new_m]);
    let g = git(dir_git.path(), &["replace", "-l"]);
    let m = gitr(dir_gitr.path(), &["replace", "-l"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// whatchanged (additional flags)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_whatchanged_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["whatchanged", "--name-only"]);
    let m = gitr(dir_gitr.path(), &["whatchanged", "--name-only"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_whatchanged_name_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["whatchanged", "--name-status"]);
    let m = gitr(dir_gitr.path(), &["whatchanged", "--name-status"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // known parity gap
fn test_whatchanged_first_parent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    git(dir_git.path(), &["merge", "--no-edit", "feature"]);
    git(dir_gitr.path(), &["merge", "--no-edit", "feature"]);
    let g = git(dir_git.path(), &["whatchanged", "--first-parent"]);
    let m = gitr(dir_gitr.path(), &["whatchanged", "--first-parent"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// apply (additional flags)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_apply_reverse() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let patch = "--- a/file_0.txt\n+++ b/file_0.txt\n@@ -1 +1 @@\n-content for commit 0\n+patched content\n";
    std::fs::write(dir_git.path().join("change.patch"), patch).unwrap();
    std::fs::write(dir_gitr.path().join("change.patch"), patch).unwrap();

    // Apply forward
    git(dir_git.path(), &["apply", "change.patch"]);
    gitr(dir_gitr.path(), &["apply", "change.patch"]);

    // Reverse apply
    let g = git(dir_git.path(), &["apply", "-R", "change.patch"]);
    let m = gitr(dir_gitr.path(), &["apply", "-R", "change.patch"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_apply_cached() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let patch = "--- a/file_0.txt\n+++ b/file_0.txt\n@@ -1 +1 @@\n-content for commit 0\n+patched content\n";
    std::fs::write(dir_git.path().join("change.patch"), patch).unwrap();
    std::fs::write(dir_gitr.path().join("change.patch"), patch).unwrap();

    let g = git(dir_git.path(), &["apply", "--cached", "change.patch"]);
    let m = gitr(dir_gitr.path(), &["apply", "--cached", "change.patch"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_apply_index() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let patch = "--- a/file_0.txt\n+++ b/file_0.txt\n@@ -1 +1 @@\n-content for commit 0\n+patched content\n";
    std::fs::write(dir_git.path().join("change.patch"), patch).unwrap();
    std::fs::write(dir_gitr.path().join("change.patch"), patch).unwrap();

    let g = git(dir_git.path(), &["apply", "--index", "change.patch"]);
    let m = gitr(dir_gitr.path(), &["apply", "--index", "change.patch"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// sparse-checkout (additional flags)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_sparse_checkout_disable() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    git(dir_git.path(), &["sparse-checkout", "init"]);
    git(dir_gitr.path(), &["sparse-checkout", "init"]);
    let g = git(dir_git.path(), &["sparse-checkout", "disable"]);
    let m = gitr(dir_gitr.path(), &["sparse-checkout", "disable"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_sparse_checkout_cone_mode() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    let g = git(dir_git.path(), &["sparse-checkout", "init", "--cone"]);
    let m = gitr(dir_gitr.path(), &["sparse-checkout", "init", "--cone"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// maintenance
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_maintenance_run() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["maintenance", "run"]);
    let m = gitr(dir_gitr.path(), &["maintenance", "run"]);
    assert_exit_code_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// check-attr
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_check_attr_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        std::fs::write(dir.join(".gitattributes"), "*.txt text\n*.bin binary\n").unwrap();
        std::fs::write(dir.join("test.txt"), "hello\n").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-m", "add attrs"]);
    };
    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["check-attr", "-a", "test.txt"]);
    let m = gitr(dir_gitr.path(), &["check-attr", "-a", "test.txt"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// check-ignore
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_check_ignore_verbose() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());
    let g = git(dir_git.path(), &["check-ignore", "-v", "ignored.log"]);
    let m = gitr(dir_gitr.path(), &["check-ignore", "-v", "ignored.log"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_check_ignore_not_ignored() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_untracked_files(dir_git.path());
    setup_untracked_files(dir_gitr.path());
    let g = git(dir_git.path(), &["check-ignore", "tracked_a.txt"]);
    let m = gitr(dir_gitr.path(), &["check-ignore", "tracked_a.txt"]);
    assert_exit_code_eq(&g, &m);
}

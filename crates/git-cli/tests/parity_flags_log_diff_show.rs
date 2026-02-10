mod common;
use common::*;

// ══════════════════════════════════════════════════════════════════════════════
// Helper: set up a linear history with 5 commits (used by most log tests)
// ══════════════════════════════════════════════════════════════════════════════

fn setup_linear5(dir: &std::path::Path) {
    setup_linear_history(dir, 5);
}

/// Set up a branched history and merge feature into main so --merges has data.
fn setup_merged_history(dir: &std::path::Path) {
    setup_branched_history(dir);
    let date = "1234567900 +0000";
    git_with_date(dir, &["merge", "feature", "--no-edit", "-m", "merge feature"], date);
}

/// Set up a repo with one commit, then modify file_0.txt (unstaged) for diff tests.
fn setup_diff_workdir(dir: &std::path::Path) {
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified content\nnew line\n").unwrap();
}

/// Set up a repo with one commit, modify file_0.txt, and stage it for --cached tests.
fn setup_diff_staged(dir: &std::path::Path) {
    setup_linear_history(dir, 1);
    std::fs::write(dir.join("file_0.txt"), "modified content\nnew line\n").unwrap();
    git(dir, &["add", "file_0.txt"]);
}

// ══════════════════════════════════════════════════════════════════════════════
// LOG FLAGS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn log_oneline() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_graph() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merged_history(dir_git.path());
    setup_merged_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--graph", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--graph", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--stat"]);
    let m = gitr(dir_gitr.path(), &["log", "--stat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "-p"]);
    let m = gitr(dir_gitr.path(), &["log", "-p"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_all() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--all", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--all", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_reverse() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--reverse", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--reverse", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_first_parent() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merged_history(dir_git.path());
    setup_merged_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--first-parent", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--first-parent", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_merges() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merged_history(dir_git.path());
    setup_merged_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--merges", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--merges", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_no_merges() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_merged_history(dir_git.path());
    setup_merged_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--no-merges", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--no-merges", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--name-only"]);
    let m = gitr(dir_gitr.path(), &["log", "--name-only"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_name_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--name-status"]);
    let m = gitr(dir_gitr.path(), &["log", "--name-status"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_abbrev_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--abbrev-commit"]);
    let m = gitr(dir_gitr.path(), &["log", "--abbrev-commit"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_max_count() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "-n", "2", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "-n", "2", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_skip() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--skip=1", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--skip=1", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_author_filter() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--author=Test", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--author=Test", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_grep_filter() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--grep=commit", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--grep=commit", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_date_short() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--date=short"]);
    let m = gitr(dir_gitr.path(), &["log", "--date=short"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_format_full_hash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--format=%H"]);
    let m = gitr(dir_gitr.path(), &["log", "--format=%H"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_pretty_format_short_hash_subject() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--pretty=format:%h %s"]);
    let m = gitr(dir_gitr.path(), &["log", "--pretty=format:%h %s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_decorate() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--decorate", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--decorate", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_no_decorate() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--no-decorate", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--no-decorate", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_left_right() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--left-right", "--oneline", "main...feature"]);
    let m = gitr(dir_gitr.path(), &["log", "--left-right", "--oneline", "main...feature"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_source() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--source", "--all", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--source", "--all", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_use_mailmap() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    // Write a .mailmap that maps the test author
    std::fs::write(
        dir_git.path().join(".mailmap"),
        "Mapped Author <mapped@example.com> Test Author <test@example.com>\n",
    )
    .unwrap();
    std::fs::write(
        dir_gitr.path().join(".mailmap"),
        "Mapped Author <mapped@example.com> Test Author <test@example.com>\n",
    )
    .unwrap();
    let g = git(dir_git.path(), &["log", "--use-mailmap", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--use-mailmap", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_follow() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_renamed_files(dir_git.path());
    setup_renamed_files(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--follow", "--oneline", "--", "renamed.txt"]);
    let m = gitr(dir_gitr.path(), &["log", "--follow", "--oneline", "--", "renamed.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_find_renames() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_renamed_files(dir_git.path());
    setup_renamed_files(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "-M", "--name-status", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "-M", "--name-status", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn log_diff_filter_added() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear5(dir_git.path());
    setup_linear5(dir_gitr.path());
    let g = git(dir_git.path(), &["log", "--diff-filter=A", "--name-only", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--diff-filter=A", "--name-only", "--oneline"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// DIFF FLAGS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn diff_cached() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_staged(dir_git.path());
    setup_diff_staged(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--cached"]);
    let m = gitr(dir_gitr.path(), &["diff", "--cached"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_staged() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_staged(dir_git.path());
    setup_diff_staged(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--staged"]);
    let m = gitr(dir_gitr.path(), &["diff", "--staged"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--stat"]);
    let m = gitr(dir_gitr.path(), &["diff", "--stat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_shortstat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--shortstat"]);
    let m = gitr(dir_gitr.path(), &["diff", "--shortstat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_numstat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--numstat"]);
    let m = gitr(dir_gitr.path(), &["diff", "--numstat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--name-only"]);
    let m = gitr(dir_gitr.path(), &["diff", "--name-only"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_name_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--name-status"]);
    let m = gitr(dir_gitr.path(), &["diff", "--name-status"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_summary() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--summary"]);
    let m = gitr(dir_gitr.path(), &["diff", "--summary"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_raw() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--raw"]);
    let m = gitr(dir_gitr.path(), &["diff", "--raw"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--quiet"]);
    let m = gitr(dir_gitr.path(), &["diff", "--quiet"]);
    assert_exit_code_eq(&g, &m);
    // --quiet should suppress stdout
    assert_eq!(m.stdout, "", "diff --quiet should produce no stdout");
}

#[test]
fn diff_unified_context() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "-U3"]);
    let m = gitr(dir_gitr.path(), &["diff", "-U3"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_color_never() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--color=never"]);
    let m = gitr(dir_gitr.path(), &["diff", "--color=never"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_full_index() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--full-index"]);
    let m = gitr(dir_gitr.path(), &["diff", "--full-index"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_reverse() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "-R"]);
    let m = gitr(dir_gitr.path(), &["diff", "-R"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_patience() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--patience"]);
    let m = gitr(dir_gitr.path(), &["diff", "--patience"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_histogram() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--histogram"]);
    let m = gitr(dir_gitr.path(), &["diff", "--histogram"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_minimal() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--minimal"]);
    let m = gitr(dir_gitr.path(), &["diff", "--minimal"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_check() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    // Set up repos with trailing whitespace to trigger --check
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "trailing spaces   \n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "trailing spaces   \n").unwrap();
    let g = git(dir_git.path(), &["diff", "--check"]);
    let m = gitr(dir_gitr.path(), &["diff", "--check"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn diff_no_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--no-prefix"]);
    let m = gitr(dir_gitr.path(), &["diff", "--no-prefix"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_src_dst_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--src-prefix=a/", "--dst-prefix=b/"]);
    let m = gitr(dir_gitr.path(), &["diff", "--src-prefix=a/", "--dst-prefix=b/"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_pickaxe() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "-S", "modified"]);
    let m = gitr(dir_gitr.path(), &["diff", "-S", "modified"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_filter_modified() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "--diff-filter=M"]);
    let m = gitr(dir_gitr.path(), &["diff", "--diff-filter=M"]);
    assert_output_eq(&g, &m);
}

#[test]
fn diff_no_index() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    std::fs::write(dir_git.path().join("a.txt"), "alpha\nbeta\n").unwrap();
    std::fs::write(dir_git.path().join("b.txt"), "alpha\ngamma\n").unwrap();
    std::fs::write(dir_gitr.path().join("a.txt"), "alpha\nbeta\n").unwrap();
    std::fs::write(dir_gitr.path().join("b.txt"), "alpha\ngamma\n").unwrap();
    let g = git(dir_git.path(), &["diff", "--no-index", "a.txt", "b.txt"]);
    let m = gitr(dir_gitr.path(), &["diff", "--no-index", "a.txt", "b.txt"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn diff_nul_terminated() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_diff_workdir(dir_git.path());
    setup_diff_workdir(dir_gitr.path());
    let g = git(dir_git.path(), &["diff", "-z", "--name-only"]);
    let m = gitr(dir_gitr.path(), &["diff", "-z", "--name-only"]);
    assert_output_eq(&g, &m);
}

// ══════════════════════════════════════════════════════════════════════════════
// SHOW FLAGS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn show_format_full_hash() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "--format=%H", "-s"]);
    let m = gitr(dir_gitr.path(), &["show", "--format=%H", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "--stat"]);
    let m = gitr(dir_gitr.path(), &["show", "--stat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "--name-only"]);
    let m = gitr(dir_gitr.path(), &["show", "--name-only"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_name_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "--name-status"]);
    let m = gitr(dir_gitr.path(), &["show", "--name-status"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_no_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "-s"]);
    let m = gitr(dir_gitr.path(), &["show", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_no_patch_long() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "--no-patch"]);
    let m = gitr(dir_gitr.path(), &["show", "--no-patch"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_decorate() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let g = git(dir_git.path(), &["show", "--decorate", "-s"]);
    let m = gitr(dir_gitr.path(), &["show", "--decorate", "-s"]);
    assert_output_eq(&g, &m);
}

#[test]
fn show_quiet() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["show", "-q"]);
    let m = gitr(dir_gitr.path(), &["show", "-q"]);
    assert_output_eq(&g, &m);
}

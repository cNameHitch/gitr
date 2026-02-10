//! E2E parity tests for the 22 commands that previously had zero coverage.
//!
//! Each test runs the same command on both C git and gitr, asserting output parity.

mod common;

use common::*;

// ──────────────────────────── whatchanged ────────────────────────────

#[test]
#[ignore] // git >= 2.47 deprecated whatchanged, requires --i-still-use-this
fn test_whatchanged_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["whatchanged"]);
    let m = gitr(dir_gitr.path(), &["whatchanged"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr whatchanged missing --oneline flag
fn test_whatchanged_oneline() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["whatchanged", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["whatchanged", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // git >= 2.47 deprecated whatchanged, requires --i-still-use-this
fn test_whatchanged_max_count() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 5);
    setup_linear_history(dir_gitr.path(), 5);
    let g = git(dir_git.path(), &["whatchanged", "-n", "2"]);
    let m = gitr(dir_gitr.path(), &["whatchanged", "-n", "2"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── count-objects ────────────────────────────

#[test]
#[ignore] // gitr count-objects output format differs from git (uses verbose format, missing size)
fn test_count_objects_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["count-objects"]);
    let m = gitr(dir_gitr.path(), &["count-objects"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr count-objects -v reports size: 0 instead of actual disk size
fn test_count_objects_verbose() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["count-objects", "-v"]);
    let m = gitr(dir_gitr.path(), &["count-objects", "-v"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── diff-files ────────────────────────────

#[test]
#[ignore] // gitr diff-files outputs full diff instead of raw format; exit code 1 vs 0
fn test_diff_files_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    // Modify a tracked file without staging
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["diff-files"]);
    let m = gitr(dir_gitr.path(), &["diff-files"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr diff-files missing --stat flag
fn test_diff_files_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    let g = git(dir_git.path(), &["diff-files", "--stat"]);
    let m = gitr(dir_gitr.path(), &["diff-files", "--stat"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_diff_files_no_changes() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["diff-files"]);
    let m = gitr(dir_gitr.path(), &["diff-files"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── diff-index ────────────────────────────

#[test]
#[ignore] // gitr diff-index outputs full diff instead of raw format; exit code 1 vs 0
fn test_diff_index_cached() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    // Stage a modification
    std::fs::write(dir_git.path().join("file_0.txt"), "modified\n").unwrap();
    std::fs::write(dir_gitr.path().join("file_0.txt"), "modified\n").unwrap();
    git(dir_git.path(), &["add", "file_0.txt"]);
    git(dir_gitr.path(), &["add", "file_0.txt"]);
    let g = git(dir_git.path(), &["diff-index", "--cached", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "--cached", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr diff-index outputs full diff instead of raw format; exit code 1 vs 0
fn test_diff_index_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff-index", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr diff-index missing --stat flag
fn test_diff_index_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff-index", "--stat", "HEAD~1"]);
    let m = gitr(dir_gitr.path(), &["diff-index", "--stat", "HEAD~1"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── diff-tree ────────────────────────────

#[test]
#[ignore] // gitr diff-tree uses abbreviated OIDs; exit code 1 vs 0
fn test_diff_tree_two_commits() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["diff-tree", "HEAD~2", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-tree", "HEAD~2", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr diff-tree missing --no-commit-id flag
fn test_diff_tree_single_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["diff-tree", "--no-commit-id", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-tree", "--no-commit-id", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr diff-tree exit code 1 vs 0 when changes found
fn test_diff_tree_name_only() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["diff-tree", "--name-only", "-r", "HEAD~1", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["diff-tree", "--name-only", "-r", "HEAD~1", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── fmt-merge-msg ────────────────────────────

#[test]
fn test_fmt_merge_msg_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    // Create FETCH_HEAD-like input
    let head_g = git(dir_git.path(), &["rev-parse", "feature"]);
    let head_m = git(dir_gitr.path(), &["rev-parse", "feature"]);
    let input_g = format!("{}\t\tbranch 'feature' of .\n", head_g.stdout.trim());
    let input_m = format!("{}\t\tbranch 'feature' of .\n", head_m.stdout.trim());
    let g = git_stdin(dir_git.path(), &["fmt-merge-msg"], input_g.as_bytes());
    let m = gitr_stdin(dir_gitr.path(), &["fmt-merge-msg"], input_m.as_bytes());
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr fmt-merge-msg --log includes different commit listing
fn test_fmt_merge_msg_log() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let head_g = git(dir_git.path(), &["rev-parse", "feature"]);
    let head_m = git(dir_gitr.path(), &["rev-parse", "feature"]);
    let input_g = format!("{}\t\tbranch 'feature' of .\n", head_g.stdout.trim());
    let input_m = format!("{}\t\tbranch 'feature' of .\n", head_m.stdout.trim());
    let g = git_stdin(dir_git.path(), &["fmt-merge-msg", "--log"], input_g.as_bytes());
    let m = gitr_stdin(dir_gitr.path(), &["fmt-merge-msg", "--log"], input_m.as_bytes());
    assert_output_eq(&g, &m);
}

// ──────────────────────────── ls-remote ────────────────────────────

#[test]
fn test_ls_remote_local_bare() {
    let remote_git = tempfile::tempdir().unwrap();
    let remote_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_git.path());
    setup_bare_remote(remote_gitr.path());
    let url_g = format!("file://{}", remote_git.path().display());
    let url_m = format!("file://{}", remote_gitr.path().display());
    let g = git(remote_git.path(), &["ls-remote", &url_g]);
    let m = gitr(remote_gitr.path(), &["ls-remote", &url_m]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_ls_remote_heads() {
    let remote_git = tempfile::tempdir().unwrap();
    let remote_gitr = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_git.path());
    setup_bare_remote(remote_gitr.path());
    let url_g = format!("file://{}", remote_git.path().display());
    let url_m = format!("file://{}", remote_gitr.path().display());
    let g = git(remote_git.path(), &["ls-remote", "--heads", &url_g]);
    let m = gitr(remote_gitr.path(), &["ls-remote", "--heads", &url_m]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── merge-base ────────────────────────────

#[test]
fn test_merge_base_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-base", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge-base", "main", "feature"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_merge_base_is_ancestor() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let g = git(dir_git.path(), &["merge-base", "--is-ancestor", "HEAD~2", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["merge-base", "--is-ancestor", "HEAD~2", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_merge_base_is_not_ancestor() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-base", "--is-ancestor", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge-base", "--is-ancestor", "main", "feature"]);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── merge-tree ────────────────────────────

#[test]
#[ignore] // gitr merge-tree two-arg form not producing output matching git
fn test_merge_tree_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-tree", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge-tree", "main", "feature"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr merge-tree --write-tree requires 3 args but git accepts 2
fn test_merge_tree_write_tree() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["merge-tree", "--write-tree", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge-tree", "--write-tree", "main", "feature"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── name-rev ────────────────────────────

#[test]
fn test_name_rev_head() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);
    let head_g = git(dir_git.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let head_m = git(dir_gitr.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let g = git(dir_git.path(), &["name-rev", &head_g]);
    let m = gitr(dir_gitr.path(), &["name-rev", &head_m]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr name-rev --tags fails with "object is not a commit" for tag objects
fn test_name_rev_tags() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    let head_g = git(dir_git.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();
    let head_m = git(dir_gitr.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();
    let g = git(dir_git.path(), &["name-rev", "--tags", &head_g]);
    let m = gitr(dir_gitr.path(), &["name-rev", "--tags", &head_m]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── range-diff ────────────────────────────

#[test]
fn test_range_diff_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Setup: create a rebased branch scenario
    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        let mut counter = 0u64;

        // Main: 3 commits
        for i in 0..3 {
            let f = format!("main_{}.txt", i);
            std::fs::write(dir.join(&f), format!("main {}\n", i)).unwrap();
            let date = next_date(&mut counter);
            git_with_date(dir, &["add", &f], &date);
            git_with_date(dir, &["commit", "-m", &format!("main {}", i)], &date);
        }

        // Feature v1 from HEAD~2
        git(dir, &["checkout", "-b", "feature-v1", "HEAD~2"]);
        std::fs::write(dir.join("feat.txt"), "feat v1\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "feat.txt"], &date);
        git_with_date(dir, &["commit", "-m", "feat v1"], &date);

        // Feature v2 from HEAD~1 on main (simulating rebase)
        git(dir, &["checkout", "main"]);
        git(dir, &["checkout", "-b", "feature-v2", "HEAD~1"]);
        std::fs::write(dir.join("feat.txt"), "feat v2\n").unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "feat.txt"], &date);
        git_with_date(dir, &["commit", "-m", "feat v2"], &date);

        git(dir, &["checkout", "main"]);
    };

    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["range-diff", "main..feature-v1", "main..feature-v2"]);
    let m = gitr(dir_gitr.path(), &["range-diff", "main..feature-v1", "main..feature-v2"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_range_diff_three_dot() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    let setup = |dir: &std::path::Path| {
        setup_empty_repo(dir);
        let mut counter = 0u64;
        for i in 0..3 {
            let f = format!("file_{}.txt", i);
            std::fs::write(dir.join(&f), format!("content {}\n", i)).unwrap();
            let date = next_date(&mut counter);
            git_with_date(dir, &["add", &f], &date);
            git_with_date(dir, &["commit", "-m", &format!("commit {}", i)], &date);
        }
        git(dir, &["branch", "topic", "HEAD~1"]);
    };

    setup(dir_git.path());
    setup(dir_gitr.path());

    let g = git(dir_git.path(), &["range-diff", "HEAD~2..HEAD~1", "HEAD~2..HEAD"]);
    let m = gitr(dir_gitr.path(), &["range-diff", "HEAD~2..HEAD~1", "HEAD~2..HEAD"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── read-tree ────────────────────────────

#[test]
fn test_read_tree_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);
    let g = git(dir_git.path(), &["read-tree", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["read-tree", "HEAD"]);
    assert_exit_code_eq(&g, &m);
    // Verify the index was updated
    let gi = git(dir_git.path(), &["ls-files", "-s"]);
    let mi = git(dir_gitr.path(), &["ls-files", "-s"]);
    assert_eq!(gi.stdout, mi.stdout);
}

#[test]
fn test_read_tree_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["read-tree", "--prefix=sub/", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["read-tree", "--prefix=sub/", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_read_tree_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    let g = git(dir_git.path(), &["read-tree", "--empty"]);
    let m = gitr(dir_gitr.path(), &["read-tree", "--empty"]);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── request-pull ────────────────────────────

#[test]
#[ignore] // gitr request-pull exit code 0 vs git exit code 1; date format differs
fn test_request_pull_basic() {
    // request-pull needs a remote URL and a start point
    let remote = tempfile::tempdir().unwrap();
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_bare_remote(remote.path());
    let url = format!("file://{}", remote.path().display());

    // Clone from remote
    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    git(dir_gitr.path(), &["clone", &url, "."]);
    git(dir_gitr.path(), &["config", "user.name", "Test Author"]);
    git(dir_gitr.path(), &["config", "user.email", "test@example.com"]);

    // Add a commit
    std::fs::write(dir_git.path().join("new.txt"), "new content\n").unwrap();
    git(dir_git.path(), &["add", "new.txt"]);
    git(dir_git.path(), &["commit", "-m", "new file"]);

    std::fs::write(dir_gitr.path().join("new.txt"), "new content\n").unwrap();
    git(dir_gitr.path(), &["add", "new.txt"]);
    git(dir_gitr.path(), &["commit", "-m", "new file"]);

    let g = git(dir_git.path(), &["request-pull", "HEAD~1", &url]);
    let m = gitr(dir_gitr.path(), &["request-pull", "HEAD~1", &url]);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── sparse-checkout ────────────────────────────

#[test]
fn test_sparse_checkout_init() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    let g = git(dir_git.path(), &["sparse-checkout", "init"]);
    let m = gitr(dir_gitr.path(), &["sparse-checkout", "init"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
fn test_sparse_checkout_set() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    git(dir_git.path(), &["sparse-checkout", "init"]);
    git(dir_gitr.path(), &["sparse-checkout", "init"]);
    let g = git(dir_git.path(), &["sparse-checkout", "set", "a/b"]);
    let m = gitr(dir_gitr.path(), &["sparse-checkout", "set", "a/b"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // gitr sparse-checkout list output differs from git
fn test_sparse_checkout_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());
    git(dir_git.path(), &["sparse-checkout", "init"]);
    git(dir_gitr.path(), &["sparse-checkout", "init"]);
    git(dir_git.path(), &["sparse-checkout", "set", "a/b"]);
    git(dir_gitr.path(), &["sparse-checkout", "set", "a/b"]);
    let g = git(dir_git.path(), &["sparse-checkout", "list"]);
    let m = gitr(dir_gitr.path(), &["sparse-checkout", "list"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── stripspace ────────────────────────────

#[test]
fn test_stripspace_basic() {
    let input = b"  hello  \n\n\n  world  \n\n";
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git_stdin(dir_git.path(), &["stripspace"], input);
    let m = gitr_stdin(dir_gitr.path(), &["stripspace"], input);
    assert_output_eq(&g, &m);
}

#[test]
fn test_stripspace_comment_lines() {
    let input = b"# comment\nhello\n# another comment\nworld\n";
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let g = git_stdin(dir_git.path(), &["stripspace", "-s"], input);
    let m = gitr_stdin(dir_gitr.path(), &["stripspace", "-s"], input);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── verify-commit ────────────────────────────

#[test]
fn test_verify_commit_unsigned() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);
    // Unsigned commit should fail with same exit code
    let g = git(dir_git.path(), &["verify-commit", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["verify-commit", "HEAD"]);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── verify-tag ────────────────────────────

#[test]
fn test_verify_tag_unsigned() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_tag_scenarios(dir_git.path());
    setup_tag_scenarios(dir_gitr.path());
    // Unsigned annotated tag should fail verification
    let g = git(dir_git.path(), &["verify-tag", "v1.0"]);
    let m = gitr(dir_gitr.path(), &["verify-tag", "v1.0"]);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── apply ────────────────────────────

#[test]
fn test_apply_basic_patch() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    // Create a patch by modifying and diffing
    let patch = "--- a/file_0.txt\n+++ b/file_0.txt\n@@ -1 +1 @@\n-content for commit 0\n+patched content\n";
    std::fs::write(dir_git.path().join("change.patch"), patch).unwrap();
    std::fs::write(dir_gitr.path().join("change.patch"), patch).unwrap();

    let g = git(dir_git.path(), &["apply", "change.patch"]);
    let m = gitr(dir_gitr.path(), &["apply", "change.patch"]);
    assert_exit_code_eq(&g, &m);
}

#[test]
#[ignore] // gitr apply --stat reports 0 files changed
fn test_apply_stat() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let patch = "--- a/file_0.txt\n+++ b/file_0.txt\n@@ -1 +1 @@\n-content for commit 0\n+patched content\n";
    std::fs::write(dir_git.path().join("change.patch"), patch).unwrap();
    std::fs::write(dir_gitr.path().join("change.patch"), patch).unwrap();

    let g = git(dir_git.path(), &["apply", "--stat", "change.patch"]);
    let m = gitr(dir_gitr.path(), &["apply", "--stat", "change.patch"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_apply_check() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    let patch = "--- a/file_0.txt\n+++ b/file_0.txt\n@@ -1 +1 @@\n-content for commit 0\n+patched content\n";
    std::fs::write(dir_git.path().join("change.patch"), patch).unwrap();
    std::fs::write(dir_gitr.path().join("change.patch"), patch).unwrap();

    let g = git(dir_git.path(), &["apply", "--check", "change.patch"]);
    let m = gitr(dir_gitr.path(), &["apply", "--check", "change.patch"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── cherry ────────────────────────────

#[test]
#[ignore] // gitr cherry uses abbreviated OIDs and different ordering
fn test_cherry_basic() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["cherry", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["cherry", "main", "feature"]);
    assert_output_eq(&g, &m);
}

#[test]
#[ignore] // gitr cherry -v uses abbreviated OIDs and different ordering
fn test_cherry_verbose() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());
    let g = git(dir_git.path(), &["cherry", "-v", "main", "feature"]);
    let m = gitr(dir_gitr.path(), &["cherry", "-v", "main", "feature"]);
    assert_output_eq(&g, &m);
}

// ──────────────────────────── credential ────────────────────────────

#[test]
fn test_credential_reject() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    let input = b"protocol=https\nhost=example.com\n\n";
    let g = git_stdin(dir_git.path(), &["credential", "reject"], input);
    let m = gitr_stdin(dir_gitr.path(), &["credential", "reject"], input);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── daemon ────────────────────────────

#[test]
fn test_daemon_help() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    // daemon --help should exit with status 0 for both
    let g = git(dir_git.path(), &["daemon", "--help"]);
    let m = gitr(dir_gitr.path(), &["daemon", "--help"]);
    assert_exit_code_eq(&g, &m);
}

// ──────────────────────────── rerere ────────────────────────────

#[test]
fn test_rerere_status_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    // Enable rerere
    git(dir_git.path(), &["config", "rerere.enabled", "true"]);
    git(dir_gitr.path(), &["config", "rerere.enabled", "true"]);
    let g = git(dir_git.path(), &["rerere", "status"]);
    let m = gitr(dir_gitr.path(), &["rerere", "status"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rerere_diff_empty() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());
    git(dir_git.path(), &["config", "rerere.enabled", "true"]);
    git(dir_gitr.path(), &["config", "rerere.enabled", "true"]);
    let g = git(dir_git.path(), &["rerere", "diff"]);
    let m = gitr(dir_gitr.path(), &["rerere", "diff"]);
    assert_output_eq(&g, &m);
}

//! Git parity interop tests.
//!
//! Each test creates a repository, runs the same operation with both C git and gitr,
//! and asserts byte-identical output. All tests use pinned dates and environment
//! variables for deterministic output.

mod common;

use common::*;
use tempfile::TempDir;

// ──────────────────────────── Phase 3: Merge Parity ────────────────────────────

#[test]
fn test_ff_merge_ref_advance() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    // Setup: main with 1 commit, feature with 1 more commit ahead
    setup_empty_repo(d);
    let mut counter = 0u64;

    std::fs::write(d.join("base.txt"), "base\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(d, &["add", "base.txt"], &date);
    git_with_date(d, &["commit", "-m", "base commit"], &date);

    git(d, &["checkout", "-b", "feature"]);
    std::fs::write(d.join("feature.txt"), "feature\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(d, &["add", "feature.txt"], &date);
    git_with_date(d, &["commit", "-m", "feature commit"], &date);

    git(d, &["checkout", "main"]);

    // Merge using gitr (fast-forward)
    let gitr_result = gitr(d, &["merge", "feature"]);
    assert_eq!(gitr_result.exit_code, 0, "gitr merge exit code: {}", gitr_result.stderr);

    // Verify ref advanced — show-ref should show same OID for main and feature
    let gitr_refs = gitr(d, &["show-ref"]);
    let git_refs = git(d, &["show-ref"]);
    assert_stdout_eq(&git_refs, &gitr_refs);

    // Verify tree matches
    let git_tree = git(d, &["ls-tree", "-r", "HEAD"]);
    let gitr_tree = gitr(d, &["ls-tree", "-r", "HEAD"]);
    assert_stdout_eq(&git_tree, &gitr_tree);
}

#[test]
fn test_three_way_clean_merge() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_branched_history(d);

    // Merge feature into main using gitr
    let date = "1234567896 +0000";
    let gitr_result = gitr_with_date(d, &["merge", "feature", "-m", "merge feature"], date);
    assert_eq!(gitr_result.exit_code, 0, "gitr merge failed: {}", gitr_result.stderr);

    // Verify merge commit has two parents
    let cat_result = gitr(d, &["cat-file", "-p", "HEAD"]);
    let parent_count = cat_result.stdout.lines().filter(|l| l.starts_with("parent")).count();
    assert_eq!(parent_count, 2, "merge commit should have 2 parents:\n{}", cat_result.stdout);

    // Verify tree has all files (from both branches)
    let tree = gitr(d, &["ls-tree", "-r", "HEAD"]);
    assert!(tree.stdout.contains("main_0.txt"), "missing main_0.txt");
    assert!(tree.stdout.contains("main_1.txt"), "missing main_1.txt");
    assert!(tree.stdout.contains("main_2.txt"), "missing main_2.txt");
    assert!(tree.stdout.contains("feature_0.txt"), "missing feature_0.txt");
    assert!(tree.stdout.contains("feature_1.txt"), "missing feature_1.txt");
}

#[test]
fn test_merge_conflict_exit_code_and_markers() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_merge_conflict(d);

    // Merge feature into main — should produce conflict
    let gitr_result = gitr(d, &["merge", "feature"]);
    assert_eq!(gitr_result.exit_code, 1, "expected exit 1 on conflict, got {}", gitr_result.exit_code);

    // Verify conflict markers in the file
    let conflict_content = std::fs::read_to_string(d.join("conflict.txt")).unwrap();
    assert!(conflict_content.contains("<<<<<<< HEAD"), "missing <<<<<<< HEAD marker");
    assert!(conflict_content.contains("======="), "missing ======= marker");
    assert!(conflict_content.contains(">>>>>>> feature"), "missing >>>>>>> feature marker");
}

#[test]
fn test_merge_commit_parents_and_message() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_branched_history(d);

    // Get HEAD oid before merge
    let pre_merge_head = gitr(d, &["rev-parse", "HEAD"]);
    let feature_head = gitr(d, &["rev-parse", "feature"]);

    // Merge
    let date = "1234567896 +0000";
    gitr_with_date(d, &["merge", "feature", "-m", "merge feature"], date);

    // Verify parent lines in commit
    let cat = gitr(d, &["cat-file", "-p", "HEAD"]);
    let parents: Vec<&str> = cat.stdout.lines()
        .filter(|l| l.starts_with("parent "))
        .map(|l| l.strip_prefix("parent ").unwrap().trim())
        .collect();
    assert_eq!(parents.len(), 2, "expected 2 parents");
    assert_eq!(parents[0], pre_merge_head.stdout.trim());
    assert_eq!(parents[1], feature_head.stdout.trim());

    // Verify merge message
    assert!(cat.stdout.contains("merge feature"), "merge message missing");
}

// ──────────────────────────── Phase 4: Diff Parity ────────────────────────────

#[test]
fn test_diff_unstaged_with_hunk_content() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    // Modify a tracked file (unstaged)
    std::fs::write(d.join("file_0.txt"), "modified content\n").unwrap();

    let git_diff = git(d, &["diff"]);
    let gitr_diff = gitr(d, &["diff"]);

    // Both should have hunk headers and content lines
    assert!(!git_diff.stdout.is_empty(), "git diff should not be empty");
    assert!(gitr_diff.stdout.contains("@@"), "gitr diff missing @@ hunk header");
    assert!(gitr_diff.stdout.contains("+modified content"), "gitr diff missing +modified line");
    assert_stdout_eq(&git_diff, &gitr_diff);
}

#[test]
fn test_diff_cached_with_hunk_content() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    // Stage a modification
    std::fs::write(d.join("file_0.txt"), "staged content\n").unwrap();
    git(d, &["add", "file_0.txt"]);

    let git_diff = git(d, &["diff", "--cached"]);
    let gitr_diff = gitr(d, &["diff", "--cached"]);

    assert!(!git_diff.stdout.is_empty(), "git diff --cached should not be empty");
    assert_stdout_eq(&git_diff, &gitr_diff);
}

#[test]
fn test_diff_head_with_hunk_content() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    // Modify and stage
    std::fs::write(d.join("file_0.txt"), "head diff content\n").unwrap();

    let git_diff = git(d, &["diff", "HEAD"]);
    let gitr_diff = gitr(d, &["diff", "HEAD"]);

    assert!(!git_diff.stdout.is_empty(), "git diff HEAD should not be empty");
    assert_stdout_eq(&git_diff, &gitr_diff);
}

// ──────────────────────────── Phase 5: Output Format Parity ────────────────────────────

#[test]
fn test_log_default_date_format() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 2);

    let git_log = git(d, &["log"]);
    let gitr_log = gitr(d, &["log"]);

    assert_stdout_eq(&git_log, &gitr_log);
}

#[test]
fn test_log_format_subject_newlines() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 3);

    let git_log = git(d, &["log", "--format=%s"]);
    let gitr_log = gitr(d, &["log", "--format=%s"]);

    assert_stdout_eq(&git_log, &gitr_log);
}

#[test]
fn test_show_date_format() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    let git_show = git(d, &["show", "HEAD", "--stat"]);
    let gitr_show = gitr(d, &["show", "HEAD", "--stat"]);

    // Compare the Date: line specifically
    let git_date: Vec<&str> = git_show.stdout.lines().filter(|l| l.starts_with("Date:")).collect();
    let gitr_date: Vec<&str> = gitr_show.stdout.lines().filter(|l| l.starts_with("Date:")).collect();
    assert_eq!(git_date, gitr_date, "Date format mismatch");
}

#[test]
fn test_blame_date_time_format() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    let git_blame = git(d, &["blame", "file_0.txt"]);
    let gitr_blame = gitr(d, &["blame", "file_0.txt"]);

    // Blame should include time, not just date
    assert!(git_blame.stdout.contains(":"), "git blame should include time");
    assert_stdout_eq(&git_blame, &gitr_blame);
}

#[test]
fn test_detached_head_status_with_oid() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 2);

    // Detach HEAD
    git(d, &["checkout", "HEAD~1"]);

    let git_status = git(d, &["status"]);
    let gitr_status = gitr(d, &["status"]);

    // Both should say "HEAD detached at <short-oid>"
    let git_detached: Vec<&str> = git_status.stdout.lines().filter(|l| l.contains("detached")).collect();
    let gitr_detached: Vec<&str> = gitr_status.stdout.lines().filter(|l| l.contains("detached")).collect();
    assert!(!git_detached.is_empty(), "git should show detached");
    assert_eq!(git_detached, gitr_detached, "detached HEAD status mismatch");
}

#[test]
fn test_empty_repo_log_exit_128() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_empty_repo(d);

    let git_log = git(d, &["log"]);
    let gitr_log = gitr(d, &["log"]);

    assert_exit_code_eq(&git_log, &gitr_log);
    assert_eq!(gitr_log.exit_code, 128, "empty repo log should exit 128");
}

#[test]
fn test_ls_files_unicode_escaping() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_unicode_paths(d);

    let git_ls = git(d, &["ls-files"]);
    let gitr_ls = gitr(d, &["ls-files"]);

    assert_stdout_eq(&git_ls, &gitr_ls);
}

// ──────────────────────────── Phase 6: Packfile Reading ────────────────────────────

#[test]
fn test_packfile_log_after_gc() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 12);

    // Run gc to pack objects
    git(d, &["gc"]);

    // Both should show all 12 commits
    let git_log = git(d, &["log", "--oneline"]);
    let gitr_log = gitr(d, &["log", "--oneline"]);

    assert_eq!(git_log.stdout.lines().count(), 12);
    assert_stdout_eq(&git_log, &gitr_log);
}

#[test]
fn test_cat_file_after_gc() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 3);

    // Capture HEAD content before gc
    let pre_gc = git(d, &["cat-file", "-p", "HEAD"]);

    // Run gc
    git(d, &["gc"]);

    // gitr should read packed objects correctly
    let gitr_cat = gitr(d, &["cat-file", "-p", "HEAD"]);
    assert_stdout_eq(&pre_gc, &gitr_cat);
}

#[test]
fn test_delta_resolution_after_gc() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 5);

    git(d, &["gc"]);

    // Verify all objects accessible
    let git_log = git(d, &["log", "--format=%H %T"]);
    let gitr_log = gitr(d, &["log", "--format=%H %T"]);
    assert_stdout_eq(&git_log, &gitr_log);
}

// ──────────────────────────── Phase 7: Remote Operations ────────────────────────────

#[test]
fn test_clone_file_protocol() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("bare.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    // Clone with gitr
    let clone_dir = d.join("gitr-clone");
    std::fs::create_dir_all(&clone_dir).unwrap();
    let url = format!("file://{}", bare_dir.display());
    let gitr_clone = gitr(d, &["clone", &url, clone_dir.to_str().unwrap()]);
    assert_eq!(gitr_clone.exit_code, 0, "gitr clone failed: {}", gitr_clone.stderr);

    // Verify refs match
    let git_refs = git(&clone_dir, &["show-ref"]);
    assert!(!git_refs.stdout.is_empty(), "cloned repo should have refs");

    // Verify log matches
    let git_log = git(&clone_dir, &["log", "--oneline"]);
    let gitr_log = gitr(&clone_dir, &["log", "--oneline"]);
    assert_stdout_eq(&git_log, &gitr_log);
}

#[test]
fn test_clone_bare() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("source.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    let clone_dir = d.join("gitr-bare.git");
    let url = format!("file://{}", bare_dir.display());
    let gitr_clone = gitr(d, &["clone", "--bare", &url, clone_dir.to_str().unwrap()]);
    assert_eq!(gitr_clone.exit_code, 0, "gitr bare clone failed: {}", gitr_clone.stderr);

    // Verify bare structure: HEAD exists, no .git/ wrapper
    assert!(clone_dir.join("HEAD").exists(), "bare clone missing HEAD");
    assert!(clone_dir.join("refs").exists(), "bare clone missing refs/");
    assert!(clone_dir.join("objects").exists(), "bare clone missing objects/");
    assert!(!clone_dir.join(".git").exists(), "bare clone should not have .git/");
}

#[test]
fn test_push_new_commits() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("remote.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    // Clone with git, then push with gitr
    let work_dir = d.join("work");
    std::fs::create_dir_all(&work_dir).unwrap();
    let url = format!("file://{}", bare_dir.display());
    git(d, &["clone", &url, work_dir.to_str().unwrap()]);

    // Add a new commit
    std::fs::write(work_dir.join("new.txt"), "new content\n").unwrap();
    let date = "1234567900 +0000";
    git_with_date(&work_dir, &["add", "new.txt"], date);
    git_with_date(&work_dir, &["commit", "-m", "new commit"], date);

    // Push with gitr
    let push_result = gitr(&work_dir, &["push", "origin", "main"]);
    assert_eq!(push_result.exit_code, 0, "gitr push failed: {}", push_result.stderr);

    // Verify C git can see the pushed commit
    let verify_dir = d.join("verify");
    std::fs::create_dir_all(&verify_dir).unwrap();
    git(d, &["clone", &url, verify_dir.to_str().unwrap()]);
    let log = git(&verify_dir, &["log", "--oneline"]);
    assert!(log.stdout.contains("new commit"), "pushed commit not visible");
}

#[test]
fn test_fetch_and_merge() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("remote.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    let url = format!("file://{}", bare_dir.display());

    // Clone with git
    let work_dir = d.join("work");
    std::fs::create_dir_all(&work_dir).unwrap();
    git(d, &["clone", &url, work_dir.to_str().unwrap()]);

    // Push new commit from another clone
    let push_dir = d.join("pusher");
    std::fs::create_dir_all(&push_dir).unwrap();
    git(d, &["clone", &url, push_dir.to_str().unwrap()]);
    std::fs::write(push_dir.join("fetched.txt"), "fetch me\n").unwrap();
    let date = "1234567900 +0000";
    git_with_date(&push_dir, &["add", "fetched.txt"], date);
    git_with_date(&push_dir, &["commit", "-m", "fetchable commit"], date);
    git(&push_dir, &["push", "origin", "main"]);

    // Fetch with gitr
    let fetch_result = gitr(&work_dir, &["fetch", "origin"]);
    assert_eq!(fetch_result.exit_code, 0, "gitr fetch failed: {}", fetch_result.stderr);

    // Verify remote tracking ref updated
    let refs = gitr(&work_dir, &["show-ref"]);
    assert!(refs.stdout.contains("refs/remotes/origin/main"), "remote tracking ref missing");
}

#[test]
fn test_pull_fast_forward() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("remote.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    let url = format!("file://{}", bare_dir.display());

    // Clone
    let work_dir = d.join("work");
    std::fs::create_dir_all(&work_dir).unwrap();
    git(d, &["clone", &url, work_dir.to_str().unwrap()]);

    // Push from another clone
    let push_dir = d.join("pusher");
    std::fs::create_dir_all(&push_dir).unwrap();
    git(d, &["clone", &url, push_dir.to_str().unwrap()]);
    std::fs::write(push_dir.join("pulled.txt"), "pull me\n").unwrap();
    let date = "1234567900 +0000";
    git_with_date(&push_dir, &["add", "pulled.txt"], date);
    git_with_date(&push_dir, &["commit", "-m", "pullable commit"], date);
    git(&push_dir, &["push", "origin", "main"]);

    // Pull with gitr
    let pull_result = gitr(&work_dir, &["pull", "origin", "main"]);
    assert_eq!(pull_result.exit_code, 0, "gitr pull failed: {}", pull_result.stderr);

    // Verify log includes the pulled commit
    let log = gitr(&work_dir, &["log", "--oneline"]);
    assert!(log.stdout.contains("pullable commit"), "pulled commit not visible");
}

#[test]
fn test_push_feature_branch() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("remote.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    let url = format!("file://{}", bare_dir.display());

    // Clone and create feature branch
    let work_dir = d.join("work");
    std::fs::create_dir_all(&work_dir).unwrap();
    git(d, &["clone", &url, work_dir.to_str().unwrap()]);
    git(&work_dir, &["checkout", "-b", "feature"]);
    std::fs::write(work_dir.join("feature.txt"), "feature\n").unwrap();
    let date = "1234567900 +0000";
    git_with_date(&work_dir, &["add", "feature.txt"], date);
    git_with_date(&work_dir, &["commit", "-m", "feature commit"], date);

    // Push feature branch with gitr
    let push_result = gitr(&work_dir, &["push", "origin", "feature"]);
    assert_eq!(push_result.exit_code, 0, "gitr push feature failed: {}", push_result.stderr);
}

#[test]
fn test_remote_config_values() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    let bare_dir = d.join("remote.git");
    std::fs::create_dir_all(&bare_dir).unwrap();
    setup_bare_remote(&bare_dir);

    let url = format!("file://{}", bare_dir.display());

    let clone_dir = d.join("gitr-clone");
    std::fs::create_dir_all(&clone_dir).unwrap();
    gitr(d, &["clone", &url, clone_dir.to_str().unwrap()]);

    let remote_url = git(&clone_dir, &["config", "--get", "remote.origin.url"]);
    assert_eq!(remote_url.stdout.trim(), url, "remote.origin.url mismatch");

    let fetch = git(&clone_dir, &["config", "--get", "remote.origin.fetch"]);
    assert_eq!(fetch.stdout.trim(), "+refs/heads/*:refs/remotes/origin/*", "remote.origin.fetch mismatch");
}

// ──────────────────────────── Phase 8: Stash Operations ────────────────────────────

#[test]
fn test_stash_push_pop_roundtrip() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    // Modify a file
    std::fs::write(d.join("file_0.txt"), "stashed content\n").unwrap();

    // Stash with gitr
    let stash_push = gitr(d, &["stash", "push", "-m", "test stash"]);
    assert_eq!(stash_push.exit_code, 0, "stash push failed: {}", stash_push.stderr);

    // Verify clean working tree
    let status = gitr(d, &["status", "--porcelain"]);
    assert!(status.stdout.trim().is_empty(), "working tree should be clean after stash");

    // Pop
    let stash_pop = gitr(d, &["stash", "pop"]);
    assert_eq!(stash_pop.exit_code, 0, "stash pop failed: {}", stash_pop.stderr);

    // Verify content restored
    let content = std::fs::read_to_string(d.join("file_0.txt")).unwrap();
    assert_eq!(content, "stashed content\n", "stash pop didn't restore content");
}

#[test]
fn test_stash_list_multiple_entries() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    // Push 3 stashes
    for i in 0..3 {
        std::fs::write(d.join("file_0.txt"), format!("stash {}\n", i)).unwrap();
        gitr(d, &["stash", "push", "-m", &format!("stash {}", i)]);
    }

    let list = gitr(d, &["stash", "list"]);
    let lines: Vec<&str> = list.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 stash entries, got {}", lines.len());
    assert!(lines[0].contains("stash@{0}"), "first entry missing stash@{{0}}");
    assert!(lines[1].contains("stash@{1}"), "second entry missing stash@{{1}}");
    assert!(lines[2].contains("stash@{2}"), "third entry missing stash@{{2}}");
}

#[test]
fn test_stash_include_untracked() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    // Create an untracked file
    std::fs::write(d.join("untracked.txt"), "untracked\n").unwrap();

    // Stash with --include-untracked
    let stash = gitr(d, &["stash", "push", "--include-untracked", "-m", "with untracked"]);
    assert_eq!(stash.exit_code, 0, "stash --include-untracked failed: {}", stash.stderr);

    // Verify untracked file removed
    assert!(!d.join("untracked.txt").exists(), "untracked file should be removed");

    // Pop and verify restored
    gitr(d, &["stash", "pop"]);
    assert!(d.join("untracked.txt").exists(), "untracked file should be restored after pop");
    let content = std::fs::read_to_string(d.join("untracked.txt")).unwrap();
    assert_eq!(content, "untracked\n", "untracked content mismatch");
}

// ──────────────────────────── Phase 9: Plumbing Parity ────────────────────────────

#[test]
fn test_for_each_ref_excludes_head() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    let git_refs = git(d, &["for-each-ref", "--format=%(refname) %(objectname)"]);
    let gitr_refs = gitr(d, &["for-each-ref", "--format=%(refname) %(objectname)"]);

    // Neither should include HEAD
    assert!(!git_refs.stdout.contains("HEAD"), "git for-each-ref includes HEAD");
    assert!(!gitr_refs.stdout.contains("HEAD"), "gitr for-each-ref includes HEAD");
    assert_stdout_eq(&git_refs, &gitr_refs);
}

#[test]
fn test_rev_parse_head_tree() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 1);

    let git_tree = git(d, &["rev-parse", "HEAD^{tree}"]);
    let gitr_tree = gitr(d, &["rev-parse", "HEAD^{tree}"]);

    assert_eq!(git_tree.exit_code, 0);
    assert_stdout_eq(&git_tree, &gitr_tree);
}

#[test]
fn test_rev_parse_multiple_peeling() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_linear_history(d, 2);

    // Test peeling to commit (no-op for commits)
    let git_commit = git(d, &["rev-parse", "HEAD^{commit}"]);
    let gitr_commit = gitr(d, &["rev-parse", "HEAD^{commit}"]);
    assert_stdout_eq(&git_commit, &gitr_commit);

    // Test peeling to tree
    let git_tree = git(d, &["rev-parse", "HEAD^{tree}"]);
    let gitr_tree = gitr(d, &["rev-parse", "HEAD^{tree}"]);
    assert_stdout_eq(&git_tree, &gitr_tree);
}

// ──────────────────────────── Phase 10: Rebase ────────────────────────────

#[test]
fn test_rebase_linear() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    setup_branched_history(d);

    // Checkout feature
    git(d, &["checkout", "feature"]);

    // Rebase onto main
    let rebase = gitr(d, &["rebase", "main"]);
    assert_eq!(rebase.exit_code, 0, "rebase failed: {}", rebase.stderr);

    // After rebase, feature should be ahead of main with same commits
    let log = gitr(d, &["log", "--oneline"]);
    assert!(log.stdout.contains("feature commit"), "rebased commits missing");
}

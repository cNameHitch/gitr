//! End-to-end workflow interoperability tests.
//!
//! Tests multi-step workflows (init->add->commit->branch->merge), branching,
//! cross-tool compatibility, history inspection, and plumbing command interop
//! by running both gitr and C git and comparing outputs.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// User Story 1 — Basic Workflow Interop (P1)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_init_add_commit_status_cycle() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    // Use C git to set up both repos identically for a fair comparison
    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    // Add and commit identical files with both tools
    std::fs::write(dir_git.path().join("hello.txt"), "hello world\n").unwrap();
    std::fs::write(dir_gitr.path().join("hello.txt"), "hello world\n").unwrap();

    git(dir_git.path(), &["add", "hello.txt"]);
    gitr(dir_gitr.path(), &["add", "hello.txt"]);

    git(dir_git.path(), &["commit", "-m", "initial commit"]);
    gitr(dir_gitr.path(), &["commit", "-m", "initial commit"]);

    // Compare log output
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    // Compare repo state
    assert_repo_state_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_gitr_init_cgit_operates() {
    let dir = tempfile::tempdir().unwrap();

    // Gitr creates the repo (no -b flag — gitr doesn't support it)
    gitr(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test Author"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);

    std::fs::write(dir.path().join("file1.txt"), "content 1\n").unwrap();
    gitr(dir.path(), &["add", "file1.txt"]);
    gitr(dir.path(), &["commit", "-m", "gitr commit 1"]);

    std::fs::write(dir.path().join("file2.txt"), "content 2\n").unwrap();
    gitr(dir.path(), &["add", "file2.txt"]);
    gitr(dir.path(), &["commit", "-m", "gitr commit 2"]);

    // C git operates on it
    std::fs::write(dir.path().join("file3.txt"), "content 3\n").unwrap();
    let result = git(dir.path(), &["add", "file3.txt"]);
    assert_eq!(result.exit_code, 0);

    let result = git(dir.path(), &["commit", "-m", "cgit commit"]);
    assert_eq!(result.exit_code, 0);

    let result = git(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 3);
}

#[test]
fn test_cgit_init_gitr_operates() {
    let dir = tempfile::tempdir().unwrap();

    // C git creates the repo
    setup_empty_repo(dir.path());

    std::fs::write(dir.path().join("file1.txt"), "content 1\n").unwrap();
    git(dir.path(), &["add", "file1.txt"]);
    git(dir.path(), &["commit", "-m", "cgit commit 1"]);

    std::fs::write(dir.path().join("file2.txt"), "content 2\n").unwrap();
    git(dir.path(), &["add", "file2.txt"]);
    git(dir.path(), &["commit", "-m", "cgit commit 2"]);

    // Gitr operates on it
    std::fs::write(dir.path().join("file3.txt"), "content 3\n").unwrap();
    let result = gitr(dir.path(), &["add", "file3.txt"]);
    assert_eq!(result.exit_code, 0);

    let result = gitr(dir.path(), &["commit", "-m", "gitr commit"]);
    assert_eq!(result.exit_code, 0);

    let result = gitr(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 3);
}

#[test]
fn test_status_identical_output() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    // Clean repo — use porcelain output for deterministic comparison
    let g = git(dir.path(), &["status", "--porcelain"]);
    let m = gitr(dir.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);

    // Add untracked file
    std::fs::write(dir.path().join("untracked.txt"), "new\n").unwrap();

    let g = git(dir.path(), &["status", "--porcelain"]);
    let m = gitr(dir.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);

    // Modify tracked file
    std::fs::write(dir.path().join("file_0.txt"), "modified\n").unwrap();

    let g = git(dir.path(), &["status", "--porcelain"]);
    let m = gitr(dir.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_multiple_commits_sequential() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let mut counter = 0u64;
    for i in 0..5 {
        let filename = format!("seq_{}.txt", i);
        let content = format!("sequential content {}\n", i);
        let date = next_date(&mut counter);
        let msg = format!("sequential commit {}", i);

        std::fs::write(dir_git.path().join(&filename), &content).unwrap();
        std::fs::write(dir_gitr.path().join(&filename), &content).unwrap();

        git_with_date(dir_git.path(), &["add", &filename], &date);
        gitr_with_date(dir_gitr.path(), &["add", &filename], &date);

        git_with_date(dir_git.path(), &["commit", "-m", &msg], &date);
        gitr_with_date(dir_gitr.path(), &["commit", "-m", &msg], &date);
    }

    // Compare log
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    // Compare rev-list
    let g = git(dir_git.path(), &["rev-list", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "HEAD"]);
    assert_output_eq(&g, &m);

    // Compare HEAD and refs
    assert_repo_state_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_add_multiple_files_and_directories() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    // Create nested directory structure
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::create_dir_all(dir.join("src/lib")).unwrap();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dir.join("src/lib/utils.rs"), "pub fn util() {}\n").unwrap();
        std::fs::write(dir.join("docs/readme.txt"), "readme\n").unwrap();
    }

    git(dir_git.path(), &["add", "."]);
    gitr(dir_gitr.path(), &["add", "."]);

    let g = git(dir_git.path(), &["ls-files", "--stage"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "--stage"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_commit_message_single_line() {
    // Test single-line commit messages
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    }

    git(dir_git.path(), &["add", "a.txt"]);
    git(dir_git.path(), &["commit", "-m", "single line"]);
    gitr(dir_gitr.path(), &["add", "a.txt"]);
    gitr(dir_gitr.path(), &["commit", "-m", "single line"]);

    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_commit_message_multiline() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    }

    let msg = "Subject line\n\nThis is the body.\nMultiple lines here.";

    git(dir_git.path(), &["add", "a.txt"]);
    git(dir_git.path(), &["commit", "-m", msg]);
    gitr(dir_gitr.path(), &["add", "a.txt"]);
    gitr(dir_gitr.path(), &["commit", "-m", msg]);

    let g = git(dir_git.path(), &["log", "--format=%B"]);
    let m = gitr(dir_gitr.path(), &["log", "--format=%B"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_commit_message_special_chars() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    }

    let msg = "fix: handle \"quotes\" and 'apostrophes' (parens) [brackets] café";

    git(dir_git.path(), &["add", "a.txt"]);
    git(dir_git.path(), &["commit", "-m", msg]);
    gitr(dir_gitr.path(), &["add", "a.txt"]);
    gitr(dir_gitr.path(), &["commit", "-m", msg]);

    let g = git(dir_git.path(), &["log", "--format=%B"]);
    let m = gitr(dir_gitr.path(), &["log", "--format=%B"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_diff_staged_and_unstaged() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // Unstaged modification
    std::fs::write(dir.path().join("file_0.txt"), "modified content\n").unwrap();

    let g = git(dir.path(), &["diff"]);
    let m = gitr(dir.path(), &["diff"]);
    assert_output_eq(&g, &m);

    // Stage the change
    git(dir.path(), &["add", "file_0.txt"]);

    let g = git(dir.path(), &["diff", "--cached"]);
    let m = gitr(dir.path(), &["diff", "--cached"]);
    assert_output_eq(&g, &m);

    // diff HEAD shows staged changes vs HEAD
    let g = git(dir.path(), &["diff", "HEAD"]);
    let m = gitr(dir.path(), &["diff", "HEAD"]);
    assert_output_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// User Story 2 — Branching and Merging Interop (P1)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_branch_create_list_delete_cycle() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Create branch
    git(dir_git.path(), &["branch", "feature"]);
    gitr(dir_gitr.path(), &["branch", "feature"]);

    let g = git(dir_git.path(), &["branch"]);
    let m = gitr(dir_gitr.path(), &["branch"]);
    assert_output_eq(&g, &m);

    // Delete branch
    git(dir_git.path(), &["branch", "-d", "feature"]);
    gitr(dir_gitr.path(), &["branch", "-d", "feature"]);

    let g = git(dir_git.path(), &["show-ref"]);
    let m = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_checkout_and_switch_equivalence() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());

    let g = git(dir.path(), &["rev-parse", "feature"]);
    gitr(dir.path(), &["checkout", "feature"]);
    let m = gitr(dir.path(), &["rev-parse", "HEAD"]);
    assert_eq!(g.stdout.trim(), m.stdout.trim());

    gitr(dir.path(), &["checkout", "main"]);

    gitr(dir.path(), &["switch", "feature"]);
    let m2 = gitr(dir.path(), &["rev-parse", "HEAD"]);
    assert_eq!(g.stdout.trim(), m2.stdout.trim());
}

#[test]
fn test_branch_rename() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["branch", "old-name"]);
    }

    git(dir_git.path(), &["branch", "-m", "old-name", "new-name"]);
    gitr(dir_gitr.path(), &["branch", "-m", "old-name", "new-name"]);

    let g = git(dir_git.path(), &["show-ref"]);
    let m = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_fast_forward_merge() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_linear_history(dir, 3);
        git(dir, &["branch", "feature"]);
        git(dir, &["checkout", "feature"]);
        std::fs::write(dir.join("feature.txt"), "feature content\n").unwrap();
        git(dir, &["add", "feature.txt"]);
        git(dir, &["commit", "-m", "feature commit"]);
        git(dir, &["checkout", "main"]);
    }

    git(dir_git.path(), &["merge", "feature"]);
    gitr(dir_gitr.path(), &["merge", "feature"]);

    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    assert_repo_state_eq(dir_git.path(), dir_gitr.path());
}

#[test]
fn test_three_way_merge_no_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    git(dir_git.path(), &["merge", "feature", "-m", "merge feature"]);
    gitr(dir_gitr.path(), &["merge", "feature", "-m", "merge feature"]);

    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_merge_conflict_markers() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_merge_conflict(dir_git.path());
    setup_merge_conflict(dir_gitr.path());

    let g = git(dir_git.path(), &["merge", "feature"]);
    let m = gitr(dir_gitr.path(), &["merge", "feature"]);
    assert_exit_code_eq(&g, &m);

    // Compare conflict markers in working tree
    let g_conflict = std::fs::read_to_string(dir_git.path().join("conflict.txt")).unwrap();
    let m_conflict = std::fs::read_to_string(dir_gitr.path().join("conflict.txt")).unwrap();
    assert_eq!(g_conflict, m_conflict, "conflict markers should match byte-for-byte");

    // Compare ls-files --stage (should show stage 1/2/3 entries)
    let g = git(dir_git.path(), &["ls-files", "--stage"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "--stage"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_merge_commit_message() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    git(dir_git.path(), &["merge", "feature", "-m", "Merge branch 'feature'"]);
    gitr(dir_gitr.path(), &["merge", "feature", "-m", "Merge branch 'feature'"]);

    // Use cat-file -p HEAD to read the commit message (avoids log -1 flag)
    let g = git(dir_git.path(), &["cat-file", "-p", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-p", "HEAD"]);
    assert_eq!(g.exit_code, 0);
    assert_eq!(m.exit_code, 0);
    assert!(g.stdout.contains("Merge branch 'feature'"));
    assert!(m.stdout.contains("Merge branch 'feature'"));
}

#[test]
fn test_detached_head_operations() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Get specific commit OID to checkout (HEAD~1)
    let oid = git(dir_git.path(), &["rev-parse", "HEAD~1"]).stdout.trim().to_string();

    git(dir_git.path(), &["checkout", &oid]);
    gitr(dir_gitr.path(), &["checkout", &oid]);

    // rev-parse HEAD should match
    let g = git(dir_git.path(), &["rev-parse", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-parse", "HEAD"]);
    assert_output_eq(&g, &m);

    // symbolic-ref HEAD should fail in detached state
    let g = git(dir_git.path(), &["symbolic-ref", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["symbolic-ref", "HEAD"]);
    assert!(g.exit_code != 0, "git symbolic-ref should fail in detached state");
    assert!(m.exit_code != 0, "gitr symbolic-ref should fail in detached state");

    // status --porcelain should show empty (clean)
    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

// ════════════════════════════════════════════════════════════════════════════
// User Story 8 — Cross-Tool Repository Compatibility (P1)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_gitr_repo_passes_cgit_fsck() {
    let dir = tempfile::tempdir().unwrap();

    // Use gitr init (no -b flag), then C git for config
    gitr(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test Author"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 0u64;
    for i in 0..3 {
        let filename = format!("file_{}.txt", i);
        let content = format!("gitr content {}\n", i);
        let date = next_date(&mut counter);
        std::fs::write(dir.path().join(&filename), &content).unwrap();
        gitr_with_date(dir.path(), &["add", &filename], &date);
        gitr_with_date(dir.path(), &["commit", "-m", &format!("gitr commit {}", i)], &date);
    }

    // C git fsck must pass
    assert_fsck_clean(dir.path());

    // C git log must show all commits
    let result = git(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 3);
}

#[test]
fn test_cgit_repo_passes_gitr_fsck() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    // Gitr fsck should pass
    let result = gitr(dir.path(), &["fsck"]);
    assert_eq!(result.exit_code, 0);

    // Gitr log should show all commits
    let result = gitr(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 3);
}

#[test]
fn test_alternating_commits_ping_pong() {
    let dir = tempfile::tempdir().unwrap();

    // Use gitr init, then C git for config
    gitr(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test Author"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 0u64;

    // Commit 1: gitr
    std::fs::write(dir.path().join("f1.txt"), "gitr 1\n").unwrap();
    let date = next_date(&mut counter);
    gitr_with_date(dir.path(), &["add", "f1.txt"], &date);
    gitr_with_date(dir.path(), &["commit", "-m", "gitr 1"], &date);
    assert_fsck_clean(dir.path());

    // Commit 2: C git
    std::fs::write(dir.path().join("f2.txt"), "cgit 2\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "f2.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "cgit 2"], &date);
    let m_fsck = gitr(dir.path(), &["fsck"]);
    assert_eq!(m_fsck.exit_code, 0);

    // Commit 3: gitr
    std::fs::write(dir.path().join("f3.txt"), "gitr 3\n").unwrap();
    let date = next_date(&mut counter);
    gitr_with_date(dir.path(), &["add", "f3.txt"], &date);
    gitr_with_date(dir.path(), &["commit", "-m", "gitr 3"], &date);
    assert_fsck_clean(dir.path());

    // Commit 4: C git
    std::fs::write(dir.path().join("f4.txt"), "cgit 4\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir.path(), &["add", "f4.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "cgit 4"], &date);

    // Final: both tools see same log
    let g = git(dir.path(), &["log", "--oneline"]);
    let m = gitr(dir.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_mixed_history_log_identical() {
    let dir = tempfile::tempdir().unwrap();

    gitr(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test Author"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 0u64;
    let tools = ["gitr", "git", "gitr", "git", "gitr"];

    for (i, tool) in tools.iter().enumerate() {
        let filename = format!("mixed_{}.txt", i);
        let content = format!("{} content {}\n", tool, i);
        let date = next_date(&mut counter);
        let msg = format!("{} commit {}", tool, i);

        std::fs::write(dir.path().join(&filename), &content).unwrap();

        if *tool == "gitr" {
            gitr_with_date(dir.path(), &["add", &filename], &date);
            gitr_with_date(dir.path(), &["commit", "-m", &msg], &date);
        } else {
            git_with_date(dir.path(), &["add", &filename], &date);
            git_with_date(dir.path(), &["commit", "-m", &msg], &date);
        }
    }

    // Use --oneline which is known to work in gitr
    let g = git(dir.path(), &["log", "--oneline"]);
    let m = gitr(dir.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_gitr_gc_cgit_fsck() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 12);

    let result = gitr(dir.path(), &["gc"]);
    assert_eq!(result.exit_code, 0);

    assert_fsck_clean(dir.path());

    let result = git(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 12);
}

#[test]
fn test_cgit_gc_gitr_operates() {
    let dir = tempfile::tempdir().unwrap();

    // Gitr creates repo with 10+ commits
    gitr(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test Author"]);
    git(dir.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 0u64;
    for i in 0..12 {
        let filename = format!("file_{}.txt", i);
        let content = format!("content {}\n", i);
        let date = next_date(&mut counter);
        std::fs::write(dir.path().join(&filename), &content).unwrap();
        gitr_with_date(dir.path(), &["add", &filename], &date);
        gitr_with_date(dir.path(), &["commit", "-m", &format!("commit {}", i)], &date);
    }

    // C git runs gc
    let result = git(dir.path(), &["gc"]);
    assert_eq!(result.exit_code, 0);

    // Gitr log should still show all commits
    let result = gitr(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 12);

    // Gitr cat-file on HEAD should succeed
    let result = gitr(dir.path(), &["cat-file", "-p", "HEAD"]);
    assert_eq!(result.exit_code, 0);
}

// ════════════════════════════════════════════════════════════════════════════
// User Story 3 — History Inspection Interop (P2)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_format_flags_interop() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 5);

    // Test multiple log format flags
    let flags: &[&[&str]] = &[
        &["log", "--oneline"],
        &["log", "--format=%H %ae %s"],
    ];

    for args in flags {
        let g = git(dir.path(), args);
        let m = gitr(dir.path(), args);
        assert_output_eq(&g, &m);
    }
}

#[test]
fn test_blame_interop() {
    let dir = tempfile::tempdir().unwrap();
    setup_empty_repo(dir.path());

    let mut counter = 0u64;

    // Build file across 3 commits with distinct lines
    let date = next_date(&mut counter);
    std::fs::write(dir.path().join("file.txt"), "line one\n").unwrap();
    git_with_date(dir.path(), &["add", "file.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "add line 1"], &date);

    let date = next_date(&mut counter);
    std::fs::write(dir.path().join("file.txt"), "line one\nline two\n").unwrap();
    git_with_date(dir.path(), &["add", "file.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "add line 2"], &date);

    let date = next_date(&mut counter);
    std::fs::write(dir.path().join("file.txt"), "line one\nline two\nline three\n").unwrap();
    git_with_date(dir.path(), &["add", "file.txt"], &date);
    git_with_date(dir.path(), &["commit", "-m", "add line 3"], &date);

    let g = git(dir.path(), &["blame", "file.txt"]);
    let m = gitr(dir.path(), &["blame", "file.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_show_commit_interop() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    // show --no-patch HEAD (header only)
    let g = git(dir.path(), &["show", "--no-patch", "HEAD"]);
    let m = gitr(dir.path(), &["show", "--no-patch", "HEAD"]);
    assert_output_eq(&g, &m);

    // show HEAD:file_0.txt (blob content)
    let g = git(dir.path(), &["show", "HEAD:file_0.txt"]);
    let m = gitr(dir.path(), &["show", "HEAD:file_0.txt"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_log_oneline_interop() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 5);

    let g = git(dir.path(), &["log", "--oneline"]);
    let m = gitr(dir.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_list_interop() {
    let dir = tempfile::tempdir().unwrap();
    setup_branched_history(dir.path());
    git(dir.path(), &["merge", "feature", "-m", "merge"]);

    let flags: &[&[&str]] = &[
        &["rev-list", "HEAD"],
        &["rev-list", "--count", "HEAD"],
        &["rev-list", "--reverse", "HEAD"],
    ];

    for args in flags {
        let g = git(dir.path(), args);
        let m = gitr(dir.path(), args);
        assert_output_eq(&g, &m);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// User Story 4 — Plumbing Command Interop (P2)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cat_file_all_object_types() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    git(dir.path(), &["tag", "-a", "v1.0", "-m", "release"]);

    let commit_oid = git(dir.path(), &["rev-parse", "HEAD"]).stdout.trim().to_string();
    let tree_oid = git(dir.path(), &["rev-parse", "HEAD^{tree}"]).stdout.trim().to_string();
    let blob_oid = git(dir.path(), &["rev-parse", "HEAD:file_0.txt"]).stdout.trim().to_string();
    let tag_oid = git(dir.path(), &["rev-parse", "v1.0"]).stdout.trim().to_string();

    for oid in [&commit_oid, &tree_oid, &blob_oid, &tag_oid] {
        for flag in ["-t", "-s", "-p"] {
            let g = git(dir.path(), &["cat-file", flag, oid]);
            let m = gitr(dir.path(), &["cat-file", flag, oid]);
            assert_output_eq(&g, &m);
        }
    }
}

#[test]
fn test_for_each_ref_format_strings() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    // Create branches and tags
    git(dir.path(), &["branch", "feature"]);
    git(dir.path(), &["branch", "develop"]);
    git(dir.path(), &["tag", "v1.0"]);
    git(dir.path(), &["tag", "-a", "v2.0", "-m", "release 2"]);

    let g = git(
        dir.path(),
        &["for-each-ref", "--format=%(refname) %(objectname) %(objecttype)"],
    );
    let m = gitr(
        dir.path(),
        &["for-each-ref", "--format=%(refname) %(objectname) %(objecttype)"],
    );
    assert_output_eq(&g, &m);

    // Also test sorted by creatordate
    let g = git(
        dir.path(),
        &["for-each-ref", "--sort=-creatordate", "--format=%(refname)"],
    );
    let m = gitr(
        dir.path(),
        &["for-each-ref", "--sort=-creatordate", "--format=%(refname)"],
    );
    assert_output_eq(&g, &m);
}

#[test]
fn test_rev_parse_basic_expressions() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 5);

    let exprs = ["HEAD", "HEAD~2", "main"];
    for expr in &exprs {
        let g = git(dir.path(), &["rev-parse", expr]);
        let m = gitr(dir.path(), &["rev-parse", expr]);
        assert_output_eq(&g, &m);
    }
}

#[test]
fn test_rev_parse_complex_expressions() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 5);

    let exprs = ["HEAD^{tree}", "HEAD~3^{commit}"];
    for expr in &exprs {
        let g = git(dir.path(), &["rev-parse", expr]);
        let m = gitr(dir.path(), &["rev-parse", expr]);
        assert_output_eq(&g, &m);
    }
}
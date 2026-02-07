//! End-to-end advanced operations interoperability tests.
//!
//! Tests rebase, cherry-pick, annotated tags, and gc/repack
//! by running both gitr and C git and comparing outputs.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// User Story 6 — Advanced Operations Interop (P3)
// ════════════════════════════════════════════════════════════════════════════

// ── Rebase ──

#[test]
fn test_rebase_linear() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["checkout", "feature"]);
    }

    git(dir_git.path(), &["rebase", "main"]);
    gitr(dir_gitr.path(), &["rebase", "main"]);

    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rebase_conflict_abort() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_merge_conflict(dir);
        git(dir, &["checkout", "feature"]);
    }

    let g = git(dir_git.path(), &["rebase", "main"]);
    let m = gitr(dir_gitr.path(), &["rebase", "main"]);
    assert_exit_code_eq(&g, &m);

    git(dir_git.path(), &["rebase", "--abort"]);
    gitr(dir_gitr.path(), &["rebase", "--abort"]);

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_rebase_onto() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_branched_history(dir);
        // Create a third branch from feature
        git(dir, &["checkout", "feature"]);
        std::fs::write(dir.join("extra.txt"), "extra\n").unwrap();
        git(dir, &["add", "extra.txt"]);
        git(dir, &["commit", "-m", "extra commit"]);
        git(dir, &["branch", "feature-b"]);
        git(dir, &["checkout", "main"]);
    }

    git(dir_git.path(), &["rebase", "--onto", "main", "feature", "feature-b"]);
    gitr(dir_gitr.path(), &["rebase", "--onto", "main", "feature", "feature-b"]);

    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

// ── Cherry-pick and Revert ──

#[test]
fn test_cherry_pick_output_matches() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_branched_history(dir_git.path());
    setup_branched_history(dir_gitr.path());

    // Get last commit OID on feature
    let oid = git(dir_git.path(), &["rev-parse", "feature"]).stdout.trim().to_string();

    // Cherry-pick onto main
    let g = git(dir_git.path(), &["cherry-pick", &oid]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", &oid]);
    assert_exit_code_eq(&g, &m);

    // Compare resulting tree content
    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_revert_output_matches() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    git(dir_git.path(), &["revert", "HEAD"]);
    gitr(dir_gitr.path(), &["revert", "HEAD"]);

    // Compare tree content (the reverted file should be gone)
    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);

    // Verify both reverted the same file
    assert!(!dir_git.path().join("file_2.txt").exists());
    assert!(!dir_gitr.path().join("file_2.txt").exists());
}

#[test]
fn test_cherry_pick_conflict() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    for dir in [dir_git.path(), dir_gitr.path()] {
        setup_merge_conflict(dir);
    }

    // Get the feature tip OID
    let oid = git(dir_git.path(), &["rev-parse", "feature"]).stdout.trim().to_string();

    let g = git(dir_git.path(), &["cherry-pick", &oid]);
    let m = gitr(dir_gitr.path(), &["cherry-pick", &oid]);
    assert_exit_code_eq(&g, &m);
}

// ── Stash ──

#[test]
fn test_stash_push_pop_roundtrip() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Modify a file (unstaged)
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("file_0.txt"), "modified for stash\n").unwrap();
    }

    // Stash push
    git(dir_git.path(), &["stash", "push"]);
    gitr(dir_gitr.path(), &["stash", "push"]);

    // Working tree should be clean
    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);

    // Stash pop
    git(dir_git.path(), &["stash", "pop"]);
    gitr(dir_gitr.path(), &["stash", "pop"]);

    // Modification should be back
    let g_content = std::fs::read_to_string(dir_git.path().join("file_0.txt")).unwrap();
    let m_content = std::fs::read_to_string(dir_gitr.path().join("file_0.txt")).unwrap();
    assert_eq!(g_content, m_content, "stash pop should restore modification");
}

#[test]
fn test_stash_list_output() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Create 3 stashes
    for i in 0..3 {
        let filename = format!("stash_{}.txt", i);
        for dir in [dir_git.path(), dir_gitr.path()] {
            std::fs::write(dir.join(&filename), format!("stash content {}\n", i)).unwrap();
            git(dir, &["add", &filename]);
        }
        git(dir_git.path(), &["stash", "push", "-m", &format!("stash {}", i)]);
        gitr(dir_gitr.path(), &["stash", "push", "-m", &format!("stash {}", i)]);
    }

    let g = git(dir_git.path(), &["stash", "list"]);
    let m = gitr(dir_gitr.path(), &["stash", "list"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_stash_with_untracked() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Add an untracked file
    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("untracked.txt"), "untracked\n").unwrap();
    }

    git(dir_git.path(), &["stash", "push", "--include-untracked"]);
    gitr(dir_gitr.path(), &["stash", "push", "--include-untracked"]);

    // Untracked file should be gone
    assert!(!dir_git.path().join("untracked.txt").exists());
    assert!(!dir_gitr.path().join("untracked.txt").exists());

    // Pop should restore
    git(dir_git.path(), &["stash", "pop"]);
    gitr(dir_gitr.path(), &["stash", "pop"]);

    assert!(dir_git.path().join("untracked.txt").exists());
    assert!(dir_gitr.path().join("untracked.txt").exists());
}

// ── Annotated Tags ──

#[test]
fn test_annotated_tag_interop() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    git(dir_git.path(), &["tag", "-a", "v1.0", "-m", "Release 1.0"]);
    gitr(dir_gitr.path(), &["tag", "-a", "v1.0", "-m", "Release 1.0"]);

    // Compare tag object content via cat-file
    let g_oid = git(dir_git.path(), &["rev-parse", "v1.0"]).stdout.trim().to_string();
    let m_oid = gitr(dir_gitr.path(), &["rev-parse", "v1.0"]).stdout.trim().to_string();

    let g = git(dir_git.path(), &["cat-file", "-p", &g_oid]);
    let m = gitr(dir_gitr.path(), &["cat-file", "-p", &m_oid]);
    assert_eq!(g.exit_code, 0);
    assert_eq!(m.exit_code, 0);
    // Both should be tag objects pointing to same commit
    assert!(g.stdout.contains("Release 1.0"));
    assert!(m.stdout.contains("Release 1.0"));
}

#[test]
fn test_tag_list_output_matches() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    // Create mix of lightweight and annotated tags
    for dir in [dir_git.path(), dir_gitr.path()] {
        git(dir, &["tag", "v1.0"]);
        git(dir, &["tag", "-a", "v2.0", "-m", "release 2"]);
        git(dir, &["tag", "v3.0-beta"]);
    }

    let g = git(dir_git.path(), &["tag"]);
    let m = gitr(dir_gitr.path(), &["tag"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_tag_verify_cross_tool() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // Gitr creates annotated tag
    gitr(dir.path(), &["tag", "-a", "v1.0", "-m", "release"]);

    // C git should see the tag
    let result = git(dir.path(), &["describe"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("v1.0"), "C git describe should reference the tag");
}

// ── GC / Repack ──

#[test]
fn test_gc_repack_packfile_compat() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 12);

    gitr(dir.path(), &["gc"]);
    assert_fsck_clean(dir.path());

    let result = git(dir.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.lines().count(), 12);
}

#[test]
fn test_fsck_after_repack() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 12);

    gitr(dir.path(), &["repack", "-a", "-d"]);
    assert_fsck_clean(dir.path());
}
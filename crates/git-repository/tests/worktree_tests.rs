//! Worktree support tests.
//!
//! Tests that linked worktrees are detected correctly and share the right
//! resources (objects, refs) while having independent HEAD and index.

use std::process::Command;

use git_repository::{Repository, RepositoryKind};

/// Create a repo with a commit, then add a linked worktree.
/// Returns (main_dir, main_worktree, worktree_dir, worktree_path).
fn setup_worktree() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    tempfile::TempDir,
    std::path::PathBuf,
) {
    let main_dir = tempfile::tempdir().unwrap();
    let main_path = std::fs::canonicalize(main_dir.path()).unwrap();

    let run = |args: &[&str], dir: &std::path::Path| -> bool {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };

    assert!(run(&["init"], &main_path));
    assert!(run(&["commit", "--allow-empty", "-m", "initial"], &main_path));

    // Create a linked worktree
    let wt_dir = tempfile::tempdir().unwrap();
    let wt_path = std::fs::canonicalize(wt_dir.path()).unwrap();
    let wt_target = wt_path.join("linked");

    assert!(run(
        &["worktree", "add", wt_target.to_str().unwrap(), "-b", "feature"],
        &main_path,
    ));

    (main_dir, main_path, wt_dir, wt_target)
}

#[test]
fn linked_worktree_detected_as_linked() {
    let (_main_dir, _main_path, _wt_dir, wt_path) = setup_worktree();

    let repo = Repository::discover(&wt_path).unwrap();
    assert_eq!(repo.kind(), RepositoryKind::LinkedWorktree);
}

#[test]
fn linked_worktree_has_correct_work_tree() {
    let (_main_dir, _main_path, _wt_dir, wt_path) = setup_worktree();

    let repo = Repository::discover(&wt_path).unwrap();
    assert_eq!(
        std::fs::canonicalize(repo.work_tree().unwrap()).unwrap(),
        std::fs::canonicalize(&wt_path).unwrap(),
    );
}

#[test]
fn linked_worktree_shares_common_dir() {
    let (_main_dir, main_path, _wt_dir, wt_path) = setup_worktree();

    let main_repo = Repository::open(&main_path).unwrap();
    let wt_repo = Repository::discover(&wt_path).unwrap();

    // Both should share the same common dir (the main .git)
    assert_eq!(
        std::fs::canonicalize(main_repo.common_dir()).unwrap(),
        std::fs::canonicalize(wt_repo.common_dir()).unwrap(),
    );
}

#[test]
fn linked_worktree_has_independent_git_dir() {
    let (_main_dir, main_path, _wt_dir, wt_path) = setup_worktree();

    let main_repo = Repository::open(&main_path).unwrap();
    let wt_repo = Repository::discover(&wt_path).unwrap();

    // Git dirs should be different
    assert_ne!(main_repo.git_dir(), wt_repo.git_dir());
}

#[test]
fn linked_worktree_shares_objects() {
    let (_main_dir, main_path, _wt_dir, wt_path) = setup_worktree();

    let main_repo = Repository::open(&main_path).unwrap();
    let wt_repo = Repository::discover(&wt_path).unwrap();

    // Both should have the same objects dir (via common_dir)
    assert_eq!(
        main_repo.odb().objects_dir(),
        wt_repo.odb().objects_dir(),
    );
}

#[test]
fn main_worktree_is_normal() {
    let (_main_dir, main_path, _wt_dir, _wt_path) = setup_worktree();

    let repo = Repository::open(&main_path).unwrap();
    assert_eq!(repo.kind(), RepositoryKind::Normal);
}

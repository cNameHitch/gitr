//! Discovery interoperability tests with C git.
//!
//! These tests verify that gitr repository discovery matches C git's behavior.
//! Tests that depend on environment variables are in env_tests.rs to avoid
//! interference with parallel test execution.

use std::process::Command;

use git_repository::{RepoError, Repository, RepositoryKind};

/// Create a temporary git repository via C git and return (tempdir, work_tree, git_dir).
fn setup_git_repo() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();

    let status = Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git init failed");

    let work_tree = std::fs::canonicalize(dir.path()).unwrap();
    let git_dir = work_tree.join(".git");
    (dir, work_tree, git_dir)
}

#[test]
fn discover_from_work_tree_root() {
    let (_dir, work_tree, git_dir) = setup_git_repo();

    let repo = Repository::discover(&work_tree).unwrap();
    assert_eq!(repo.git_dir(), git_dir);
    assert_eq!(repo.work_tree().unwrap(), work_tree);
    assert_eq!(repo.kind(), RepositoryKind::Normal);
}

#[test]
fn discover_from_subdirectory() {
    let (_dir, work_tree, git_dir) = setup_git_repo();

    // Create a deep subdirectory
    let sub = work_tree.join("a").join("b").join("c");
    std::fs::create_dir_all(&sub).unwrap();

    let repo = Repository::discover(&sub).unwrap();
    assert_eq!(repo.git_dir(), git_dir);
    assert_eq!(repo.work_tree().unwrap(), work_tree);
}

#[test]
fn discover_bare_repository() {
    let dir = tempfile::tempdir().unwrap();

    let status = Command::new("git")
        .args(["init", "--bare"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git init --bare failed");

    let bare_dir = std::fs::canonicalize(dir.path()).unwrap();
    let repo = Repository::open(&bare_dir).unwrap();
    assert_eq!(repo.kind(), RepositoryKind::Bare);
    assert!(repo.is_bare());
    assert!(repo.work_tree().is_none());
}

#[test]
fn discover_not_a_repo() {
    let dir = tempfile::tempdir().unwrap();
    let result = Repository::discover(dir.path());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), RepoError::NotFound(_)));
}

#[test]
fn discover_gitdir_file_redirect() {
    let (_dir, _work_tree, git_dir) = setup_git_repo();

    // Create a separate directory with a .git file pointing to the real git dir
    let other = tempfile::tempdir().unwrap();
    let other_path = std::fs::canonicalize(other.path()).unwrap();
    std::fs::write(
        other_path.join(".git"),
        format!("gitdir: {}", git_dir.display()),
    )
    .unwrap();

    let repo = Repository::discover(&other_path).unwrap();
    assert_eq!(repo.git_dir(), git_dir);
}

#[test]
fn open_from_git_dir() {
    let (_dir, _work_tree, git_dir) = setup_git_repo();

    let repo = Repository::open(&git_dir).unwrap();
    assert_eq!(repo.git_dir(), git_dir);
    assert_eq!(repo.kind(), RepositoryKind::Normal);
}

#[test]
fn open_from_work_tree() {
    let (_dir, work_tree, git_dir) = setup_git_repo();

    let repo = Repository::open(&work_tree).unwrap();
    assert_eq!(repo.git_dir(), git_dir);
    assert_eq!(repo.work_tree().unwrap(), work_tree);
}

#[test]
fn reinit_is_safe_noop() {
    let (_dir, work_tree, _git_dir) = setup_git_repo();

    // Re-init should succeed without error
    let repo = Repository::init(&work_tree).unwrap();
    assert_eq!(repo.kind(), RepositoryKind::Normal);
}

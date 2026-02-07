//! Tests for the Repository struct — open, accessors, and convenience methods.

use std::process::Command;

use git_repository::{Repository, RepositoryKind};

/// Create a temporary git repository via C git with an initial commit.
fn setup_repo_with_commit() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let work_tree = std::fs::canonicalize(dir.path()).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&work_tree)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
    };

    assert!(run(&["init"]).success());
    assert!(run(&["commit", "--allow-empty", "-m", "initial"]).success());

    (dir, work_tree)
}

/// Create a temporary empty (unborn) git repository.
fn setup_empty_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let work_tree = std::fs::canonicalize(dir.path()).unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&work_tree)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();

    (dir, work_tree)
}

#[test]
fn open_from_work_tree() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    assert_eq!(repo.kind(), RepositoryKind::Normal);
    assert!(!repo.is_bare());
    assert_eq!(repo.work_tree().unwrap(), work_tree);
    assert_eq!(repo.git_dir(), work_tree.join(".git"));
}

#[test]
fn open_from_git_dir() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let git_dir = work_tree.join(".git");
    let repo = Repository::open(&git_dir).unwrap();

    assert_eq!(repo.git_dir(), git_dir);
    assert_eq!(repo.kind(), RepositoryKind::Normal);
}

#[test]
fn odb_accessor_works() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    // ODB should be functional — we should be able to access it
    let _odb = repo.odb();
    assert_eq!(repo.odb().objects_dir(), work_tree.join(".git").join("objects"));
}

#[test]
fn refs_accessor_works() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    // Refs should be accessible
    let _refs = repo.refs();
}

#[test]
fn config_accessor_works() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    // Config should be accessible and contain bare = false
    let config = repo.config();
    let bare = config.get_bool("core.bare").unwrap();
    assert_eq!(bare, Some(false));
}

#[test]
fn index_accessor_works() {
    // Use gitr-init repo (no index file initially = empty index)
    let dir = tempfile::tempdir().unwrap();
    let _repo = Repository::init(dir.path()).unwrap();
    let work_tree = std::fs::canonicalize(dir.path()).unwrap();

    let mut repo = Repository::open(&work_tree).unwrap();
    let index = repo.index().unwrap();
    assert_eq!(index.len(), 0);
}

#[test]
fn head_oid_with_commit() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    let head_oid = repo.head_oid().unwrap();
    assert!(head_oid.is_some(), "HEAD should resolve to an OID after a commit");

    // Compare with C git
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&work_tree)
        .output()
        .unwrap();
    let c_oid = String::from_utf8(output.stdout).unwrap().trim().to_string();
    assert_eq!(head_oid.unwrap().to_hex(), c_oid);
}

#[test]
fn current_branch_after_init() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    let branch = repo.current_branch().unwrap();
    assert!(branch.is_some());
    // C git may use 'master' or 'main' depending on configuration
    let name = branch.unwrap();
    assert!(
        name == "main" || name == "master",
        "expected main or master, got: {name}"
    );
}

#[test]
fn is_unborn_on_new_repo() {
    let (_dir, work_tree) = setup_empty_repo();
    let repo = Repository::open(&work_tree).unwrap();

    assert!(repo.is_unborn().unwrap(), "new repo should be unborn");
}

#[test]
fn is_unborn_false_after_commit() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    assert!(!repo.is_unborn().unwrap(), "repo with commit should not be unborn");
}

#[test]
fn head_oid_none_on_unborn() {
    let (_dir, work_tree) = setup_empty_repo();
    let repo = Repository::open(&work_tree).unwrap();

    let head = repo.head_oid().unwrap();
    assert!(head.is_none(), "unborn repo should have no HEAD OID");
}

#[test]
fn common_dir_equals_git_dir_for_normal_repo() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    assert_eq!(repo.common_dir(), repo.git_dir());
}

#[test]
fn hash_algo_default_sha1() {
    let (_dir, work_tree) = setup_repo_with_commit();
    let repo = Repository::open(&work_tree).unwrap();

    assert_eq!(repo.hash_algo(), git_hash::HashAlgorithm::Sha1);
}

#[test]
fn reload_index() {
    // Use gitr-init repo to avoid C git TREE extension parsing issue
    let dir = tempfile::tempdir().unwrap();
    let _repo = Repository::init(dir.path()).unwrap();
    let work_tree = std::fs::canonicalize(dir.path()).unwrap();

    let mut repo = Repository::open(&work_tree).unwrap();

    // Load index first time
    let _idx1 = repo.index().unwrap();

    // Reload
    let _idx2 = repo.reload_index().unwrap();
}

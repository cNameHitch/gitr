//! Tests for environment variable handling.
//!
//! These tests manipulate process-global environment variables, so they use
//! a mutex to ensure they run one at a time and don't interfere with other tests.

use std::process::Command;
use std::sync::Mutex;

use git_repository::Repository;

/// Global lock for env-var tests to prevent parallel interference.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn setup_git_repo() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();

    let work_tree = std::fs::canonicalize(dir.path()).unwrap();
    let git_dir = work_tree.join(".git");
    (dir, work_tree, git_dir)
}

#[test]
fn git_dir_env_overrides_discovery() {
    let _lock = ENV_LOCK.lock().unwrap();
    let (_dir, _work_tree, git_dir) = setup_git_repo();

    let other = tempfile::tempdir().unwrap();

    std::env::set_var("GIT_DIR", &git_dir);
    let repo = Repository::discover(other.path()).unwrap();
    std::env::remove_var("GIT_DIR");

    assert_eq!(repo.git_dir(), git_dir);
}

#[test]
fn git_work_tree_env_overrides_work_tree() {
    let _lock = ENV_LOCK.lock().unwrap();
    let (_dir, _work_tree, git_dir) = setup_git_repo();

    let custom_wt = tempfile::tempdir().unwrap();
    let custom_wt_path = std::fs::canonicalize(custom_wt.path()).unwrap();

    std::env::set_var("GIT_WORK_TREE", &custom_wt_path);
    let repo = Repository::open(&git_dir).unwrap();
    std::env::remove_var("GIT_WORK_TREE");

    assert_eq!(repo.work_tree().unwrap(), custom_wt_path);
}

#[test]
fn git_ceiling_directories_blocks_discovery() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let dir_path = std::fs::canonicalize(dir.path()).unwrap();

    let sub = dir_path.join("a").join("b");
    std::fs::create_dir_all(&sub).unwrap();

    std::env::set_var("GIT_CEILING_DIRECTORIES", dir_path.to_str().unwrap());
    let result = Repository::discover(&sub);
    std::env::remove_var("GIT_CEILING_DIRECTORIES");

    assert!(result.is_err());
}

#[test]
fn git_object_directory_env_overrides_objects_path() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();

    let work_tree = std::fs::canonicalize(dir.path()).unwrap();

    // Create an alternate objects directory with the required structure
    let alt_objects = tempfile::tempdir().unwrap();
    let alt_path = std::fs::canonicalize(alt_objects.path()).unwrap();
    std::fs::create_dir_all(alt_path.join("pack")).unwrap();
    std::fs::create_dir_all(alt_path.join("info")).unwrap();

    std::env::set_var("GIT_OBJECT_DIRECTORY", &alt_path);
    let repo = Repository::open(&work_tree).unwrap();
    std::env::remove_var("GIT_OBJECT_DIRECTORY");

    assert_eq!(repo.odb().objects_dir(), alt_path);
}

#[test]
fn git_index_file_env_overrides_index_path() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _repo_init = Repository::init(dir.path()).unwrap();
    let work_tree = std::fs::canonicalize(dir.path()).unwrap();

    let custom_index = tempfile::NamedTempFile::new().unwrap();

    std::env::set_var("GIT_INDEX_FILE", custom_index.path());
    let mut repo = Repository::open(&work_tree).unwrap();
    let _idx = repo.index();
    std::env::remove_var("GIT_INDEX_FILE");
}

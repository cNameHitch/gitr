//! Initialization interoperability tests with C git.
//!
//! These tests verify that repositories created by gitr are usable by C git
//! and have identical structure to those created by `git init`.

use std::process::Command;

use git_repository::{InitOptions, Repository, RepositoryKind};

/// Verify C git can operate on a gitr-initialized repo.
fn c_git_status(repo_dir: &std::path::Path) -> bool {
    Command::new("git")
        .args(["status"])
        .current_dir(repo_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get C git's --git-dir from a directory.
fn c_git_git_dir(repo_dir: &std::path::Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(repo_dir)
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Check if C git considers a directory a bare repo.
fn c_git_is_bare(repo_dir: &std::path::Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--is-bare-repository"])
        .current_dir(repo_dir)
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        == "true"
}

#[test]
fn init_creates_standard_structure() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    let git_dir = repo.git_dir();

    // Verify standard directories exist
    assert!(git_dir.join("objects").is_dir());
    assert!(git_dir.join("objects").join("info").is_dir());
    assert!(git_dir.join("objects").join("pack").is_dir());
    assert!(git_dir.join("refs").is_dir());
    assert!(git_dir.join("refs").join("heads").is_dir());
    assert!(git_dir.join("refs").join("tags").is_dir());
    assert!(git_dir.join("hooks").is_dir());
    assert!(git_dir.join("info").is_dir());

    // Verify standard files exist
    assert!(git_dir.join("HEAD").is_file());
    assert!(git_dir.join("config").is_file());
    assert!(git_dir.join("description").is_file());
    assert!(git_dir.join("info").join("exclude").is_file());

    // HEAD should point to main by default
    let head = std::fs::read_to_string(git_dir.join("HEAD")).unwrap();
    assert_eq!(head.trim(), "ref: refs/heads/main");
}

#[test]
fn init_is_usable_by_c_git() {
    let dir = tempfile::tempdir().unwrap();
    let _repo = Repository::init(dir.path()).unwrap();

    // C git should be able to operate on the repo
    assert!(c_git_status(dir.path()));

    // C git should recognize the git dir
    let git_dir = c_git_git_dir(dir.path());
    assert_eq!(git_dir, ".git");
}

#[test]
fn init_bare_creates_bare_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init_bare(dir.path()).unwrap();

    assert_eq!(repo.kind(), RepositoryKind::Bare);
    assert!(repo.is_bare());
    assert!(repo.work_tree().is_none());

    // Bare repo structure: HEAD, objects, refs directly in dir
    assert!(dir.path().join("HEAD").is_file());
    assert!(dir.path().join("objects").is_dir());
    assert!(dir.path().join("refs").is_dir());

    // Config should have bare = true
    let config = std::fs::read_to_string(dir.path().join("config")).unwrap();
    assert!(config.contains("bare = true"));
}

#[test]
fn init_bare_usable_by_c_git() {
    let dir = tempfile::tempdir().unwrap();
    let _repo = Repository::init_bare(dir.path()).unwrap();

    // C git should recognize it as bare
    assert!(c_git_is_bare(dir.path()));
}

#[test]
fn init_reinit_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let _repo = Repository::init(dir.path()).unwrap();

    // Create a file inside .git to verify it's preserved
    let marker = dir.path().join(".git").join("test_marker");
    std::fs::write(&marker, "preserved").unwrap();

    // Re-init
    let _repo2 = Repository::init(dir.path()).unwrap();

    // Marker should still exist
    assert!(marker.exists());
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "preserved");
}

#[test]
fn init_with_custom_default_branch() {
    let dir = tempfile::tempdir().unwrap();
    let opts = InitOptions {
        default_branch: Some("develop".to_string()),
        ..Default::default()
    };
    let _repo = Repository::init_opts(dir.path(), &opts).unwrap();

    let head = std::fs::read_to_string(dir.path().join(".git").join("HEAD")).unwrap();
    assert_eq!(head.trim(), "ref: refs/heads/develop");
}

#[test]
fn init_with_template_directory() {
    let dir = tempfile::tempdir().unwrap();

    // Create a template directory with a custom hook
    let template = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(template.path().join("hooks")).unwrap();
    std::fs::write(
        template.path().join("hooks").join("pre-commit"),
        "#!/bin/sh\nexit 0\n",
    )
    .unwrap();

    let opts = InitOptions {
        template_dir: Some(template.path().to_path_buf()),
        ..Default::default()
    };
    let repo = Repository::init_opts(dir.path(), &opts).unwrap();

    // The template hook should be copied
    let hook = repo.git_dir().join("hooks").join("pre-commit");
    assert!(hook.exists());
    let content = std::fs::read_to_string(&hook).unwrap();
    assert!(content.contains("exit 0"));
}

#[test]
fn c_git_init_matches_gitr_structure() {
    // Init with C git
    let c_dir = tempfile::tempdir().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(c_dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();

    // Init with gitr
    let m_dir = tempfile::tempdir().unwrap();
    let _repo = Repository::init(m_dir.path()).unwrap();

    // Both should have the same set of directories
    let c_git = c_dir.path().join(".git");
    let m_git = m_dir.path().join(".git");

    for subdir in &["objects", "objects/info", "objects/pack", "refs", "refs/heads", "refs/tags", "info"] {
        assert!(
            c_git.join(subdir).is_dir(),
            "C git missing dir: {subdir}"
        );
        assert!(
            m_git.join(subdir).is_dir(),
            "gitr missing dir: {subdir}"
        );
    }

    // Both should have HEAD and config
    for file in &["HEAD", "config", "description"] {
        assert!(
            c_git.join(file).is_file(),
            "C git missing file: {file}"
        );
        assert!(
            m_git.join(file).is_file(),
            "gitr missing file: {file}"
        );
    }
}

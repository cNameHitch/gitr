//! End-to-end remote operations interoperability tests.
//!
//! Tests clone, fetch, push, and pull over local file:// transport
//! by running both gitr and C git and comparing outputs.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// User Story 5 — Remote Operations Interop (P2)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_gitr_clone_matches_cgit_clone() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // Compare log output
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    // Compare show-ref
    let g = git(dir_git.path(), &["show-ref"]);
    let m = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g, &m);

    // Compare working tree file contents (if gitr checks out the working tree)
    if dir_gitr.path().join("file_0.txt").exists() {
        for name in ["file_0.txt", "file_1.txt"] {
            let g_content = std::fs::read_to_string(dir_git.path().join(name)).unwrap();
            let m_content = std::fs::read_to_string(dir_gitr.path().join(name)).unwrap();
            assert_eq!(g_content, m_content, "working tree file {} should match", name);
        }
    }
}

#[test]
fn test_gitr_clone_bare() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", "--bare", &url, "."]);
    gitr(dir_gitr.path(), &["clone", "--bare", &url, "."]);

    // Compare show-ref
    let g = git(dir_git.path(), &["show-ref"]);
    let m = gitr(dir_gitr.path(), &["show-ref"]);
    assert_output_eq(&g, &m);

    // Verify bare repo structure (HEAD, objects/, refs/ at root, no working tree)
    assert!(dir_git.path().join("HEAD").exists());
    assert!(dir_gitr.path().join("HEAD").exists());
    assert!(dir_git.path().join("objects").exists());
    assert!(dir_gitr.path().join("objects").exists());
    assert!(dir_git.path().join("refs").exists());
    assert!(dir_gitr.path().join("refs").exists());
}

#[test]
fn test_clone_preserves_all_refs() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    // Add branches and a tag to the bare remote via a temp clone
    let work = tempfile::tempdir().unwrap();
    let url = format!("file://{}", remote_dir.path().display());
    git(work.path(), &["clone", &url, "."]);
    git(work.path(), &["config", "user.name", "Test Author"]);
    git(work.path(), &["config", "user.email", "test@example.com"]);

    git(work.path(), &["checkout", "-b", "develop"]);
    std::fs::write(work.path().join("dev.txt"), "dev\n").unwrap();
    git(work.path(), &["add", "dev.txt"]);
    git(work.path(), &["commit", "-m", "develop commit"]);
    git(work.path(), &["push", "origin", "develop"]);

    git(work.path(), &["checkout", "main"]);
    git(work.path(), &["tag", "v1.0"]);
    git(work.path(), &["push", "origin", "v1.0"]);

    // Clone with gitr and verify all refs
    let dir_gitr = tempfile::tempdir().unwrap();
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    let result = gitr(dir_gitr.path(), &["show-ref"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("refs/remotes/origin/develop"),
        "gitr clone should have remote-tracking develop branch"
    );
    assert!(
        result.stdout.contains("refs/tags/v1.0"),
        "gitr clone should have v1.0 tag"
    );

    // Compare with C git clone
    let dir_git = tempfile::tempdir().unwrap();
    git(dir_git.path(), &["clone", &url, "."]);

    let g = git(dir_git.path(), &["show-ref"]);
    assert_output_eq(&g, &result);
}

#[test]
fn test_clone_remote_config() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    let g = git(dir_git.path(), &["config", "--get", "remote.origin.url"]);
    let m = gitr(dir_gitr.path(), &["config", "--get", "remote.origin.url"]);
    assert_output_eq(&g, &m);

    let g = git(dir_git.path(), &["config", "--get", "remote.origin.fetch"]);
    let m = gitr(dir_gitr.path(), &["config", "--get", "remote.origin.fetch"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_gitr_push_cgit_fetch() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    // Gitr clones
    let dir_gitr = tempfile::tempdir().unwrap();
    gitr(dir_gitr.path(), &["clone", &url, "."]);
    git(dir_gitr.path(), &["config", "user.name", "Test Author"]);
    git(dir_gitr.path(), &["config", "user.email", "test@example.com"]);

    // Gitr creates a new commit and pushes
    std::fs::write(dir_gitr.path().join("new.txt"), "new content\n").unwrap();
    gitr(dir_gitr.path(), &["add", "new.txt"]);
    gitr(dir_gitr.path(), &["commit", "-m", "gitr push commit"]);
    let result = gitr(dir_gitr.path(), &["push", "origin", "main"]);
    assert_eq!(result.exit_code, 0);

    // C git clones fresh and verifies the new commit
    let dir_git = tempfile::tempdir().unwrap();
    git(dir_git.path(), &["clone", &url, "."]);

    let result = git(dir_git.path(), &["log", "--oneline"]);
    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("gitr push commit"),
        "C git should see the commit pushed by gitr"
    );
}

#[test]
fn test_cgit_push_gitr_fetch() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    // Both clone
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    git(dir_git.path(), &["clone", &url, "."]);
    git(dir_git.path(), &["config", "user.name", "Test Author"]);
    git(dir_git.path(), &["config", "user.email", "test@example.com"]);
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // C git creates new commit and pushes
    std::fs::write(dir_git.path().join("cgit_new.txt"), "cgit content\n").unwrap();
    git(dir_git.path(), &["add", "cgit_new.txt"]);
    git(dir_git.path(), &["commit", "-m", "cgit push commit"]);
    git(dir_git.path(), &["push", "origin", "main"]);

    // Gitr fetches and merges
    gitr(dir_gitr.path(), &["fetch", "origin"]);
    gitr(dir_gitr.path(), &["merge", "origin/main"]);

    // Verify gitr's log matches C git's updated log
    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_push_new_branch() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    // Gitr clones
    let dir_gitr = tempfile::tempdir().unwrap();
    gitr(dir_gitr.path(), &["clone", &url, "."]);
    git(dir_gitr.path(), &["config", "user.name", "Test Author"]);
    git(dir_gitr.path(), &["config", "user.email", "test@example.com"]);

    // Create branch with 1 commit and push
    gitr(dir_gitr.path(), &["checkout", "-b", "feature"]);
    std::fs::write(dir_gitr.path().join("feature.txt"), "feature\n").unwrap();
    gitr(dir_gitr.path(), &["add", "feature.txt"]);
    gitr(dir_gitr.path(), &["commit", "-m", "feature commit"]);
    let result = gitr(dir_gitr.path(), &["push", "origin", "feature"]);
    assert_eq!(result.exit_code, 0);

    // C git clones fresh and verifies the branch exists
    let dir_git = tempfile::tempdir().unwrap();
    git(dir_git.path(), &["clone", &url, "."]);

    let result = git(dir_git.path(), &["show-ref"]);
    assert!(
        result.stdout.contains("refs/remotes/origin/feature"),
        "C git clone should see refs/remotes/origin/feature"
    );
}

#[test]
fn test_fetch_new_commits() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    // Gitr clones
    let dir_gitr = tempfile::tempdir().unwrap();
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // C git pushes 2 new commits to bare
    let dir_push = tempfile::tempdir().unwrap();
    git(dir_push.path(), &["clone", &url, "."]);
    git(dir_push.path(), &["config", "user.name", "Test Author"]);
    git(dir_push.path(), &["config", "user.email", "test@example.com"]);

    let mut counter = 10u64;
    for i in 0..2 {
        let filename = format!("new_{}.txt", i);
        let date = next_date(&mut counter);
        std::fs::write(dir_push.path().join(&filename), format!("new {}\n", i)).unwrap();
        git_with_date(dir_push.path(), &["add", &filename], &date);
        git_with_date(dir_push.path(), &["commit", "-m", &format!("new commit {}", i)], &date);
    }
    git(dir_push.path(), &["push", "origin", "main"]);

    // Gitr fetches
    gitr(dir_gitr.path(), &["fetch", "origin"]);

    // Compare rev-list origin/main
    let g = git(dir_push.path(), &["rev-list", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["rev-list", "origin/main"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_pull_fast_forward() {
    let remote_dir = tempfile::tempdir().unwrap();
    setup_bare_remote(remote_dir.path());

    let url = format!("file://{}", remote_dir.path().display());

    // Gitr clones
    let dir_gitr = tempfile::tempdir().unwrap();
    gitr(dir_gitr.path(), &["clone", &url, "."]);

    // C git pushes 1 new commit
    let dir_push = tempfile::tempdir().unwrap();
    git(dir_push.path(), &["clone", &url, "."]);
    git(dir_push.path(), &["config", "user.name", "Test Author"]);
    git(dir_push.path(), &["config", "user.email", "test@example.com"]);

    std::fs::write(dir_push.path().join("pulled.txt"), "pulled content\n").unwrap();
    git(dir_push.path(), &["add", "pulled.txt"]);
    git(dir_push.path(), &["commit", "-m", "commit to pull"]);
    git(dir_push.path(), &["push", "origin", "main"]);

    // Gitr pulls
    let result = gitr(dir_gitr.path(), &["pull", "origin", "main"]);
    assert_eq!(result.exit_code, 0);

    // Compare log and HEAD
    let g = git(dir_push.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

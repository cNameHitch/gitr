//! Tests for commit-graph correctness and fallback behavior.

use std::path::Path;
use std::process::Command;

use git_hash::{HashAlgorithm, ObjectId};
use git_object::Object;
use git_repository::Repository;
use git_revwalk::{CommitGraph, CommitGraphWriter, RevWalk, SortOrder};

/// Helper: run a git command in the given directory and return stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "author@test.com")
        .env("GIT_COMMITTER_NAME", "Test Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@test.com")
        .output()
        .expect("failed to run git");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git {:?} failed: {}", args, stderr);
    }
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn git_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "author@test.com")
        .env("GIT_COMMITTER_NAME", "Test Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@test.com");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("failed to run git");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("git {:?} failed: {}", args, stderr);
    }
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Create a repo with N sequential commits.
fn create_linear_repo(dir: &Path, n: usize) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    for i in 0..n {
        let filename = format!("file_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("content {}\n", i)).unwrap();
        let date = format!("{} +0000", 1234567890u64 + i as u64 + 1);
        git_env(
            dir,
            &["add", &filename],
            &[
                ("GIT_AUTHOR_DATE", &date),
                ("GIT_COMMITTER_DATE", &date),
            ],
        );
        git_env(
            dir,
            &["commit", "-m", &format!("commit {}", i)],
            &[
                ("GIT_AUTHOR_DATE", &date),
                ("GIT_COMMITTER_DATE", &date),
            ],
        );
    }
}

/// T018: RevWalk produces identical commit ordering with and without a commit-graph.
#[test]
fn revwalk_ordering_identical_with_and_without_commit_graph() {
    let dir = tempfile::tempdir().unwrap();
    create_linear_repo(dir.path(), 30);

    // Collect commits WITHOUT commit-graph
    let oids_without: Vec<ObjectId>;
    {
        let repo = Repository::open(dir.path()).unwrap();
        let mut walk = RevWalk::new(&repo).unwrap();
        walk.push_head().unwrap();
        oids_without = walk.collect::<Result<Vec<_>, _>>().unwrap();
    }

    // Generate commit-graph with C git
    git(dir.path(), &["commit-graph", "write"]);

    // Collect commits WITH commit-graph
    let oids_with: Vec<ObjectId>;
    {
        let repo = Repository::open(dir.path()).unwrap();
        let mut walk = RevWalk::new(&repo).unwrap();
        walk.push_head().unwrap();
        oids_with = walk.collect::<Result<Vec<_>, _>>().unwrap();
    }

    assert_eq!(
        oids_without.len(),
        oids_with.len(),
        "commit count mismatch: without graph = {}, with graph = {}",
        oids_without.len(),
        oids_with.len()
    );

    for (i, (a, b)) in oids_without.iter().zip(oids_with.iter()).enumerate() {
        assert_eq!(a, b, "commit mismatch at position {}: {:?} vs {:?}", i, a, b);
    }
}

/// T019: read_commit_meta gracefully falls back when commit-graph is absent.
#[test]
fn revwalk_works_without_commit_graph() {
    let dir = tempfile::tempdir().unwrap();
    create_linear_repo(dir.path(), 20);

    // Ensure no commit-graph file exists
    let graph_path = dir.path().join(".git/objects/info/commit-graph");
    assert!(!graph_path.exists(), "commit-graph should not exist yet");

    // RevWalk should still work fine via ODB fallback
    let repo = Repository::open(dir.path()).unwrap();
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.push_head().unwrap();
    let oids: Vec<ObjectId> = walk.collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(oids.len(), 20, "expected 20 commits, got {}", oids.len());

    // Verify against git rev-list
    let expected = git(dir.path(), &["rev-list", "HEAD"]);
    let expected_oids: Vec<&str> = expected.lines().collect();
    assert_eq!(oids.len(), expected_oids.len());

    for (i, (walk_oid, git_oid)) in oids.iter().zip(expected_oids.iter()).enumerate() {
        assert_eq!(
            &walk_oid.to_string(),
            git_oid,
            "mismatch at position {}: revwalk={}, git={}",
            i,
            walk_oid,
            git_oid
        );
    }
}

/// Topological sort produces identical results with and without commit-graph.
#[test]
fn revwalk_topo_ordering_identical_with_and_without_commit_graph() {
    let dir = tempfile::tempdir().unwrap();
    create_linear_repo(dir.path(), 20);

    // Without commit-graph
    let oids_without: Vec<ObjectId>;
    {
        let repo = Repository::open(dir.path()).unwrap();
        let mut walk = RevWalk::new(&repo).unwrap();
        walk.set_sort(SortOrder::Topological);
        walk.push_head().unwrap();
        oids_without = walk.collect::<Result<Vec<_>, _>>().unwrap();
    }

    // Generate commit-graph
    git(dir.path(), &["commit-graph", "write"]);

    // With commit-graph
    let oids_with: Vec<ObjectId>;
    {
        let repo = Repository::open(dir.path()).unwrap();
        let mut walk = RevWalk::new(&repo).unwrap();
        walk.set_sort(SortOrder::Topological);
        walk.push_head().unwrap();
        oids_with = walk.collect::<Result<Vec<_>, _>>().unwrap();
    }

    assert_eq!(oids_without, oids_with, "topological ordering differs with commit-graph");
}

/// T040: Round-trip test — write with CommitGraphWriter, read back with CommitGraph::open.
#[test]
fn commit_graph_writer_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    create_linear_repo(dir.path(), 25);

    let repo = Repository::open(dir.path()).unwrap();
    let graph_path = dir.path().join(".git/objects/info/commit-graph");

    // Collect all commit data
    let mut walk = RevWalk::new(&repo).unwrap();
    walk.push_head().unwrap();
    let oids: Vec<ObjectId> = walk.collect::<Result<Vec<_>, _>>().unwrap();

    // Write commit-graph with our writer
    let mut writer = CommitGraphWriter::new(HashAlgorithm::Sha1);
    for oid in &oids {
        let obj = repo.odb().read(oid).unwrap();
        if let Some(Object::Commit(commit)) = obj {
            writer.add_commit(*oid, commit.tree, commit.parents, commit.committer.date.timestamp);
        }
    }
    writer.write(&graph_path).unwrap();

    // Read back with CommitGraph::open
    let graph = CommitGraph::open(&graph_path).unwrap();

    // Verify: num_commits matches
    assert_eq!(
        graph.num_commits() as usize,
        oids.len(),
        "commit count mismatch"
    );

    // Verify: every commit can be looked up
    for oid in &oids {
        let entry = graph.lookup(oid);
        assert!(
            entry.is_some(),
            "commit {} not found in graph after roundtrip",
            oid
        );
    }

    // Verify: checksum is valid
    graph.verify().unwrap();

    // Verify: C git also accepts our graph
    let verify_output = Command::new("git")
        .args(["commit-graph", "verify"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        verify_output.status.success(),
        "C git commit-graph verify failed: {}",
        String::from_utf8_lossy(&verify_output.stderr)
    );
}

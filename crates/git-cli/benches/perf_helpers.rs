//! Shared helpers for performance comparison benchmarks.
//!
//! Provides process runners and repo setup utilities adapted from
//! `crates/git-cli/tests/common/mod.rs`, simplified for benchmark use
//! (no assertion helpers needed).

use std::path::{Path, PathBuf};
use std::process::Command;

// ──────────────────────────── Binary Discovery ────────────────────────────

/// Discover the path to the compiled `gitr` binary.
///
/// When run via `cargo bench`, the binary lives two directories up from
/// the benchmark executable in `target/release/deps/`.
pub fn gitr_bin() -> PathBuf {
    // cargo bench compiles into target/release/deps/<bench_name>-<hash>
    // The gitr binary is at target/release/gitr
    let mut path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    path.push("gitr");
    path
}

// ──────────────────────────── Environment Pinning ────────────────────────────

/// Apply deterministic environment variables to a `Command`.
fn pin_env(cmd: &mut Command, dir: &Path) {
    cmd.env("GIT_AUTHOR_NAME", "Bench Author")
        .env("GIT_AUTHOR_EMAIL", "bench@example.com")
        .env("GIT_AUTHOR_DATE", "1234567890 +0000")
        .env("GIT_COMMITTER_NAME", "Bench Committer")
        .env("GIT_COMMITTER_EMAIL", "bench@example.com")
        .env("GIT_COMMITTER_DATE", "1234567890 +0000")
        .env("TZ", "UTC")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir.parent().unwrap_or(dir))
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "protocol.file.allow")
        .env("GIT_CONFIG_VALUE_0", "always");
}

// ──────────────────────────── Process Runners ────────────────────────────

/// Run C git in `dir` with the given arguments. Panics on spawn failure.
pub fn run_git(dir: &Path, args: &[&str]) {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    let output = cmd.output().expect("failed to run git");
    if !output.status.success() {
        // Silently ignore for benchmarks — some commands may legitimately
        // return non-zero (e.g., diff with changes).
    }
}

/// Run gitr in `dir` with the given arguments. Panics on spawn failure.
pub fn run_gitr(dir: &Path, args: &[&str]) {
    let mut cmd = Command::new(gitr_bin());
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    let output = cmd.output().expect("failed to run gitr");
    if !output.status.success() {
        // Silently ignore for benchmarks.
    }
}

/// Run C git with a specific date override. Returns output for setup use.
fn git_setup(dir: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    cmd.output().expect("failed to run git")
}

/// Run C git with a specific date for setup commits.
fn git_setup_with_date(dir: &Path, args: &[&str], epoch: &str) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    cmd.env("GIT_AUTHOR_DATE", epoch)
        .env("GIT_COMMITTER_DATE", epoch);
    cmd.output().expect("failed to run git")
}

/// Run C git and capture stdout as a String (for setup that needs output).
pub fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = git_setup(dir, args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Generate a deterministic date string.
fn next_date(counter: &mut u64) -> String {
    *counter += 1;
    format!("{} +0000", 1234567890u64 + *counter)
}

// ──────────────────────────── Repo Setup ────────────────────────────

/// Repository size preset for benchmarks.
#[derive(Clone, Copy)]
pub enum RepoSize {
    /// 10 files, 10 commits, 2 branches
    Small,
    /// 1,000 files, 100 commits, 10 branches
    Medium,
    /// 10,000 files, 500 commits, 20 branches
    Large,
}

impl RepoSize {
    pub fn label(self) -> &'static str {
        match self {
            RepoSize::Small => "small",
            RepoSize::Medium => "medium",
            RepoSize::Large => "large",
        }
    }

    fn params(self) -> (usize, usize, usize) {
        match self {
            RepoSize::Small => (10, 10, 2),
            RepoSize::Medium => (1_000, 100, 10),
            RepoSize::Large => (10_000, 500, 20),
        }
    }
}

/// Build a test repository at `dir` with the given size preset.
///
/// Creates files spread across directories, sequential commits on main,
/// branches at evenly-spaced points, and tags on every 10th commit.
pub fn setup_repo(dir: &Path, size: RepoSize) {
    let (files, commits, branches) = size.params();

    // git init
    git_setup(dir, &["init", "-b", "main"]);
    git_setup(dir, &["config", "user.name", "Bench Author"]);
    git_setup(dir, &["config", "user.email", "bench@example.com"]);

    let mut counter = 0u64;

    // Create initial file set (spread across directories)
    if files > 0 {
        for i in 0..files {
            let subdir = format!("dir_{}", i % 50);
            std::fs::create_dir_all(dir.join(&subdir)).unwrap();
            let filename = format!("{}/file_{}.txt", subdir, i);
            std::fs::write(dir.join(&filename), format!("initial content {}\n", i)).unwrap();
        }
        let date = next_date(&mut counter);
        git_setup_with_date(dir, &["add", "."], &date);
        git_setup_with_date(dir, &["commit", "-m", "initial files"], &date);
    }

    // Sequential commits (each modifies/adds a unique file)
    let start = if files > 0 { 1 } else { 0 };
    for i in start..commits {
        let filename = format!("commit_file_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("commit content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_setup_with_date(dir, &["add", &filename], &date);
        git_setup_with_date(dir, &["commit", "-m", &format!("commit {}", i)], &date);
    }

    // Create branches at evenly-spaced points
    if branches > 0 && commits > 0 {
        let interval = std::cmp::max(1, commits / branches);
        for b in 0..branches {
            let offset = std::cmp::min(b * interval, commits.saturating_sub(1));
            let rev = format!("HEAD~{}", commits.saturating_sub(1) - offset);
            git_setup(dir, &["branch", &format!("branch-{}", b), &rev]);
        }
    }

    // Create tags on every 10th commit (or fewer for small repos)
    let tag_interval = if commits >= 20 { 10 } else { std::cmp::max(1, commits / 2) };
    for (tag_count, i) in (0..commits).step_by(tag_interval).enumerate() {
        let rev = format!("HEAD~{}", commits.saturating_sub(1) - i);
        git_setup(dir, &["tag", &format!("v0.{}", tag_count), &rev]);
    }
}

/// Introduce working-tree modifications for diff/status benchmarks.
///
/// Modifies some existing files and creates new untracked files.
pub fn dirty_worktree(dir: &Path, count: usize) {
    // Modify existing tracked files
    for i in 0..count {
        let filename = format!("commit_file_{}.txt", i);
        let path = dir.join(&filename);
        if path.exists() {
            std::fs::write(&path, format!("modified content {}\n", i)).unwrap();
        }
    }
    // Create untracked files
    for i in 0..std::cmp::min(count, 5) {
        std::fs::write(
            dir.join(format!("untracked_{}.txt", i)),
            format!("untracked {}\n", i),
        )
        .unwrap();
    }
}

/// Stage some changes for `diff --cached` benchmarks.
pub fn stage_changes(dir: &Path, count: usize) {
    for i in 0..count {
        let filename = format!("commit_file_{}.txt", i);
        let path = dir.join(&filename);
        if path.exists() {
            std::fs::write(&path, format!("staged content {}\n", i)).unwrap();
        }
    }
    git_setup(dir, &["add", "."]);
}

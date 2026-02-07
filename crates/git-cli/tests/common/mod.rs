//! Shared test harness for git-cli integration tests.
//!
//! Provides process runners, assertion helpers, and repo setup utilities
//! used by all test files. Environment variables are fully pinned for
//! deterministic output across machines and CI runners.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ──────────────────────────── Types ────────────────────────────

/// Captured output from running a command.
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// ──────────────────────────── Binary Discovery ────────────────────────────

/// Discover the path to the compiled `gitr` binary.
pub fn gitr_bin() -> PathBuf {
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

// ──────────────────────────── Process Runners ────────────────────────────

/// Apply the full set of pinned environment variables to a `Command`.
fn pin_env(cmd: &mut Command, dir: &Path) {
    cmd.env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_AUTHOR_DATE", "1234567890 +0000")
        .env("GIT_COMMITTER_NAME", "Test Committer")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
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

/// Run C git in `dir` with the given arguments. Returns a `CommandResult`.
pub fn git(dir: &Path, args: &[&str]) -> CommandResult {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    let output = cmd.output().expect("failed to run git");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run the gitr binary in `dir` with the given arguments. Returns a `CommandResult`.
pub fn gitr(dir: &Path, args: &[&str]) -> CommandResult {
    let mut cmd = Command::new(gitr_bin());
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    let output = cmd.output().expect("failed to run gitr");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run C git with a specific date override (for multi-commit scenarios).
pub fn git_with_date(dir: &Path, args: &[&str], epoch: &str) -> CommandResult {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    cmd.env("GIT_AUTHOR_DATE", epoch)
        .env("GIT_COMMITTER_DATE", epoch);
    let output = cmd.output().expect("failed to run git");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run gitr with a specific date override (for multi-commit scenarios).
pub fn gitr_with_date(dir: &Path, args: &[&str], epoch: &str) -> CommandResult {
    let mut cmd = Command::new(gitr_bin());
    cmd.args(args).current_dir(dir);
    pin_env(&mut cmd, dir);
    cmd.env("GIT_AUTHOR_DATE", epoch)
        .env("GIT_COMMITTER_DATE", epoch);
    let output = cmd.output().expect("failed to run gitr");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run C git with piped stdin in `dir`. Returns a `CommandResult`.
pub fn git_stdin(dir: &Path, args: &[&str], stdin_bytes: &[u8]) -> CommandResult {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    pin_env(&mut cmd, dir);
    let mut child = cmd.spawn().expect("failed to spawn git");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(stdin_bytes).unwrap();
    }
    let output = child.wait_with_output().expect("failed to wait on git");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run gitr with piped stdin in `dir`. Returns a `CommandResult`.
pub fn gitr_stdin(dir: &Path, args: &[&str], stdin_bytes: &[u8]) -> CommandResult {
    let mut cmd = Command::new(gitr_bin());
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    pin_env(&mut cmd, dir);
    let mut child = cmd.spawn().expect("failed to spawn gitr");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(stdin_bytes).unwrap();
    }
    let output = child.wait_with_output().expect("failed to wait on gitr");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run C git with piped stdin and a specific date override.
pub fn git_stdin_with_date(dir: &Path, args: &[&str], stdin_bytes: &[u8], epoch: &str) -> CommandResult {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    pin_env(&mut cmd, dir);
    cmd.env("GIT_AUTHOR_DATE", epoch)
        .env("GIT_COMMITTER_DATE", epoch);
    let mut child = cmd.spawn().expect("failed to spawn git");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(stdin_bytes).unwrap();
    }
    let output = child.wait_with_output().expect("failed to wait on git");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Run gitr with piped stdin and a specific date override.
pub fn gitr_stdin_with_date(dir: &Path, args: &[&str], stdin_bytes: &[u8], epoch: &str) -> CommandResult {
    let mut cmd = Command::new(gitr_bin());
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    pin_env(&mut cmd, dir);
    cmd.env("GIT_AUTHOR_DATE", epoch)
        .env("GIT_COMMITTER_DATE", epoch);
    let mut child = cmd.spawn().expect("failed to spawn gitr");
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(stdin_bytes).unwrap();
    }
    let output = child.wait_with_output().expect("failed to wait on gitr");
    CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(128),
    }
}

/// Generate a deterministic date string. Returns `"(1234567890 + counter) +0000"`.
pub fn next_date(counter: &mut u64) -> String {
    *counter += 1;
    format!("{} +0000", 1234567890u64 + *counter)
}

// ──────────────────────────── Assertion Helpers ────────────────────────────

/// Assert that stdout and exit_code are identical between git and gitr results.
pub fn assert_output_eq(git_result: &CommandResult, gitr_result: &CommandResult) {
    if git_result.exit_code != gitr_result.exit_code {
        panic!(
            "Exit code mismatch:\n  git:  {}\n  gitr: {}\n\ngit stdout:\n{}\ngitr stdout:\n{}\ngit stderr:\n{}\ngitr stderr:\n{}",
            git_result.exit_code, gitr_result.exit_code,
            git_result.stdout, gitr_result.stdout,
            git_result.stderr, gitr_result.stderr,
        );
    }
    if git_result.stdout != gitr_result.stdout {
        panic!(
            "Stdout mismatch (exit codes both {}):\n--- git ---\n{}\n--- gitr ---\n{}\n--- end ---",
            git_result.exit_code, git_result.stdout, gitr_result.stdout,
        );
    }
}

/// Assert that only stdout matches (ignoring exit code).
pub fn assert_stdout_eq(git_result: &CommandResult, gitr_result: &CommandResult) {
    if git_result.stdout != gitr_result.stdout {
        panic!(
            "Stdout mismatch:\n--- git (exit {}) ---\n{}\n--- gitr (exit {}) ---\n{}\n--- end ---",
            git_result.exit_code, git_result.stdout,
            gitr_result.exit_code, gitr_result.stdout,
        );
    }
}

/// Assert that only exit codes match.
pub fn assert_exit_code_eq(git_result: &CommandResult, gitr_result: &CommandResult) {
    if git_result.exit_code != gitr_result.exit_code {
        panic!(
            "Exit code mismatch:\n  git:  {} (stdout: {:?})\n  gitr: {} (stdout: {:?})",
            git_result.exit_code,
            git_result.stdout.chars().take(200).collect::<String>(),
            gitr_result.exit_code,
            gitr_result.stdout.chars().take(200).collect::<String>(),
        );
    }
}

/// Run `git fsck --full` on the given directory.
pub fn fsck(dir: &Path) -> CommandResult {
    git(dir, &["fsck", "--full"])
}

/// Run fsck and assert it passes cleanly.
pub fn assert_fsck_clean(dir: &Path) {
    let result = fsck(dir);
    assert_eq!(
        result.exit_code, 0,
        "fsck failed (exit {}):\nstdout: {}\nstderr: {}",
        result.exit_code, result.stdout, result.stderr,
    );
}

/// Compare two repository directories for equivalent state:
/// HEAD ref, all refs under refs/, and the set of loose object IDs.
pub fn assert_repo_state_eq(dir_a: &Path, dir_b: &Path) {
    // Compare HEAD
    let head_a = std::fs::read_to_string(dir_a.join(".git/HEAD"))
        .unwrap_or_else(|_| String::from("(no HEAD)"));
    let head_b = std::fs::read_to_string(dir_b.join(".git/HEAD"))
        .unwrap_or_else(|_| String::from("(no HEAD)"));
    if head_a != head_b {
        panic!(
            "HEAD divergence:\n  dir_a: {:?}\n  dir_b: {:?}",
            head_a.trim(),
            head_b.trim(),
        );
    }

    // Compare refs
    let refs_a = collect_refs(dir_a);
    let refs_b = collect_refs(dir_b);
    if refs_a != refs_b {
        panic!(
            "Refs divergence:\n  dir_a refs: {:?}\n  dir_b refs: {:?}",
            refs_a, refs_b,
        );
    }

    // Compare loose object IDs
    let objs_a = collect_loose_objects(dir_a);
    let objs_b = collect_loose_objects(dir_b);
    if objs_a != objs_b {
        let only_a: Vec<_> = objs_a.iter().filter(|o| !objs_b.contains(o)).collect();
        let only_b: Vec<_> = objs_b.iter().filter(|o| !objs_a.contains(o)).collect();
        panic!(
            "Object set divergence:\n  only in dir_a: {:?}\n  only in dir_b: {:?}",
            only_a, only_b,
        );
    }
}

/// Recursively collect all refs under `.git/refs/` as `(refname, oid)` pairs.
fn collect_refs(dir: &Path) -> Vec<(String, String)> {
    let refs_dir = dir.join(".git/refs");
    let mut refs = Vec::new();
    if refs_dir.exists() {
        collect_refs_recursive(&refs_dir, &refs_dir, &mut refs);
    }
    refs.sort();
    refs
}

fn collect_refs_recursive(base: &Path, current: &Path, refs: &mut Vec<(String, String)>) {
    if let Ok(entries) = std::fs::read_dir(current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_refs_recursive(base, &path, refs);
            } else if path.is_file() {
                let rel = path.strip_prefix(base).unwrap().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                refs.push((rel, content));
            }
        }
    }
}

/// Collect loose object IDs from `.git/objects/` (excluding `info/` and `pack/`).
fn collect_loose_objects(dir: &Path) -> Vec<String> {
    let objects_dir = dir.join(".git/objects");
    let mut oids = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&objects_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip non-object directories
            if name == "info" || name == "pack" {
                continue;
            }
            if entry.path().is_dir() && name.len() == 2 {
                if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                    for sub in sub_entries.flatten() {
                        let sub_name = sub.file_name().to_string_lossy().to_string();
                        oids.push(format!("{}{}", name, sub_name));
                    }
                }
            }
        }
    }
    oids.sort();
    oids
}

// ──────────────────────────── Repo Setup Helpers ────────────────────────────

/// Initialize an empty repo with `git init -b main` and basic config. No commits.
pub fn setup_empty_repo(dir: &Path) {
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.name", "Test Author"]);
    git(dir, &["config", "user.email", "test@example.com"]);
}

/// Create a repo with `n` sequential commits, each adding/modifying a file.
/// Uses deterministic content and incrementing dates.
pub fn setup_linear_history(dir: &Path, n: usize) {
    setup_empty_repo(dir);
    let mut counter = 0u64;
    for i in 0..n {
        let filename = format!("file_{}.txt", i);
        let content = format!("content for commit {}\n", i);
        std::fs::write(dir.join(&filename), &content).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        let msg = format!("commit {}", i);
        git_with_date(dir, &["commit", "-m", &msg], &date);
    }
}

/// Create a repo with main (3 commits) and feature (2 commits diverging from commit 2).
/// Non-conflicting changes to different files.
pub fn setup_branched_history(dir: &Path) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    // 3 commits on main
    for i in 0..3 {
        let filename = format!("main_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("main content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        git_with_date(dir, &["commit", "-m", &format!("main commit {}", i)], &date);
    }

    // Branch from commit 2 (HEAD~1)
    git(dir, &["checkout", "-b", "feature", "HEAD~1"]);

    // 2 divergent commits on feature (different files than main commit 2)
    for i in 0..2 {
        let filename = format!("feature_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("feature content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        git_with_date(
            dir,
            &["commit", "-m", &format!("feature commit {}", i)],
            &date,
        );
    }

    // Return to main
    git(dir, &["checkout", "main"]);
}

/// Create a merge conflict scenario: both branches modify the same lines of `conflict.txt`.
pub fn setup_merge_conflict(dir: &Path) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    // Initial commit with conflict.txt
    std::fs::write(dir.join("conflict.txt"), "line 1\nline 2\nline 3\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "conflict.txt"], &date);
    git_with_date(dir, &["commit", "-m", "initial"], &date);

    // Main branch modifies conflict.txt
    std::fs::write(dir.join("conflict.txt"), "line 1\nmain change\nline 3\n").unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "conflict.txt"], &date);
    git_with_date(dir, &["commit", "-m", "main change"], &date);

    // Create feature branch from initial commit
    git(dir, &["checkout", "-b", "feature", "HEAD~1"]);

    // Feature branch modifies same line of conflict.txt
    std::fs::write(
        dir.join("conflict.txt"),
        "line 1\nfeature change\nline 3\n",
    )
    .unwrap();
    let date = next_date(&mut counter);
    git_with_date(dir, &["add", "conflict.txt"], &date);
    git_with_date(dir, &["commit", "-m", "feature change"], &date);

    // Return to main
    git(dir, &["checkout", "main"]);
}

/// Create a bare repository populated with 2 commits via a temp working clone.
pub fn setup_bare_remote(dir: &Path) {
    git(dir, &["init", "--bare", "-b", "main"]);

    // Create a temp working dir to push initial commits (unique per-call)
    let work_tmp = tempfile::tempdir().unwrap();
    let work_dir = work_tmp.path();

    let url = format!("file://{}", dir.display());
    git(work_dir, &["clone", &url, "."]);
    git(work_dir, &["config", "user.name", "Test Author"]);
    git(work_dir, &["config", "user.email", "test@example.com"]);

    let mut counter = 0u64;
    for i in 0..2 {
        let filename = format!("file_{}.txt", i);
        std::fs::write(work_dir.join(&filename), format!("content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(work_dir, &["add", &filename], &date);
        git_with_date(work_dir, &["commit", "-m", &format!("commit {}", i)], &date);
    }
    git(work_dir, &["push", "origin", "main"]);
}

/// Create a repo with a binary file (PNG-like header + known bytes).
pub fn setup_binary_files(dir: &Path) {
    setup_empty_repo(dir);

    // PNG-like header + 256 deterministic bytes
    let mut data = vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    for i in 0..256u16 {
        data.push((i & 0xFF) as u8);
    }
    std::fs::write(dir.join("image.bin"), &data).unwrap();
    git(dir, &["add", "image.bin"]);
    git(dir, &["commit", "-m", "add binary file"]);
}

/// Create a repo with files using unicode characters in filenames.
pub fn setup_unicode_paths(dir: &Path) {
    setup_empty_repo(dir);

    std::fs::write(dir.join("café.txt"), "coffee\n").unwrap();
    std::fs::write(dir.join("naïve.txt"), "naive\n").unwrap();
    std::fs::write(dir.join("日本語.txt"), "japanese\n").unwrap();

    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "add unicode files"]);
}

/// Create a repo with 10 levels of nested directories.
pub fn setup_nested_dirs(dir: &Path) {
    setup_empty_repo(dir);

    let nested = dir.join("a/b/c/d/e/f/g/h/i/j");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("file.txt"), "deeply nested\n").unwrap();

    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "add nested dirs"]);
}

/// Create a committed repo with untracked, ignored, and tracked files for `clean` tests.
///
/// - 2 tracked files (tracked_a.txt, tracked_b.txt)
/// - 2 untracked files (untracked_a.txt, untracked_b.txt)
/// - 1 untracked directory with a file (untracked_dir/file.txt)
/// - .gitignore ignoring *.log, plus an ignored file (ignored.log) on disk
pub fn setup_untracked_files(dir: &Path) {
    setup_empty_repo(dir);

    // Tracked files
    std::fs::write(dir.join("tracked_a.txt"), "tracked a\n").unwrap();
    std::fs::write(dir.join("tracked_b.txt"), "tracked b\n").unwrap();
    std::fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "initial commit"]);

    // Untracked files (created after commit)
    std::fs::write(dir.join("untracked_a.txt"), "untracked a\n").unwrap();
    std::fs::write(dir.join("untracked_b.txt"), "untracked b\n").unwrap();
    std::fs::create_dir_all(dir.join("untracked_dir")).unwrap();
    std::fs::write(dir.join("untracked_dir/file.txt"), "untracked dir file\n").unwrap();

    // Ignored file on disk
    std::fs::write(dir.join("ignored.log"), "ignored content\n").unwrap();
}

/// Create a repo at `dir` with a submodule whose bare remote is at `sub_dir`.
///
/// `sub_dir` gets a bare repo with 2 commits. `dir` gets the submodule added under `sub/`.
pub fn setup_submodule_repo(dir: &Path, sub_dir: &Path) {
    // Create bare remote for the submodule
    setup_bare_remote(sub_dir);

    // Create main repo
    setup_empty_repo(dir);
    std::fs::write(dir.join("main_file.txt"), "main repo content\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "initial main commit"]);

    // Add submodule
    let sub_url = format!("file://{}", sub_dir.display());
    git(dir, &["submodule", "add", &sub_url, "sub"]);
    git(dir, &["commit", "-m", "add submodule"]);
}

/// Create a repo with the specified number of sequential commits, branches, and files.
///
/// - `commits`: number of sequential commits on main (each adds/modifies a unique file)
/// - `branches`: number of branches forked at even intervals from main history
/// - `files`: number of files created in the initial commit (spread across nested dirs)
pub fn setup_large_repo(dir: &Path, commits: usize, branches: usize, files: usize) {
    setup_empty_repo(dir);
    let mut counter = 0u64;

    // Create initial set of files (spread across directories)
    if files > 0 {
        for i in 0..files {
            let subdir = format!("dir_{}", i % 10);
            std::fs::create_dir_all(dir.join(&subdir)).unwrap();
            let filename = format!("{}/file_{}.txt", subdir, i);
            std::fs::write(dir.join(&filename), format!("content {}\n", i)).unwrap();
        }
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", "."], &date);
        git_with_date(dir, &["commit", "-m", "initial files"], &date);
    }

    // Create sequential commits
    let start = if files > 0 { 1 } else { 0 };
    for i in start..commits {
        let filename = format!("commit_file_{}.txt", i);
        std::fs::write(dir.join(&filename), format!("commit content {}\n", i)).unwrap();
        let date = next_date(&mut counter);
        git_with_date(dir, &["add", &filename], &date);
        git_with_date(dir, &["commit", "-m", &format!("commit {}", i)], &date);
    }

    // Create branches at evenly-spaced points in history
    if branches > 0 && commits > 0 {
        let interval = std::cmp::max(1, commits / branches);
        for b in 0..branches {
            let offset = std::cmp::min(b * interval, commits.saturating_sub(1));
            let rev = format!("HEAD~{}", commits.saturating_sub(1) - offset);
            git(dir, &["branch", &format!("branch-{}", b), &rev]);
        }
    }
}

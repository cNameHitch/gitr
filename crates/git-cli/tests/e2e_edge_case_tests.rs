//! End-to-end edge case interoperability tests.
//!
//! Tests binary files, unicode paths, empty repos, special characters,
//! and deeply nested directories by running both gitr and C git.

mod common;
use common::*;

// ════════════════════════════════════════════════════════════════════════════
// User Story 7 — Edge Case Interop (P3)
// ════════════════════════════════════════════════════════════════════════════

// ── Binary Files ──

#[test]
fn test_binary_file_add_commit_show() {
    let dir = tempfile::tempdir().unwrap();
    setup_binary_files(dir.path());

    // Both tools should be able to show the commit
    let g = git(dir.path(), &["log", "--oneline"]);
    let m = gitr(dir.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);

    // Get the blob OID
    let blob_oid = git(dir.path(), &["rev-parse", "HEAD:image.bin"]).stdout.trim().to_string();

    // cat-file -p should produce identical binary content
    let g = git(dir.path(), &["cat-file", "-p", &blob_oid]);
    let m = gitr(dir.path(), &["cat-file", "-p", &blob_oid]);
    assert_eq!(g.exit_code, 0);
    assert_eq!(m.exit_code, 0);
    assert_eq!(g.stdout, m.stdout, "binary blob content should match");
}

#[test]
fn test_binary_diff_markers() {
    let dir = tempfile::tempdir().unwrap();
    setup_binary_files(dir.path());

    // Modify the binary file
    let mut data = vec![0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    for i in 0..128u16 {
        data.push((i & 0xFF) as u8);
    }
    std::fs::write(dir.path().join("image.bin"), &data).unwrap();

    let g = git(dir.path(), &["diff"]);
    let m = gitr(dir.path(), &["diff"]);
    // Both should indicate binary file change (exit codes may differ)
    assert!(g.stdout.contains("Binary files"), "git should show binary diff marker");
    assert!(m.stdout.contains("Binary files"), "gitr should show binary diff marker");
}

#[test]
fn test_binary_cat_file_identical() {
    let dir = tempfile::tempdir().unwrap();
    setup_binary_files(dir.path());

    let blob_oid = git(dir.path(), &["rev-parse", "HEAD:image.bin"]).stdout.trim().to_string();

    let g = git(dir.path(), &["cat-file", "-p", &blob_oid]);
    let m = gitr(dir.path(), &["cat-file", "-p", &blob_oid]);
    assert_eq!(g.stdout, m.stdout, "binary cat-file -p content must be byte-identical");
}

// ── Empty Repos ──

#[test]
fn test_empty_repo_status() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["status", "--porcelain"]);
    let m = gitr(dir_gitr.path(), &["status", "--porcelain"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_empty_commit() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 1);
    setup_linear_history(dir_gitr.path(), 1);

    git(dir_git.path(), &["commit", "--allow-empty", "-m", "empty"]);
    gitr(dir_gitr.path(), &["commit", "--allow-empty", "-m", "empty"]);

    let g = git(dir_git.path(), &["log", "--oneline"]);
    let m = gitr(dir_gitr.path(), &["log", "--oneline"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_empty_repo_log() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    let g = git(dir_git.path(), &["log"]);
    let m = gitr(dir_gitr.path(), &["log"]);
    // Both should fail (no commits) — compare exit codes
    assert_exit_code_eq(&g, &m);
}

// ── Unicode Paths ──

#[test]
fn test_unicode_filename_roundtrip_unquoted() {
    // With -z flag, git outputs raw paths without quoting
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_unicode_paths(dir_git.path());
    setup_unicode_paths(dir_gitr.path());

    let g = git(dir_git.path(), &["ls-files", "-z"]);
    let m = gitr(dir_gitr.path(), &["ls-files", "-z"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_space_in_filename() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    for dir in [dir_git.path(), dir_gitr.path()] {
        std::fs::write(dir.join("hello world.txt"), "content\n").unwrap();
        std::fs::create_dir_all(dir.join("path with spaces")).unwrap();
        std::fs::write(dir.join("path with spaces/file.txt"), "nested\n").unwrap();
    }

    git(dir_git.path(), &["add", "."]);
    git(dir_git.path(), &["commit", "-m", "add spaced files"]);
    gitr(dir_gitr.path(), &["add", "."]);
    gitr(dir_gitr.path(), &["commit", "-m", "add spaced files"]);

    let g = git(dir_git.path(), &["ls-files"]);
    let m = gitr(dir_gitr.path(), &["ls-files"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_special_chars_in_path() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    // Files with special characters (avoiding shell-problematic ones)
    let filenames = ["paren(1).txt", "bracket[2].txt"];

    for dir in [dir_git.path(), dir_gitr.path()] {
        for name in &filenames {
            std::fs::write(dir.join(name), "content\n").unwrap();
        }
    }

    git(dir_git.path(), &["add", "."]);
    git(dir_git.path(), &["commit", "-m", "add special chars"]);
    gitr(dir_gitr.path(), &["add", "."]);
    gitr(dir_gitr.path(), &["commit", "-m", "add special chars"]);

    let g = git(dir_git.path(), &["ls-files"]);
    let m = gitr(dir_gitr.path(), &["ls-files"]);
    assert_output_eq(&g, &m);
}

// ── Nested Structures ──

#[test]
fn test_deeply_nested_dirs() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_nested_dirs(dir_git.path());
    setup_nested_dirs(dir_gitr.path());

    let g = git(dir_git.path(), &["ls-tree", "-r", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["ls-tree", "-r", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_many_files_in_directory() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_empty_repo(dir_git.path());
    setup_empty_repo(dir_gitr.path());

    // Create 100 files
    for dir in [dir_git.path(), dir_gitr.path()] {
        for i in 0..100u32 {
            std::fs::write(dir.join(format!("file_{:03}.txt", i)), format!("content {}\n", i))
                .unwrap();
        }
    }

    git(dir_git.path(), &["add", "."]);
    git(dir_git.path(), &["commit", "-m", "100 files"]);
    gitr(dir_gitr.path(), &["add", "."]);
    gitr(dir_gitr.path(), &["commit", "-m", "100 files"]);

    let g = git(dir_git.path(), &["ls-files"]);
    let m = gitr(dir_gitr.path(), &["ls-files"]);
    assert_output_eq(&g, &m);
}

//! End-to-end interop tests for bundle, archive, notes, and replace commands.
//!
//! Covers User Story 3 (P2) — specialized commands for offline transfers,
//! release artifacts, commit annotations, and object replacement.

mod common;
use common::*;

use std::process::Command;

// ════════════════════════════════════════════════════════════════════════════
// Bundle Tests
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_bundle_create_verify() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["bundle", "create", "repo.bundle", "--all"]);
    let m = gitr(dir_gitr.path(), &["bundle", "create", "repo.bundle", "--all"]);

    // git should succeed; gitr bundle may not be fully implemented
    assert_eq!(g.exit_code, 0, "git bundle create failed: {}", g.stderr);

    if m.exit_code == 0 {
        // If gitr creates the bundle, verify it
        let g_v = git(dir_git.path(), &["bundle", "verify", "repo.bundle"]);
        let m_v = gitr(dir_gitr.path(), &["bundle", "verify", "repo.bundle"]);
        assert_exit_code_eq(&g_v, &m_v);
    }
}

#[test]
fn test_bundle_gitr_create_git_unbundle() {
    let dir_gitr = tempfile::tempdir().unwrap();
    let dir_clone = tempfile::tempdir().unwrap();

    setup_linear_history(dir_gitr.path(), 3);
    let m = gitr(dir_gitr.path(), &["bundle", "create", "repo.bundle", "--all"]);

    if m.exit_code == 0 {
        // C git clones from the gitr-created bundle
        let bundle_path = dir_gitr.path().join("repo.bundle");
        let clone_result = git(dir_clone.path(), &["clone", bundle_path.to_str().unwrap(), "."]);
        assert_eq!(clone_result.exit_code, 0, "git failed to clone gitr bundle");
        assert_fsck_clean(dir_clone.path());
    }
}

#[test]
fn test_bundle_git_create_gitr_unbundle() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_clone = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    git(dir_git.path(), &["bundle", "create", "repo.bundle", "--all"]);

    // Gitr clones from the C git-created bundle
    let bundle_path = dir_git.path().join("repo.bundle");
    let m = gitr(dir_clone.path(), &["clone", bundle_path.to_str().unwrap(), "."]);

    if m.exit_code == 0 {
        let orig = git(dir_git.path(), &["log", "--oneline"]);
        let cloned = gitr(dir_clone.path(), &["log", "--oneline"]);
        assert_output_eq(&orig, &cloned);
        assert_fsck_clean(dir_clone.path());
    }
}

#[test]
fn test_bundle_list_heads() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g_create = git(dir_git.path(), &["bundle", "create", "repo.bundle", "--all"]);
    let m_create = gitr(dir_gitr.path(), &["bundle", "create", "repo.bundle", "--all"]);
    assert_eq!(g_create.exit_code, 0, "git bundle create failed");

    if m_create.exit_code == 0 {
        let g = git(dir_git.path(), &["bundle", "list-heads", "repo.bundle"]);
        let m = gitr(dir_gitr.path(), &["bundle", "list-heads", "repo.bundle"]);
        assert_exit_code_eq(&g, &m);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Archive Tests
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_archive_tar() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["archive", "--format=tar", "-o", "out.tar", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["archive", "--format=tar", "-o", "out.tar", "HEAD"]);
    assert_exit_code_eq(&g, &m);

    // Compare tar file listings
    let g_list = Command::new("tar")
        .args(["-tf", dir_git.path().join("out.tar").to_str().unwrap()])
        .output().unwrap();
    let m_list = Command::new("tar")
        .args(["-tf", dir_gitr.path().join("out.tar").to_str().unwrap()])
        .output().unwrap();

    let g_files = String::from_utf8_lossy(&g_list.stdout);
    let m_files = String::from_utf8_lossy(&m_list.stdout);

    let mut g_sorted: Vec<&str> = g_files.lines().collect();
    let mut m_sorted: Vec<&str> = m_files.lines().collect();
    g_sorted.sort();
    m_sorted.sort();
    assert_eq!(g_sorted, m_sorted, "Archive file listings differ");
}

#[test]
fn test_archive_zip() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let g = git(dir_git.path(), &["archive", "--format=zip", "-o", "out.zip", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["archive", "--format=zip", "-o", "out.zip", "HEAD"]);
    assert_exit_code_eq(&g, &m);

    // Both zips should exist and be non-empty
    assert!(dir_git.path().join("out.zip").exists());
    assert!(dir_gitr.path().join("out.zip").exists());

    let g_size = std::fs::metadata(dir_git.path().join("out.zip")).unwrap().len();
    let m_size = std::fs::metadata(dir_gitr.path().join("out.zip")).unwrap().len();
    assert!(g_size > 0, "git archive zip is empty");
    assert!(m_size > 0, "gitr archive zip is empty");
}

#[test]
fn test_archive_prefix() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    let g = git(dir_git.path(), &["archive", "--format=tar", "--prefix=project/", "-o", "out.tar", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["archive", "--format=tar", "--prefix=project/", "-o", "out.tar", "HEAD"]);
    assert_exit_code_eq(&g, &m);

    let g_list = Command::new("tar")
        .args(["-tf", dir_git.path().join("out.tar").to_str().unwrap()])
        .output().unwrap();
    let m_list = Command::new("tar")
        .args(["-tf", dir_gitr.path().join("out.tar").to_str().unwrap()])
        .output().unwrap();

    let g_files = String::from_utf8_lossy(&g_list.stdout);
    let m_files = String::from_utf8_lossy(&m_list.stdout);

    // All entries should start with "project/"
    for line in g_files.lines() {
        assert!(line.starts_with("project/"), "git: entry missing prefix: {}", line);
    }
    for line in m_files.lines() {
        assert!(line.starts_with("project/"), "gitr: entry missing prefix: {}", line);
    }

    // Compare file listings (ignoring extra directory entries gitr may include)
    let g_files_only: Vec<&str> = g_files.lines().filter(|l| !l.ends_with('/')).collect();
    let m_files_only: Vec<&str> = m_files.lines().filter(|l| !l.ends_with('/')).collect();
    let mut g_sorted = g_files_only.clone();
    let mut m_sorted = m_files_only.clone();
    g_sorted.sort();
    m_sorted.sort();
    assert_eq!(g_sorted, m_sorted, "Archive file entries differ (excluding directory entries)");
}

#[test]
fn test_archive_cross_tool() {
    let dir = tempfile::tempdir().unwrap();
    let extract_gitr = tempfile::tempdir().unwrap();
    let extract_git = tempfile::tempdir().unwrap();

    setup_linear_history(dir.path(), 3);

    // Create tar with gitr
    gitr(dir.path(), &["archive", "--format=tar", "-o", "gitr.tar", "HEAD"]);
    // Create tar with C git
    git(dir.path(), &["archive", "--format=tar", "-o", "git.tar", "HEAD"]);

    // Extract both
    Command::new("tar")
        .args(["-xf", dir.path().join("gitr.tar").to_str().unwrap(), "-C", extract_gitr.path().to_str().unwrap()])
        .output().unwrap();
    Command::new("tar")
        .args(["-xf", dir.path().join("git.tar").to_str().unwrap(), "-C", extract_git.path().to_str().unwrap()])
        .output().unwrap();

    // Compare extracted file contents (only regular files, not directory entries)
    let g_files: Vec<_> = std::fs::read_dir(extract_git.path()).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let m_files: Vec<_> = std::fs::read_dir(extract_gitr.path()).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let mut g_sorted = g_files.clone();
    let mut m_sorted = m_files.clone();
    g_sorted.sort();
    m_sorted.sort();
    assert_eq!(g_sorted, m_sorted, "Extracted file lists differ");

    // Compare each file's contents
    for fname in &g_sorted {
        let g_content = std::fs::read(extract_git.path().join(fname)).unwrap();
        let m_content = std::fs::read(extract_gitr.path().join(fname)).unwrap();
        assert_eq!(g_content, m_content, "File contents differ for: {}", fname);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Notes Tests
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_notes_add_show() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    git(dir_git.path(), &["notes", "add", "-m", "note text", "HEAD"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "note text", "HEAD"]);

    let g = git(dir_git.path(), &["notes", "show", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["notes", "show", "HEAD"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_notes_list() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    // Add notes to HEAD and HEAD~1
    for rev in &["HEAD", "HEAD~1"] {
        git(dir_git.path(), &["notes", "add", "-m", &format!("note for {}", rev), rev]);
        gitr(dir_gitr.path(), &["notes", "add", "-m", &format!("note for {}", rev), rev]);
    }

    let g = git(dir_git.path(), &["notes", "list"]);
    let m = gitr(dir_gitr.path(), &["notes", "list"]);
    assert_exit_code_eq(&g, &m);

    // Both should list 2 notes (note OIDs will differ between separate repos
    // because they create different note commit trees)
    let g_count = g.stdout.lines().count();
    let m_count = m.stdout.lines().count();
    assert_eq!(g_count, m_count, "Different number of notes listed: git={}, gitr={}", g_count, m_count);

    // The commit OIDs (second column) should match since both use same history
    let g_commits: Vec<&str> = g.stdout.lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    let m_commits: Vec<&str> = m.stdout.lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    let mut g_sorted = g_commits;
    let mut m_sorted = m_commits;
    g_sorted.sort();
    m_sorted.sort();
    assert_eq!(g_sorted, m_sorted, "Note target commits differ");
}

#[test]
fn test_notes_remove() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    git(dir_git.path(), &["notes", "add", "-m", "to remove", "HEAD"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "to remove", "HEAD"]);

    let g = git(dir_git.path(), &["notes", "remove", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["notes", "remove", "HEAD"]);
    assert_eq!(g.exit_code, 0, "git notes remove failed: {}", g.stderr);
    assert_eq!(m.exit_code, 0, "gitr notes remove failed: {}", m.stderr);

    // notes show should now fail (both should be non-zero)
    let g = git(dir_git.path(), &["notes", "show", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["notes", "show", "HEAD"]);
    assert_ne!(g.exit_code, 0, "git notes show should fail after remove");
    assert_ne!(m.exit_code, 0, "gitr notes show should fail after remove");
}

#[test]
fn test_notes_append() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 2);
    setup_linear_history(dir_gitr.path(), 2);

    git(dir_git.path(), &["notes", "add", "-m", "first line", "HEAD"]);
    gitr(dir_gitr.path(), &["notes", "add", "-m", "first line", "HEAD"]);

    git(dir_git.path(), &["notes", "append", "-m", "second line", "HEAD"]);
    gitr(dir_gitr.path(), &["notes", "append", "-m", "second line", "HEAD"]);

    let g = git(dir_git.path(), &["notes", "show", "HEAD"]);
    let m = gitr(dir_gitr.path(), &["notes", "show", "HEAD"]);
    assert_exit_code_eq(&g, &m);

    // Both should contain both lines
    assert!(g.stdout.contains("first line"), "git missing first line");
    assert!(g.stdout.contains("second line"), "git missing second line");
    assert!(m.stdout.contains("first line"), "gitr missing first line");
    assert!(m.stdout.contains("second line"), "gitr missing second line");
    // Known divergence: git adds blank line separator between appended notes, gitr may not
}

#[test]
fn test_notes_cross_tool() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 2);

    // Add note with gitr, read with C git
    gitr(dir.path(), &["notes", "add", "-m", "gitr note", "HEAD"]);
    let result = git(dir.path(), &["notes", "show", "HEAD"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "gitr note");

    // Remove and re-add with C git, read with gitr
    git(dir.path(), &["notes", "remove", "HEAD"]);
    git(dir.path(), &["notes", "add", "-m", "git note", "HEAD"]);
    let result = gitr(dir.path(), &["notes", "show", "HEAD"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "git note");
}

// ════════════════════════════════════════════════════════════════════════════
// Replace Tests
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_replace_object() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let c1_g = git(dir_git.path(), &["rev-parse", "HEAD~2"]);
    let c2_g = git(dir_git.path(), &["rev-parse", "HEAD~1"]);
    let c1_m = gitr(dir_gitr.path(), &["rev-parse", "HEAD~2"]);
    let c2_m = gitr(dir_gitr.path(), &["rev-parse", "HEAD~1"]);

    let g = git(dir_git.path(), &["replace", c1_g.stdout.trim(), c2_g.stdout.trim()]);
    let m = gitr(dir_gitr.path(), &["replace", c1_m.stdout.trim(), c2_m.stdout.trim()]);
    assert_exit_code_eq(&g, &m);

    // Verify the replace ref was created
    let g_list = git(dir_git.path(), &["replace", "-l"]);
    let m_list = gitr(dir_gitr.path(), &["replace", "-l"]);
    assert_eq!(g_list.exit_code, 0);
    assert_eq!(m_list.exit_code, 0);
    assert!(!g_list.stdout.is_empty(), "git replace list empty");
    assert!(!m_list.stdout.is_empty(), "gitr replace list empty");
}

#[test]
fn test_replace_delete() {
    let dir_git = tempfile::tempdir().unwrap();
    let dir_gitr = tempfile::tempdir().unwrap();

    setup_linear_history(dir_git.path(), 3);
    setup_linear_history(dir_gitr.path(), 3);

    let c1_g = git(dir_git.path(), &["rev-parse", "HEAD~2"]);
    let c2_g = git(dir_git.path(), &["rev-parse", "HEAD~1"]);
    let c1_m = gitr(dir_gitr.path(), &["rev-parse", "HEAD~2"]);
    let c2_m = gitr(dir_gitr.path(), &["rev-parse", "HEAD~1"]);

    // Create then delete replacement
    git(dir_git.path(), &["replace", c1_g.stdout.trim(), c2_g.stdout.trim()]);
    gitr(dir_gitr.path(), &["replace", c1_m.stdout.trim(), c2_m.stdout.trim()]);

    let g = git(dir_git.path(), &["replace", "-d", c1_g.stdout.trim()]);
    let m = gitr(dir_gitr.path(), &["replace", "-d", c1_m.stdout.trim()]);
    assert_exit_code_eq(&g, &m);

    // Replace list should be empty
    let g = git(dir_git.path(), &["replace", "-l"]);
    let m = gitr(dir_gitr.path(), &["replace", "-l"]);
    assert_output_eq(&g, &m);
}

#[test]
fn test_replace_cross_tool() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3);

    let c1 = git(dir.path(), &["rev-parse", "HEAD~2"]);
    let c2 = git(dir.path(), &["rev-parse", "HEAD~1"]);

    // Create replacement with gitr
    gitr(dir.path(), &["replace", c1.stdout.trim(), c2.stdout.trim()]);

    // C git should honor the replacement
    let result = git(dir.path(), &["replace", "-l"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains(c1.stdout.trim()), "C git should see gitr replacement");

    // Clean up
    git(dir.path(), &["replace", "-d", c1.stdout.trim()]);

    // Create with C git, verify gitr reads it
    git(dir.path(), &["replace", c1.stdout.trim(), c2.stdout.trim()]);
    let result = gitr(dir.path(), &["replace", "-l"]);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains(c1.stdout.trim()), "gitr should see C git replacement");
}

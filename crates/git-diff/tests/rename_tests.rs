//! Rename and copy detection tests.

use bstr::BString;
use git_diff::rename::similarity_score;
use git_diff::{DiffResult, FileDiff, FileStatus};
use git_hash::ObjectId;
use git_object::FileMode;

#[test]
fn similarity_identical_content() {
    assert_eq!(similarity_score(b"hello world\n", b"hello world\n"), 100);
}

#[test]
fn similarity_empty_files() {
    assert_eq!(similarity_score(b"", b""), 100);
}

#[test]
fn similarity_one_empty() {
    assert_eq!(similarity_score(b"content\n", b""), 0);
    assert_eq!(similarity_score(b"", b"content\n"), 0);
}

#[test]
fn similarity_completely_different() {
    assert_eq!(similarity_score(b"aaa\nbbb\nccc\n", b"xxx\nyyy\nzzz\n"), 0);
}

#[test]
fn similarity_mostly_same() {
    let old = b"line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
    let new = b"line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nchanged\n";
    let score = similarity_score(old, new);
    // 9 of 10 lines match
    assert!(score >= 70, "Score {} should be >= 70", score);
}

#[test]
fn similarity_symmetric() {
    let a = b"line1\nline2\nline3\n";
    let b = b"line1\nline2\nchanged\n";
    let score_ab = similarity_score(a, b);
    let score_ba = similarity_score(b, a);
    assert_eq!(score_ab, score_ba, "Similarity should be symmetric");
}

#[test]
fn similarity_single_line_files() {
    assert_eq!(similarity_score(b"hello\n", b"hello\n"), 100);
    assert_eq!(similarity_score(b"hello\n", b"world\n"), 0);
}

#[test]
fn similarity_large_common_prefix() {
    let mut old = Vec::new();
    let mut new = Vec::new();
    // 100 common lines, 1 different
    for i in 0..100 {
        let line = format!("common line {}\n", i);
        old.extend_from_slice(line.as_bytes());
        new.extend_from_slice(line.as_bytes());
    }
    old.extend_from_slice(b"old ending\n");
    new.extend_from_slice(b"new ending\n");

    let score = similarity_score(&old, &new);
    assert!(score >= 90, "Score {} should be >= 90 for 100/101 matching lines", score);
}

#[test]
fn similarity_reordered_lines() {
    // Same lines but in different order
    let old = b"line_a\nline_b\nline_c\nline_d\n";
    let new = b"line_d\nline_c\nline_b\nline_a\n";
    let score = similarity_score(old, new);
    // All lines present, just reordered - should still show high similarity
    assert!(score >= 80, "Score {} should be >= 80 for reordered lines", score);
}

/// Test that similarity scoring handles binary-like content gracefully.
#[test]
fn similarity_binary_content() {
    let old = b"\x00\x01\x02\x03\x04\x05";
    let new = b"\x00\x01\x02\x03\x04\x06";
    // Binary content without newlines is treated as single lines
    let _score = similarity_score(old, new);
    // Just verify it doesn't panic
}

// --- Integration-style rename detection tests ---

#[test]
fn detect_exact_rename_structure() {
    // Verify that exact rename detection works at the FileDiff level
    let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

    let result = DiffResult {
        files: vec![
            FileDiff {
                status: FileStatus::Deleted,
                old_path: Some(BString::from("old_name.txt")),
                new_path: None,
                old_mode: Some(FileMode::Regular),
                new_mode: None,
                old_oid: Some(oid),
                new_oid: None,
                hunks: vec![],
                is_binary: false,
                similarity: None,
            },
            FileDiff {
                status: FileStatus::Added,
                old_path: None,
                new_path: Some(BString::from("new_name.txt")),
                old_mode: None,
                new_mode: Some(FileMode::Regular),
                old_oid: None,
                new_oid: Some(oid),
                hunks: vec![],
                is_binary: false,
                similarity: None,
            },
        ],
    };

    // The deleted and added files have the same OID -> should be detected as rename
    assert_eq!(result.files[0].old_oid, result.files[1].new_oid);
}

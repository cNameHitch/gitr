//! Comprehensive algorithm correctness tests.
//!
//! Tests all three diff algorithms (Myers, Histogram, Patience)
//! produce correct results and that the generated hunks are valid.

use git_diff::algorithm::{diff_edits, diff_lines, EditOp};
use git_diff::DiffAlgorithm;

/// Helper to verify edit script correctness: applying the edits to old should produce new.
fn verify_edit_script(old: &[u8], new: &[u8], algorithm: DiffAlgorithm) {
    let old_lines: Vec<&[u8]> = git_diff::algorithm::split_lines(old);
    let new_lines: Vec<&[u8]> = git_diff::algorithm::split_lines(new);
    let edits = diff_edits(old, new, algorithm);

    // Reconstruct new from old + edits
    let mut reconstructed: Vec<&[u8]> = Vec::new();
    for edit in &edits {
        match edit.op {
            EditOp::Equal => {
                reconstructed.push(old_lines[edit.old_index]);
            }
            EditOp::Insert => {
                reconstructed.push(new_lines[edit.new_index]);
            }
            EditOp::Delete => {
                // Skip this old line
            }
        }
    }
    assert_eq!(
        reconstructed, new_lines,
        "Edit script for algorithm {:?} does not reconstruct new from old",
        algorithm
    );
}

/// Test all algorithms on a given input pair.
fn test_all_algorithms(old: &[u8], new: &[u8]) {
    for algo in [
        DiffAlgorithm::Myers,
        DiffAlgorithm::Minimal,
        DiffAlgorithm::Histogram,
        DiffAlgorithm::Patience,
    ] {
        verify_edit_script(old, new, algo);
    }
}

#[test]
fn empty_to_empty() {
    test_all_algorithms(b"", b"");
}

#[test]
fn empty_to_content() {
    test_all_algorithms(b"", b"hello\nworld\n");
}

#[test]
fn content_to_empty() {
    test_all_algorithms(b"hello\nworld\n", b"");
}

#[test]
fn identical_content() {
    let content = b"line1\nline2\nline3\n";
    test_all_algorithms(content, content);
}

#[test]
fn single_line_change() {
    test_all_algorithms(b"hello\n", b"world\n");
}

#[test]
fn insert_at_beginning() {
    test_all_algorithms(b"b\nc\n", b"a\nb\nc\n");
}

#[test]
fn insert_at_end() {
    test_all_algorithms(b"a\nb\n", b"a\nb\nc\n");
}

#[test]
fn insert_in_middle() {
    test_all_algorithms(b"a\nc\n", b"a\nb\nc\n");
}

#[test]
fn delete_from_beginning() {
    test_all_algorithms(b"a\nb\nc\n", b"b\nc\n");
}

#[test]
fn delete_from_end() {
    test_all_algorithms(b"a\nb\nc\n", b"a\nb\n");
}

#[test]
fn delete_from_middle() {
    test_all_algorithms(b"a\nb\nc\n", b"a\nc\n");
}

#[test]
fn replace_single_line() {
    test_all_algorithms(b"a\nb\nc\n", b"a\nx\nc\n");
}

#[test]
fn multiple_changes() {
    test_all_algorithms(
        b"a\nb\nc\nd\ne\n",
        b"a\nB\nc\nD\ne\n",
    );
}

#[test]
fn completely_different() {
    test_all_algorithms(b"a\nb\nc\n", b"x\ny\nz\n");
}

#[test]
fn no_trailing_newline_old() {
    test_all_algorithms(b"hello", b"hello\n");
}

#[test]
fn no_trailing_newline_new() {
    test_all_algorithms(b"hello\n", b"hello");
}

#[test]
fn no_trailing_newline_both() {
    test_all_algorithms(b"hello", b"world");
}

#[test]
fn duplicate_lines() {
    test_all_algorithms(
        b"a\na\na\nb\nb\nb\n",
        b"a\na\nc\nb\nb\n",
    );
}

#[test]
fn large_insert() {
    let mut old = Vec::new();
    let mut new = Vec::new();
    for i in 0..10 {
        old.extend_from_slice(format!("line{}\n", i).as_bytes());
    }
    for i in 0..10 {
        new.extend_from_slice(format!("line{}\n", i).as_bytes());
        if i == 5 {
            for j in 0..20 {
                new.extend_from_slice(format!("inserted{}\n", j).as_bytes());
            }
        }
    }
    test_all_algorithms(&old, &new);
}

#[test]
fn large_delete() {
    let mut old = Vec::new();
    let mut new = Vec::new();
    for i in 0..30 {
        old.extend_from_slice(format!("line{}\n", i).as_bytes());
    }
    for i in 0..30 {
        if !(10..20).contains(&i) {
            new.extend_from_slice(format!("line{}\n", i).as_bytes());
        }
    }
    test_all_algorithms(&old, &new);
}

// --- Hunk generation tests ---

#[test]
fn hunks_simple_change() {
    let old = b"a\nb\nc\nd\ne\n";
    let new = b"a\nb\nX\nd\ne\n";
    let hunks = diff_lines(old, new, DiffAlgorithm::Myers, 3);
    assert_eq!(hunks.len(), 1, "Expected 1 hunk");
    let hunk = &hunks[0];
    assert!(hunk.old_count > 0);
    assert!(hunk.new_count > 0);
}

#[test]
fn hunks_no_changes() {
    let content = b"a\nb\nc\n";
    let hunks = diff_lines(content, content, DiffAlgorithm::Myers, 3);
    assert!(hunks.is_empty(), "Identical content should produce no hunks");
}

#[test]
fn hunks_all_new() {
    let hunks = diff_lines(b"", b"a\nb\nc\n", DiffAlgorithm::Myers, 3);
    assert_eq!(hunks.len(), 1);
    // All lines should be additions
    for line in &hunks[0].lines {
        assert!(
            matches!(line, git_diff::DiffLine::Addition(_)),
            "Expected all additions"
        );
    }
}

#[test]
fn hunks_all_deleted() {
    let hunks = diff_lines(b"a\nb\nc\n", b"", DiffAlgorithm::Myers, 3);
    assert_eq!(hunks.len(), 1);
    for line in &hunks[0].lines {
        assert!(
            matches!(line, git_diff::DiffLine::Deletion(_)),
            "Expected all deletions"
        );
    }
}

#[test]
fn hunks_separated_changes_merged() {
    // Two changes close together should be merged into one hunk with context=3
    let old = b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
    let new = b"1\n2\nX\n4\n5\nY\n7\n8\n9\n10\n";
    let hunks = diff_lines(old, new, DiffAlgorithm::Myers, 3);
    // Changes at lines 3 and 6 are only 2 lines apart, within 2*3 context
    assert_eq!(hunks.len(), 1, "Close changes should be merged");
}

#[test]
fn hunks_separated_changes_split() {
    // Two changes far apart should produce separate hunks
    let mut old = String::new();
    let mut new = String::new();
    for i in 1..=20 {
        old.push_str(&format!("line{}\n", i));
        if i == 3 {
            new.push_str("CHANGED\n");
        } else if i == 18 {
            new.push_str("ALSO_CHANGED\n");
        } else {
            new.push_str(&format!("line{}\n", i));
        }
    }
    let hunks = diff_lines(old.as_bytes(), new.as_bytes(), DiffAlgorithm::Myers, 3);
    assert_eq!(hunks.len(), 2, "Distant changes should be separate hunks");
}

#[test]
fn context_zero_minimal_hunks() {
    let old = b"a\nb\nc\nd\ne\n";
    let new = b"a\nX\nc\nd\ne\n";
    let hunks = diff_lines(old, new, DiffAlgorithm::Myers, 0);
    assert_eq!(hunks.len(), 1);
    // With 0 context, hunk should only contain the changed lines
    let hunk = &hunks[0];
    let context_count = hunk
        .lines
        .iter()
        .filter(|l| matches!(l, git_diff::DiffLine::Context(_)))
        .count();
    assert_eq!(context_count, 0, "With context=0, no context lines expected");
}

#[test]
fn algorithms_produce_same_edit_count() {
    // For a simple case, all algorithms should produce minimal edits
    let old = b"a\nb\nc\n";
    let new = b"a\nx\nc\n";

    for algo in [
        DiffAlgorithm::Myers,
        DiffAlgorithm::Histogram,
        DiffAlgorithm::Patience,
    ] {
        let edits = diff_edits(old, new, algo);
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        let deletes = edits.iter().filter(|e| e.op == EditOp::Delete).count();
        assert_eq!(inserts, 1, "{:?} should have 1 insert", algo);
        assert_eq!(deletes, 1, "{:?} should have 1 delete", algo);
    }
}

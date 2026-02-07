//! Integration tests for three-way content merge.

use git_merge::content::{merge_content, MergeLabels};
use git_merge::{ConflictStyle, MergeOptions};

fn labels() -> MergeLabels<'static> {
    MergeLabels {
        base: "base",
        ours: "HEAD",
        theirs: "feature",
    }
}

#[test]
fn clean_merge_non_overlapping() {
    let base = b"line1\nline2\nline3\nline4\nline5\n";
    let ours = b"MODIFIED1\nline2\nline3\nline4\nline5\n";
    let theirs = b"line1\nline2\nline3\nline4\nMODIFIED5\n";

    let result = merge_content(base, ours, theirs, &MergeOptions::default(), &labels());
    assert!(result.is_clean());
    let content = String::from_utf8_lossy(result.content());
    assert!(content.contains("MODIFIED1"), "Expected MODIFIED1 in: {}", content);
    assert!(content.contains("MODIFIED5"), "Expected MODIFIED5 in: {}", content);
}

#[test]
fn conflict_same_region() {
    let base = b"a\nb\nc\n";
    let ours = b"a\nX\nc\n";
    let theirs = b"a\nY\nc\n";

    let result = merge_content(base, ours, theirs, &MergeOptions::default(), &labels());
    assert!(!result.is_clean());

    let content = String::from_utf8_lossy(result.content());
    assert!(content.contains("<<<<<<< HEAD"));
    assert!(content.contains("======="));
    assert!(content.contains(">>>>>>> feature"));
}

#[test]
fn diff3_markers_include_base() {
    let base = b"a\noriginal\nc\n";
    let ours = b"a\nours_change\nc\n";
    let theirs = b"a\ntheirs_change\nc\n";

    let mut opts = MergeOptions::default();
    opts.conflict_style = ConflictStyle::Diff3;

    let result = merge_content(base, ours, theirs, &opts, &labels());
    assert!(!result.is_clean());

    let content = String::from_utf8_lossy(result.content());
    assert!(content.contains("||||||| base"));
    assert!(content.contains("original"));
}

#[test]
fn identical_changes_are_clean() {
    let base = b"a\nold\nc\n";
    let ours = b"a\nnew\nc\n";
    let theirs = b"a\nnew\nc\n";

    let result = merge_content(base, ours, theirs, &MergeOptions::default(), &labels());
    assert!(result.is_clean());
    assert_eq!(result.content(), ours);
}

#[test]
fn base_equals_ours_takes_theirs() {
    let base = b"unchanged\n";
    let ours = b"unchanged\n";
    let theirs = b"modified\n";

    let result = merge_content(base, ours, theirs, &MergeOptions::default(), &labels());
    assert!(result.is_clean());
    assert_eq!(result.content(), theirs);
}

#[test]
fn base_equals_theirs_takes_ours() {
    let base = b"unchanged\n";
    let ours = b"modified\n";
    let theirs = b"unchanged\n";

    let result = merge_content(base, ours, theirs, &MergeOptions::default(), &labels());
    assert!(result.is_clean());
    assert_eq!(result.content(), ours);
}

#[test]
fn multiple_conflicts() {
    let base = b"a\nb\nc\nd\ne\n";
    let ours = b"X\nb\nY\nd\ne\n";
    let theirs = b"A\nb\nB\nd\ne\n";

    let result = merge_content(base, ours, theirs, &MergeOptions::default(), &labels());
    assert!(!result.is_clean());

    let content = String::from_utf8_lossy(result.content());
    let conflict_marker_count = content.matches("<<<<<<< HEAD").count();
    assert!(conflict_marker_count >= 2, "Expected 2+ conflicts, got {}", conflict_marker_count);
}

#[test]
fn strategy_option_theirs_resolves_conflicts() {
    let base = b"a\nb\nc\n";
    let ours = b"a\nX\nc\n";
    let theirs = b"a\nY\nc\n";

    let mut opts = MergeOptions::default();
    opts.strategy_options.push("theirs".to_string());

    let result = merge_content(base, ours, theirs, &opts, &labels());
    assert!(result.is_clean());
    assert_eq!(result.content(), theirs);
}

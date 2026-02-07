//! Three-way content merge using diff hunks.
//!
//! Takes base, ours, and theirs versions of a file and produces a merged result,
//! inserting conflict markers where changes overlap.

use crate::{ConflictStyle, ContentMergeResult, MergeOptions};
use git_diff::algorithm::{diff_edits, split_lines, Edit, EditOp};

/// Labels for conflict markers.
#[derive(Debug, Clone)]
pub struct MergeLabels<'a> {
    pub base: &'a str,
    pub ours: &'a str,
    pub theirs: &'a str,
}

impl<'a> Default for MergeLabels<'a> {
    fn default() -> Self {
        Self {
            base: "base",
            ours: "ours",
            theirs: "theirs",
        }
    }
}

/// Perform a three-way content merge.
///
/// Diffs base→ours and base→theirs, then interleaves non-overlapping changes
/// and reports conflicts for overlapping regions.
pub fn merge_content(
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    options: &MergeOptions,
    labels: &MergeLabels<'_>,
) -> ContentMergeResult {
    // If ours == theirs, no merge needed — take either side.
    if ours == theirs {
        return ContentMergeResult::Clean(ours.to_vec());
    }

    // If base == ours, theirs wins cleanly.
    if base == ours {
        return ContentMergeResult::Clean(theirs.to_vec());
    }

    // If base == theirs, ours wins cleanly.
    if base == theirs {
        return ContentMergeResult::Clean(ours.to_vec());
    }

    // Check strategy options for forced resolution.
    let favor_ours = options.strategy_options.iter().any(|o| o == "ours");
    let favor_theirs = options.strategy_options.iter().any(|o| o == "theirs");

    if favor_ours {
        return ContentMergeResult::Clean(ours.to_vec());
    }
    if favor_theirs {
        return ContentMergeResult::Clean(theirs.to_vec());
    }

    // Perform line-level three-way merge.
    let base_lines = split_lines(base);
    let ours_lines = split_lines(ours);
    let theirs_lines = split_lines(theirs);

    let edits_ours = diff_edits(base, ours, options.diff_algorithm);
    let edits_theirs = diff_edits(base, theirs, options.diff_algorithm);

    // Convert edits into change regions relative to the base.
    let hunks_ours = collect_change_regions(&edits_ours);
    let hunks_theirs = collect_change_regions(&edits_theirs);

    merge_regions(
        &base_lines,
        &ours_lines,
        &theirs_lines,
        &hunks_ours,
        &hunks_theirs,
        options.conflict_style,
        labels,
    )
}

/// A contiguous region of changes relative to the base.
#[derive(Debug, Clone)]
struct ChangeRegion {
    /// Start line in base (0-indexed).
    base_start: usize,
    /// Number of lines removed from base.
    base_len: usize,
    /// Start line in the modified file (0-indexed).
    new_start: usize,
    /// Number of lines added.
    new_len: usize,
}

/// Collect contiguous change regions from a sequence of edit operations.
fn collect_change_regions(edits: &[Edit]) -> Vec<ChangeRegion> {
    let mut regions = Vec::new();
    let mut i = 0;

    while i < edits.len() {
        if edits[i].op == EditOp::Equal {
            i += 1;
            continue;
        }

        // Start of a change region.
        let base_start = edits[i].old_index;
        let new_start = edits[i].new_index;
        let mut base_end = base_start;
        let mut new_end = new_start;

        while i < edits.len() && edits[i].op != EditOp::Equal {
            match edits[i].op {
                EditOp::Delete => {
                    base_end = edits[i].old_index + 1;
                }
                EditOp::Insert => {
                    new_end = edits[i].new_index + 1;
                }
                EditOp::Equal => unreachable!(),
            }
            i += 1;
        }

        regions.push(ChangeRegion {
            base_start,
            base_len: base_end - base_start,
            new_start,
            new_len: new_end - new_start,
        });
    }

    regions
}

/// Merge change regions from ours and theirs against the base.
fn merge_regions(
    base_lines: &[&[u8]],
    ours_lines: &[&[u8]],
    theirs_lines: &[&[u8]],
    hunks_ours: &[ChangeRegion],
    hunks_theirs: &[ChangeRegion],
    conflict_style: ConflictStyle,
    labels: &MergeLabels<'_>,
) -> ContentMergeResult {
    let mut output: Vec<u8> = Vec::new();
    let mut conflict_count = 0;

    let mut base_pos = 0;
    let mut oi = 0; // index into hunks_ours
    let mut ti = 0; // index into hunks_theirs

    while oi < hunks_ours.len() || ti < hunks_theirs.len() {
        let o_region = hunks_ours.get(oi);
        let t_region = hunks_theirs.get(ti);

        match (o_region, t_region) {
            (Some(o), Some(t)) => {
                let o_end = o.base_start + o.base_len;
                let t_end = t.base_start + t.base_len;

                if o_end < t.base_start || (o_end == t.base_start && o.base_start < t.base_start) {
                    // Ours comes strictly first, no overlap.
                    emit_base_lines(&mut output, base_lines, base_pos, o.base_start);
                    emit_lines(&mut output, ours_lines, o.new_start, o.new_len);
                    base_pos = o_end;
                    oi += 1;
                } else if t_end < o.base_start || (t_end == o.base_start && t.base_start < o.base_start) {
                    // Theirs comes strictly first, no overlap.
                    emit_base_lines(&mut output, base_lines, base_pos, t.base_start);
                    emit_lines(&mut output, theirs_lines, t.new_start, t.new_len);
                    base_pos = t_end;
                    ti += 1;
                } else {
                    // Overlapping regions — check if the changes are identical.
                    let ours_content = collect_lines(ours_lines, o.new_start, o.new_len);
                    let theirs_content = collect_lines(theirs_lines, t.new_start, t.new_len);

                    let overlap_base_start = o.base_start.min(t.base_start);
                    let overlap_base_end = o_end.max(t_end);

                    emit_base_lines(&mut output, base_lines, base_pos, overlap_base_start);

                    if ours_content == theirs_content {
                        // Identical changes — accept cleanly.
                        output.extend_from_slice(&ours_content);
                    } else {
                        // Conflict.
                        conflict_count += 1;
                        let base_content =
                            collect_lines(base_lines, overlap_base_start, overlap_base_end - overlap_base_start);
                        emit_conflict(
                            &mut output,
                            &ours_content,
                            &theirs_content,
                            &base_content,
                            conflict_style,
                            labels,
                        );
                    }

                    base_pos = overlap_base_end;
                    oi += 1;
                    ti += 1;
                }
            }
            (Some(o), None) => {
                let o_end = o.base_start + o.base_len;
                emit_base_lines(&mut output, base_lines, base_pos, o.base_start);
                emit_lines(&mut output, ours_lines, o.new_start, o.new_len);
                base_pos = o_end;
                oi += 1;
            }
            (None, Some(t)) => {
                let t_end = t.base_start + t.base_len;
                emit_base_lines(&mut output, base_lines, base_pos, t.base_start);
                emit_lines(&mut output, theirs_lines, t.new_start, t.new_len);
                base_pos = t_end;
                ti += 1;
            }
            (None, None) => unreachable!(),
        }
    }

    // Emit remaining base lines.
    emit_base_lines(&mut output, base_lines, base_pos, base_lines.len());

    if conflict_count > 0 {
        ContentMergeResult::Conflict {
            content: output,
            conflict_count,
        }
    } else {
        ContentMergeResult::Clean(output)
    }
}

/// Emit unchanged base lines from `from` to `to` (exclusive).
fn emit_base_lines(output: &mut Vec<u8>, base_lines: &[&[u8]], from: usize, to: usize) {
    for i in from..to {
        if i < base_lines.len() {
            output.extend_from_slice(base_lines[i]);
            output.push(b'\n');
        }
    }
}

/// Emit lines from a side.
fn emit_lines(output: &mut Vec<u8>, lines: &[&[u8]], start: usize, count: usize) {
    for i in start..start + count {
        if i < lines.len() {
            output.extend_from_slice(lines[i]);
            output.push(b'\n');
        }
    }
}

/// Collect lines into a single byte buffer.
fn collect_lines(lines: &[&[u8]], start: usize, count: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    for i in start..start + count {
        if i < lines.len() {
            buf.extend_from_slice(lines[i]);
            buf.push(b'\n');
        }
    }
    buf
}

/// Emit conflict markers.
fn emit_conflict(
    output: &mut Vec<u8>,
    ours_content: &[u8],
    theirs_content: &[u8],
    base_content: &[u8],
    style: ConflictStyle,
    labels: &MergeLabels<'_>,
) {
    // <<<<<<< ours-label
    output.extend_from_slice(b"<<<<<<< ");
    output.extend_from_slice(labels.ours.as_bytes());
    output.push(b'\n');

    output.extend_from_slice(ours_content);

    match style {
        ConflictStyle::Diff3 | ConflictStyle::ZDiff3 => {
            // ||||||| base-label
            output.extend_from_slice(b"||||||| ");
            output.extend_from_slice(labels.base.as_bytes());
            output.push(b'\n');
            output.extend_from_slice(base_content);
        }
        ConflictStyle::Merge => {}
    }

    // =======
    output.extend_from_slice(b"=======\n");

    output.extend_from_slice(theirs_content);

    // >>>>>>> theirs-label
    output.extend_from_slice(b">>>>>>> ");
    output.extend_from_slice(labels.theirs.as_bytes());
    output.push(b'\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_opts() -> MergeOptions {
        MergeOptions::default()
    }

    fn default_labels() -> MergeLabels<'static> {
        MergeLabels {
            base: "base",
            ours: "HEAD",
            theirs: "feature",
        }
    }

    #[test]
    fn identical_ours_theirs() {
        let base = b"line1\nline2\n";
        let ours = b"line1\nline2\nline3\n";
        let theirs = b"line1\nline2\nline3\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        assert!(result.is_clean());
        assert_eq!(result.content(), ours);
    }

    #[test]
    fn only_ours_changed() {
        let base = b"line1\nline2\n";
        let ours = b"line1\nmodified\n";
        let theirs = b"line1\nline2\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        assert!(result.is_clean());
        assert_eq!(result.content(), ours);
    }

    #[test]
    fn only_theirs_changed() {
        let base = b"line1\nline2\n";
        let ours = b"line1\nline2\n";
        let theirs = b"line1\nmodified\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        assert!(result.is_clean());
        assert_eq!(result.content(), theirs);
    }

    #[test]
    fn non_overlapping_changes() {
        let base = b"line1\nline2\nline3\nline4\n";
        let ours = b"modified1\nline2\nline3\nline4\n";
        let theirs = b"line1\nline2\nline3\nmodified4\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        assert!(result.is_clean());
        let content = String::from_utf8_lossy(result.content());
        assert!(content.contains("modified1"));
        assert!(content.contains("modified4"));
    }

    #[test]
    fn overlapping_conflict() {
        let base = b"line1\nline2\nline3\n";
        let ours = b"line1\nours_change\nline3\n";
        let theirs = b"line1\ntheirs_change\nline3\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        assert!(!result.is_clean());
        let content = String::from_utf8_lossy(result.content());
        assert!(content.contains("<<<<<<< HEAD"));
        assert!(content.contains("======="));
        assert!(content.contains(">>>>>>> feature"));
        assert!(content.contains("ours_change"));
        assert!(content.contains("theirs_change"));
    }

    #[test]
    fn diff3_conflict_style() {
        let base = b"line1\noriginal\nline3\n";
        let ours = b"line1\nours\nline3\n";
        let theirs = b"line1\ntheirs\nline3\n";
        let mut opts = default_opts();
        opts.conflict_style = ConflictStyle::Diff3;
        let result = merge_content(base, ours, theirs, &opts, &default_labels());
        assert!(!result.is_clean());
        let content = String::from_utf8_lossy(result.content());
        assert!(content.contains("||||||| base"));
        assert!(content.contains("original"));
    }

    #[test]
    fn strategy_option_ours() {
        let base = b"line1\nline2\n";
        let ours = b"ours_content\n";
        let theirs = b"theirs_content\n";
        let mut opts = default_opts();
        opts.strategy_options.push("ours".to_string());
        let result = merge_content(base, ours, theirs, &opts, &default_labels());
        assert!(result.is_clean());
        assert_eq!(result.content(), ours);
    }

    #[test]
    fn strategy_option_theirs() {
        let base = b"line1\nline2\n";
        let ours = b"ours_content\n";
        let theirs = b"theirs_content\n";
        let mut opts = default_opts();
        opts.strategy_options.push("theirs".to_string());
        let result = merge_content(base, ours, theirs, &opts, &default_labels());
        assert!(result.is_clean());
        assert_eq!(result.content(), theirs);
    }

    #[test]
    fn both_sides_identical_changes() {
        let base = b"line1\noriginal\nline3\n";
        let ours = b"line1\nsame_change\nline3\n";
        let theirs = b"line1\nsame_change\nline3\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        assert!(result.is_clean());
        let content = String::from_utf8_lossy(result.content());
        assert!(content.contains("same_change"));
    }

    #[test]
    fn empty_base() {
        let base = b"";
        let ours = b"ours line\n";
        let theirs = b"theirs line\n";
        let result = merge_content(base, ours, theirs, &default_opts(), &default_labels());
        // Both adding from nothing is a conflict (add/add).
        assert!(!result.is_clean());
    }
}

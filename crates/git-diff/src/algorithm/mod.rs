//! Diff algorithms: Myers, histogram, patience.

pub mod histogram;
pub mod myers;
pub mod patience;

use bstr::BString;

use crate::{DiffAlgorithm, DiffLine, Hunk};

/// An edit operation in the edit script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOp {
    /// Line present in both old and new (equal).
    Equal,
    /// Line inserted (present only in new).
    Insert,
    /// Line deleted (present only in old).
    Delete,
}

/// A single edit in the edit script, referencing lines by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edit {
    /// The operation.
    pub op: EditOp,
    /// Index into the old sequence (meaningful for Equal and Delete).
    pub old_index: usize,
    /// Index into the new sequence (meaningful for Equal and Insert).
    pub new_index: usize,
}

/// Compute a line-level diff between two byte slices.
///
/// Returns a list of edits representing the transformation from `old` to `new`.
pub fn diff_edits(old: &[u8], new: &[u8], algorithm: DiffAlgorithm) -> Vec<Edit> {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);

    match algorithm {
        DiffAlgorithm::Myers | DiffAlgorithm::Minimal => {
            myers::diff(&old_lines, &new_lines, algorithm == DiffAlgorithm::Minimal)
        }
        DiffAlgorithm::Histogram => histogram::diff(&old_lines, &new_lines),
        DiffAlgorithm::Patience => patience::diff(&old_lines, &new_lines),
    }
}

/// Compute a line-level diff and produce hunks with context lines.
///
/// This is the main entry point for line diffing: takes two byte slices,
/// computes the diff using the specified algorithm, and produces hunks
/// suitable for unified diff output.
pub fn diff_lines(old: &[u8], new: &[u8], algorithm: DiffAlgorithm, context_lines: u32) -> Vec<Hunk> {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let edits = match algorithm {
        DiffAlgorithm::Myers | DiffAlgorithm::Minimal => {
            myers::diff(&old_lines, &new_lines, algorithm == DiffAlgorithm::Minimal)
        }
        DiffAlgorithm::Histogram => histogram::diff(&old_lines, &new_lines),
        DiffAlgorithm::Patience => patience::diff(&old_lines, &new_lines),
    };

    edits_to_hunks(&edits, &old_lines, &new_lines, context_lines)
}

/// Convert a list of edits into hunks with context lines.
fn edits_to_hunks(
    edits: &[Edit],
    old_lines: &[&[u8]],
    new_lines: &[&[u8]],
    context_lines: u32,
) -> Vec<Hunk> {
    if edits.is_empty() {
        return Vec::new();
    }

    let ctx = context_lines as usize;

    // Find ranges of changes (non-Equal edits)
    let mut change_ranges: Vec<(usize, usize)> = Vec::new(); // (start, end) indices into edits
    let mut i = 0;
    while i < edits.len() {
        if edits[i].op != EditOp::Equal {
            let start = i;
            while i < edits.len() && edits[i].op != EditOp::Equal {
                i += 1;
            }
            change_ranges.push((start, i));
        } else {
            i += 1;
        }
    }

    if change_ranges.is_empty() {
        return Vec::new();
    }

    // Merge nearby change ranges (if the gap is <= 2*context_lines)
    let mut merged_ranges: Vec<(usize, usize)> = Vec::new();
    let mut current = change_ranges[0];
    for &(start, end) in &change_ranges[1..] {
        // Count equal lines between current.1 and start
        let gap = start - current.1;
        if gap <= 2 * ctx {
            current.1 = end;
        } else {
            merged_ranges.push(current);
            current = (start, end);
        }
    }
    merged_ranges.push(current);

    // Build hunks from merged ranges
    let mut hunks = Vec::new();
    for (change_start, change_end) in merged_ranges {
        let mut lines = Vec::new();

        // Context before
        let ctx_before_start = change_start.saturating_sub(ctx);
        let mut old_start = edits[change_start].old_index;
        let mut new_start = edits[change_start].new_index;

        // Add leading context lines
        for j in ctx_before_start..change_start {
            if edits[j].op == EditOp::Equal {
                let line_data = old_lines[edits[j].old_index];
                lines.push(DiffLine::Context(BString::from(line_data)));
                if old_start > 0 {
                    old_start = old_start.min(edits[j].old_index);
                }
                new_start = new_start.min(edits[j].new_index);
            }
        }

        // Recalculate old_start and new_start based on context
        if !lines.is_empty() {
            old_start = edits[ctx_before_start].old_index;
            new_start = edits[ctx_before_start].new_index;
        }

        // Change lines
        let mut old_count = lines.len() as u32; // context lines counted for old
        let mut new_count = lines.len() as u32; // context lines counted for new
        for j in change_start..change_end {
            match edits[j].op {
                EditOp::Equal => {
                    let line_data = old_lines[edits[j].old_index];
                    lines.push(DiffLine::Context(BString::from(line_data)));
                    old_count += 1;
                    new_count += 1;
                }
                EditOp::Delete => {
                    let line_data = old_lines[edits[j].old_index];
                    lines.push(DiffLine::Deletion(BString::from(line_data)));
                    old_count += 1;
                }
                EditOp::Insert => {
                    let line_data = new_lines[edits[j].new_index];
                    lines.push(DiffLine::Addition(BString::from(line_data)));
                    new_count += 1;
                }
            }
        }

        // Context after
        let ctx_after_end = (change_end + ctx).min(edits.len());
        for j in change_end..ctx_after_end {
            if edits[j].op == EditOp::Equal {
                let line_data = old_lines[edits[j].old_index];
                lines.push(DiffLine::Context(BString::from(line_data)));
                old_count += 1;
                new_count += 1;
            }
        }

        hunks.push(Hunk {
            old_start: (old_start + 1) as u32, // 1-based
            old_count,
            new_start: (new_start + 1) as u32, // 1-based
            new_count,
            header: None,
            lines,
        });
    }

    hunks
}

/// Split a byte slice into lines (preserving line endings).
pub fn split_lines(data: &[u8]) -> Vec<&[u8]> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, &byte) in data.iter().enumerate() {
        if byte == b'\n' {
            lines.push(&data[start..=i]);
            start = i + 1;
        }
    }
    // If there's content after the last newline, it's a line without trailing newline
    if start < data.len() {
        lines.push(&data[start..]);
    }
    lines
}

/// Compute a hash for a line (used for fast comparison).
/// Uses DJB2a (xor variant) matching xdiff's approach.
pub(crate) fn line_hash(line: &[u8]) -> u64 {
    let mut hash: u64 = 5381;
    for &b in line {
        hash = hash.wrapping_mul(33) ^ (b as u64);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_lines_empty() {
        assert!(split_lines(b"").is_empty());
    }

    #[test]
    fn split_lines_single_no_newline() {
        let lines = split_lines(b"hello");
        assert_eq!(lines, vec![b"hello".as_slice()]);
    }

    #[test]
    fn split_lines_single_with_newline() {
        let lines = split_lines(b"hello\n");
        assert_eq!(lines, vec![b"hello\n".as_slice()]);
    }

    #[test]
    fn split_lines_multiple() {
        let lines = split_lines(b"a\nb\nc\n");
        assert_eq!(lines, vec![b"a\n".as_slice(), b"b\n", b"c\n"]);
    }

    #[test]
    fn split_lines_no_trailing_newline() {
        let lines = split_lines(b"a\nb");
        assert_eq!(lines, vec![b"a\n".as_slice(), b"b"]);
    }

    #[test]
    fn line_hash_deterministic() {
        assert_eq!(line_hash(b"hello\n"), line_hash(b"hello\n"));
        assert_ne!(line_hash(b"hello\n"), line_hash(b"world\n"));
    }
}

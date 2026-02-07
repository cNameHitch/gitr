//! Histogram diff algorithm.
//!
//! A variant of patience diff that uses occurrence counting
//! to find unique matching lines. Tends to produce more readable
//! diffs for code changes. Matches C git's xdiff/xhistogram.c.

use super::{Edit, EditOp, line_hash};
use std::collections::HashMap;

/// Compute a diff using the histogram algorithm.
pub fn diff(old: &[&[u8]], new: &[&[u8]]) -> Vec<Edit> {
    if old.is_empty() && new.is_empty() {
        return Vec::new();
    }
    if old.is_empty() {
        return new
            .iter()
            .enumerate()
            .map(|(i, _)| Edit {
                op: EditOp::Insert,
                old_index: 0,
                new_index: i,
            })
            .collect();
    }
    if new.is_empty() {
        return old
            .iter()
            .enumerate()
            .map(|(i, _)| Edit {
                op: EditOp::Delete,
                old_index: i,
                new_index: 0,
            })
            .collect();
    }

    let mut edits = Vec::new();
    histogram_recurse(old, new, 0, 0, &mut edits, 0);
    edits
}

/// Maximum recursion depth before falling back to Myers.
const MAX_RECURSION: usize = 64;

fn histogram_recurse(
    old: &[&[u8]],
    new: &[&[u8]],
    old_offset: usize,
    new_offset: usize,
    edits: &mut Vec<Edit>,
    depth: usize,
) {
    if old.is_empty() && new.is_empty() {
        return;
    }

    // Fall back to Myers at max recursion depth
    if depth >= MAX_RECURSION {
        let fallback = super::myers::diff(old, new, false);
        for mut e in fallback {
            e.old_index += old_offset;
            e.new_index += new_offset;
            edits.push(e);
        }
        return;
    }

    if old.is_empty() {
        for (i, _) in new.iter().enumerate() {
            edits.push(Edit {
                op: EditOp::Insert,
                old_index: old_offset,
                new_index: new_offset + i,
            });
        }
        return;
    }
    if new.is_empty() {
        for (i, _) in old.iter().enumerate() {
            edits.push(Edit {
                op: EditOp::Delete,
                old_index: old_offset + i,
                new_index: new_offset,
            });
        }
        return;
    }

    // Trim common prefix
    let prefix_len = old
        .iter()
        .zip(new.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Trim common suffix
    let suffix_len = old[prefix_len..]
        .iter()
        .rev()
        .zip(new[prefix_len..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    // Emit prefix equals
    for i in 0..prefix_len {
        edits.push(Edit {
            op: EditOp::Equal,
            old_index: old_offset + i,
            new_index: new_offset + i,
        });
    }

    let old_mid = &old[prefix_len..old.len() - suffix_len];
    let new_mid = &new[prefix_len..new.len() - suffix_len];
    let mid_old_offset = old_offset + prefix_len;
    let mid_new_offset = new_offset + prefix_len;

    if old_mid.is_empty() && new_mid.is_empty() {
        // Only prefix/suffix, no middle
    } else if old_mid.is_empty() {
        for (i, _) in new_mid.iter().enumerate() {
            edits.push(Edit {
                op: EditOp::Insert,
                old_index: mid_old_offset,
                new_index: mid_new_offset + i,
            });
        }
    } else if new_mid.is_empty() {
        for (i, _) in old_mid.iter().enumerate() {
            edits.push(Edit {
                op: EditOp::Delete,
                old_index: mid_old_offset + i,
                new_index: mid_new_offset,
            });
        }
    } else {
        // Build histogram of lines in old (hash -> (count, indices))
        let mut histogram: HashMap<u64, (usize, Vec<usize>)> = HashMap::new();
        for (i, line) in old_mid.iter().enumerate() {
            let h = line_hash(line);
            let entry = histogram.entry(h).or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(i);
        }

        // Find the lowest-occurrence line from old that also appears in new
        let mut best_count = usize::MAX;
        let mut best_old_idx = None;
        let mut best_new_idx = None;

        for (j, line) in new_mid.iter().enumerate() {
            let h = line_hash(line);
            if let Some((count, indices)) = histogram.get(&h) {
                // Verify actual content match (hash collision check)
                for &oi in indices {
                    if old_mid[oi] == *line && *count < best_count {
                        best_count = *count;
                        best_old_idx = Some(oi);
                        best_new_idx = Some(j);
                    }
                }
            }
        }

        if let (Some(oi), Some(ni)) = (best_old_idx, best_new_idx) {
            // Found a pivot: recurse on segments before and after
            histogram_recurse(
                &old_mid[..oi],
                &new_mid[..ni],
                mid_old_offset,
                mid_new_offset,
                edits,
                depth + 1,
            );

            // The matching line itself
            edits.push(Edit {
                op: EditOp::Equal,
                old_index: mid_old_offset + oi,
                new_index: mid_new_offset + ni,
            });

            histogram_recurse(
                &old_mid[oi + 1..],
                &new_mid[ni + 1..],
                mid_old_offset + oi + 1,
                mid_new_offset + ni + 1,
                edits,
                depth + 1,
            );
        } else {
            // No common line found: everything is a change
            for (i, _) in old_mid.iter().enumerate() {
                edits.push(Edit {
                    op: EditOp::Delete,
                    old_index: mid_old_offset + i,
                    new_index: mid_new_offset,
                });
            }
            for (j, _) in new_mid.iter().enumerate() {
                edits.push(Edit {
                    op: EditOp::Insert,
                    old_index: mid_old_offset + old_mid.len(),
                    new_index: mid_new_offset + j,
                });
            }
        }
    }

    // Emit suffix equals
    for i in 0..suffix_len {
        edits.push(Edit {
            op: EditOp::Equal,
            old_index: old.len() - suffix_len + old_offset + i,
            new_index: new.len() - suffix_len + new_offset + i,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical() {
        let a = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let b = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let edits = diff(&a, &b);
        assert!(edits.iter().all(|e| e.op == EditOp::Equal));
        assert_eq!(edits.len(), 3);
    }

    #[test]
    fn insert_line() {
        let a = vec![b"a\n".as_slice(), b"c\n"];
        let b = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let edits = diff(&a, &b);
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        assert_eq!(inserts, 1);
    }

    #[test]
    fn delete_line() {
        let a = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let b = vec![b"a\n".as_slice(), b"c\n"];
        let edits = diff(&a, &b);
        let deletes = edits.iter().filter(|e| e.op == EditOp::Delete).count();
        assert_eq!(deletes, 1);
    }

    #[test]
    fn empty_inputs() {
        let empty: Vec<&[u8]> = vec![];
        let a = vec![b"x\n".as_slice()];
        assert!(diff(&empty, &empty).is_empty());
        assert_eq!(diff(&empty, &a).len(), 1);
        assert_eq!(diff(&a, &empty).len(), 1);
    }
}

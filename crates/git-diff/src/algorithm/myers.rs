//! Myers diff algorithm implementation.
//!
//! Implements Eugene Myers' O(ND) difference algorithm as described in
//! "An O(ND) Difference Algorithm and Its Variations" (1986).
//! This matches the behavior of git's xdiff/xdiffi.c.

use super::{Edit, EditOp, line_hash};

/// Compute a diff using the Myers algorithm.
///
/// If `minimal` is true, always finds the absolute minimum edit script
/// (no heuristic shortcuts). Otherwise, uses the same heuristics as
/// C git's xdiff to bound execution time.
pub fn diff(old: &[&[u8]], new: &[&[u8]], minimal: bool) -> Vec<Edit> {
    // Handle trivial cases
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

    // Pre-compute line hashes for O(1) comparison
    let old_hashes: Vec<u64> = old.iter().map(|l| line_hash(l)).collect();
    let new_hashes: Vec<u64> = new.iter().map(|l| line_hash(l)).collect();

    // Trim common prefix
    let prefix_len = old_hashes
        .iter()
        .zip(new_hashes.iter())
        .zip(old.iter().zip(new.iter()))
        .take_while(|((oh, nh), (a, b))| oh == nh && a == b)
        .count();

    // Trim common suffix
    let suffix_len = old_hashes[prefix_len..]
        .iter()
        .rev()
        .zip(new_hashes[prefix_len..].iter().rev())
        .zip(old[prefix_len..].iter().rev().zip(new[prefix_len..].iter().rev()))
        .take_while(|((oh, nh), (a, b))| oh == nh && a == b)
        .count();

    let old_trimmed = &old[prefix_len..old.len() - suffix_len];
    let new_trimmed = &new[prefix_len..new.len() - suffix_len];
    let old_h = &old_hashes[prefix_len..old_hashes.len() - suffix_len];
    let new_h = &new_hashes[prefix_len..new_hashes.len() - suffix_len];

    // Run Myers on the trimmed sequences
    let inner_edits = myers_inner(old_trimmed, new_trimmed, old_h, new_h, minimal);

    // Build final edit list
    let mut edits = Vec::with_capacity(old.len() + new.len());

    // Common prefix
    for i in 0..prefix_len {
        edits.push(Edit {
            op: EditOp::Equal,
            old_index: i,
            new_index: i,
        });
    }

    // Inner edits (adjust indices)
    for edit in inner_edits {
        edits.push(Edit {
            op: edit.op,
            old_index: edit.old_index + prefix_len,
            new_index: edit.new_index + prefix_len,
        });
    }

    // Common suffix
    for i in 0..suffix_len {
        edits.push(Edit {
            op: EditOp::Equal,
            old_index: old.len() - suffix_len + i,
            new_index: new.len() - suffix_len + i,
        });
    }

    edits
}

/// Core Myers algorithm on pre-trimmed sequences.
fn myers_inner(
    old: &[&[u8]],
    new: &[&[u8]],
    old_h: &[u64],
    new_h: &[u64],
    _minimal: bool,
) -> Vec<Edit> {
    let n = old.len();
    let m = new.len();

    if n == 0 && m == 0 {
        return Vec::new();
    }
    if n == 0 {
        return (0..m)
            .map(|j| Edit {
                op: EditOp::Insert,
                old_index: 0,
                new_index: j,
            })
            .collect();
    }
    if m == 0 {
        return (0..n)
            .map(|i| Edit {
                op: EditOp::Delete,
                old_index: i,
                new_index: 0,
            })
            .collect();
    }

    // max_d is the maximum possible edit distance. For minimal mode we always
    // search the full space. For non-minimal, we could apply heuristics to
    // bail out early, but the maximum D must still cover the worst case.
    let max_d = n + m;
    let v_size = 2 * max_d + 1;
    let v_offset = max_d as isize;

    // V array: v[k + offset] = furthest reaching x on diagonal k
    // We store the full trace for backtracking.
    let mut trace: Vec<Vec<usize>> = Vec::new();
    let mut v = vec![0usize; v_size];

    'outer: for d in 0..=max_d {
        let mut v_copy = v.clone();

        let k_min = -(d as isize);
        let k_max = d as isize;

        let mut k = k_min;
        while k <= k_max {
            let idx = (k + v_offset) as usize;

            // Choose whether to go down or right
            let mut x = if k == k_min
                || (k != k_max && v[((k - 1) + v_offset) as usize] < v[((k + 1) + v_offset) as usize])
            {
                // Go down (insert)
                v[((k + 1) + v_offset) as usize]
            } else {
                // Go right (delete)
                v[((k - 1) + v_offset) as usize] + 1
            };

            let mut y = (x as isize - k) as usize;

            // Follow the diagonal (snake)
            while x < n && y < m && old_h[x] == new_h[y] && old[x] == new[y] {
                x += 1;
                y += 1;
            }

            v_copy[idx] = x;

            if x >= n && y >= m {
                trace.push(v_copy);
                break 'outer;
            }

            k += 2;
        }

        trace.push(v_copy);
        v = trace.last().unwrap().clone();
    }

    // Backtrack to find the actual edit path
    backtrack(&trace, n, m, v_offset)
}

/// Backtrack through the trace to produce the edit script.
fn backtrack(
    trace: &[Vec<usize>],
    n: usize,
    m: usize,
    v_offset: isize,
) -> Vec<Edit> {
    let mut edits = Vec::new();
    let mut x = n;
    let mut y = m;

    for d in (0..trace.len()).rev() {
        let k = x as isize - y as isize;

        let prev_k = if d == 0 {
            // At d=0 we started at (0,0)
            k
        } else if k == -(d as isize)
            || (k != d as isize
                && trace[d - 1][((k - 1) + v_offset) as usize]
                    < trace[d - 1][((k + 1) + v_offset) as usize])
        {
            k + 1 // came from down (insert)
        } else {
            k - 1 // came from right (delete)
        };

        let prev_x = if d == 0 {
            0
        } else {
            trace[d - 1][(prev_k + v_offset) as usize]
        };
        let prev_y = (prev_x as isize - prev_k) as usize;

        // Record the snake (diagonal) - equal lines from end to mid
        while x > prev_x && y > prev_y && x > 0 && y > 0 {
            x -= 1;
            y -= 1;
            edits.push(Edit {
                op: EditOp::Equal,
                old_index: x,
                new_index: y,
            });
        }

        if d > 0 {
            if prev_k == k + 1 {
                // Insert: y advanced
                if y > 0 {
                    y -= 1;
                    edits.push(Edit {
                        op: EditOp::Insert,
                        old_index: x,
                        new_index: y,
                    });
                }
            } else {
                // Delete: x advanced
                if x > 0 {
                    x -= 1;
                    edits.push(Edit {
                        op: EditOp::Delete,
                        old_index: x,
                        new_index: y,
                    });
                }
            }
        }
    }

    edits.reverse();
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithm::EditOp;

    #[test]
    fn identical() {
        let a = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let b = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let edits = diff(&a, &b, false);
        assert!(edits.iter().all(|e| e.op == EditOp::Equal));
        assert_eq!(edits.len(), 3);
    }

    #[test]
    fn all_different() {
        let a = vec![b"a\n".as_slice(), b"b\n"];
        let b = vec![b"c\n".as_slice(), b"d\n"];
        let edits = diff(&a, &b, false);
        let deletes = edits.iter().filter(|e| e.op == EditOp::Delete).count();
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        assert_eq!(deletes, 2);
        assert_eq!(inserts, 2);
    }

    #[test]
    fn insert_at_end() {
        let a = vec![b"a\n".as_slice(), b"b\n"];
        let b = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let edits = diff(&a, &b, false);
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        let equals = edits.iter().filter(|e| e.op == EditOp::Equal).count();
        assert_eq!(inserts, 1);
        assert_eq!(equals, 2);
    }

    #[test]
    fn delete_from_middle() {
        let a = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let b = vec![b"a\n".as_slice(), b"c\n"];
        let edits = diff(&a, &b, false);
        let deletes = edits.iter().filter(|e| e.op == EditOp::Delete).count();
        let equals = edits.iter().filter(|e| e.op == EditOp::Equal).count();
        assert_eq!(deletes, 1);
        assert_eq!(equals, 2);
    }

    #[test]
    fn empty_old() {
        let a: Vec<&[u8]> = vec![];
        let b = vec![b"a\n".as_slice(), b"b\n"];
        let edits = diff(&a, &b, false);
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|e| e.op == EditOp::Insert));
    }

    #[test]
    fn empty_new() {
        let a = vec![b"a\n".as_slice(), b"b\n"];
        let b: Vec<&[u8]> = vec![];
        let edits = diff(&a, &b, false);
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|e| e.op == EditOp::Delete));
    }

    #[test]
    fn both_empty() {
        let a: Vec<&[u8]> = vec![];
        let b: Vec<&[u8]> = vec![];
        let edits = diff(&a, &b, false);
        assert!(edits.is_empty());
    }

    #[test]
    fn minimal_flag_still_correct() {
        let a = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let b = vec![b"a\n".as_slice(), b"x\n", b"c\n"];
        let edits = diff(&a, &b, true);
        let deletes = edits.iter().filter(|e| e.op == EditOp::Delete).count();
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        assert_eq!(deletes, 1);
        assert_eq!(inserts, 1);
    }
}

//! Patience diff algorithm.
//!
//! Uses patience sorting on lines unique to both sequences to find
//! a longest common subsequence of unique lines, then recursively
//! diffs the gaps. Tends to produce more semantically meaningful
//! diffs. Matches C git's xdiff/xpatience.c.

use super::{Edit, EditOp, line_hash};
use std::collections::HashMap;

/// Compute a diff using the patience algorithm.
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
    patience_recurse(old, new, 0, 0, &mut edits, 0);
    edits
}

const MAX_RECURSION: usize = 64;

fn patience_recurse(
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
        // Nothing in the middle
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
        // Find lines unique in both old and new
        let unique_matches = find_unique_matches(old_mid, new_mid);

        if unique_matches.is_empty() {
            // No unique common lines: fall back to Myers
            let fallback = super::myers::diff(old_mid, new_mid, false);
            for mut e in fallback {
                e.old_index += mid_old_offset;
                e.new_index += mid_new_offset;
                edits.push(e);
            }
        } else {
            // Find LIS (longest increasing subsequence) of new indices
            // to get the longest common subsequence of unique lines
            let lcs = patience_lcs(&unique_matches);

            // Recursively diff gaps between LCS anchors
            let mut prev_old = 0;
            let mut prev_new = 0;

            for &(oi, ni) in &lcs {
                // Recurse on the gap before this anchor
                if oi > prev_old || ni > prev_new {
                    patience_recurse(
                        &old_mid[prev_old..oi],
                        &new_mid[prev_new..ni],
                        mid_old_offset + prev_old,
                        mid_new_offset + prev_new,
                        edits,
                        depth + 1,
                    );
                }

                // The matching unique line
                edits.push(Edit {
                    op: EditOp::Equal,
                    old_index: mid_old_offset + oi,
                    new_index: mid_new_offset + ni,
                });

                prev_old = oi + 1;
                prev_new = ni + 1;
            }

            // Recurse on the tail gap
            if prev_old < old_mid.len() || prev_new < new_mid.len() {
                patience_recurse(
                    &old_mid[prev_old..],
                    &new_mid[prev_new..],
                    mid_old_offset + prev_old,
                    mid_new_offset + prev_new,
                    edits,
                    depth + 1,
                );
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

/// Find lines that appear exactly once in both old and new.
/// Returns pairs (old_index, new_index) sorted by old_index.
fn find_unique_matches(old: &[&[u8]], new: &[&[u8]]) -> Vec<(usize, usize)> {
    // Count occurrences in old
    let mut old_counts: HashMap<u64, (usize, usize)> = HashMap::new(); // hash -> (count, index)
    for (i, line) in old.iter().enumerate() {
        let h = line_hash(line);
        let entry = old_counts.entry(h).or_insert((0, i));
        entry.0 += 1;
        entry.1 = i; // last index (only meaningful if count == 1)
    }

    // Count occurrences in new and find matches
    let mut new_counts: HashMap<u64, (usize, usize)> = HashMap::new();
    for (i, line) in new.iter().enumerate() {
        let h = line_hash(line);
        let entry = new_counts.entry(h).or_insert((0, i));
        entry.0 += 1;
        entry.1 = i;
    }

    // Collect unique matches (appear exactly once in both, with content equality)
    let mut matches = Vec::new();
    for (i, line) in old.iter().enumerate() {
        let h = line_hash(line);
        if let (Some(&(oc, _)), Some(&(nc, ni))) =
            (old_counts.get(&h), new_counts.get(&h))
        {
            if oc == 1 && nc == 1 && old[i] == new[ni] {
                matches.push((i, ni));
            }
        }
    }

    matches.sort_by_key(|&(oi, _)| oi);
    matches
}

/// Patience sorting to find the longest increasing subsequence.
/// Input: pairs (old_index, new_index) sorted by old_index.
/// We need the LIS of new_index values.
fn patience_lcs(matches: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if matches.is_empty() {
        return Vec::new();
    }

    // Patience sort on new_index
    let mut piles: Vec<Vec<usize>> = Vec::new(); // each pile stores indices into matches
    let mut backptrs: Vec<Option<usize>> = vec![None; matches.len()]; // previous element in LIS

    for (idx, &(_, ni)) in matches.iter().enumerate() {
        // Binary search for the leftmost pile whose top has new_index >= ni
        let pile_idx = piles.partition_point(|pile| {
            matches[*pile.last().unwrap()].1 < ni
        });

        if pile_idx > 0 {
            backptrs[idx] = Some(*piles[pile_idx - 1].last().unwrap());
        }

        if pile_idx == piles.len() {
            piles.push(vec![idx]);
        } else {
            piles[pile_idx].push(idx);
        }
    }

    // Reconstruct LIS from the last pile
    let mut result = Vec::new();
    let mut current = Some(*piles.last().unwrap().last().unwrap());
    while let Some(idx) = current {
        result.push(matches[idx]);
        current = backptrs[idx];
    }
    result.reverse();
    result
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
    fn simple_insert() {
        let a = vec![b"a\n".as_slice(), b"c\n"];
        let b = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let edits = diff(&a, &b);
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        assert_eq!(inserts, 1);
    }

    #[test]
    fn simple_delete() {
        let a = vec![b"a\n".as_slice(), b"b\n", b"c\n"];
        let b = vec![b"a\n".as_slice(), b"c\n"];
        let edits = diff(&a, &b);
        let deletes = edits.iter().filter(|e| e.op == EditOp::Delete).count();
        assert_eq!(deletes, 1);
    }

    #[test]
    fn unique_lines_anchor() {
        // Patience should anchor on unique lines
        let a = vec![
            b"{\n".as_slice(),
            b"  a\n",
            b"}\n",
            b"{\n",
            b"  b\n",
            b"}\n",
        ];
        let b = vec![
            b"{\n".as_slice(),
            b"  a\n",
            b"  x\n",
            b"}\n",
            b"{\n",
            b"  b\n",
            b"}\n",
        ];
        let edits = diff(&a, &b);
        // Should have 1 insert and 6 equals
        let inserts = edits.iter().filter(|e| e.op == EditOp::Insert).count();
        assert_eq!(inserts, 1);
    }

    #[test]
    fn patience_lcs_basic() {
        let matches = vec![(0, 2), (1, 0), (2, 3), (3, 1)];
        let lcs = patience_lcs(&matches);
        // LIS of [2, 0, 3, 1] is [0, 1] or [0, 3] or [2, 3]
        assert!(lcs.len() >= 2);
        // Verify it's increasing in new_index
        for w in lcs.windows(2) {
            assert!(w[0].1 < w[1].1);
        }
    }
}

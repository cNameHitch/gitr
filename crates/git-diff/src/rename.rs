//! Rename and copy detection.
//!
//! Detects file renames (same content moved to a different path) and copies,
//! using exact OID matching and fuzzy similarity scoring.

use git_odb::ObjectDatabase;

use crate::{DiffError, DiffResult, FileDiff, FileStatus};
use crate::tree::read_blob;

/// Run rename detection on a DiffResult, converting matching
/// delete+add pairs into renames.
pub fn detect_renames(
    odb: &ObjectDatabase,
    result: &mut DiffResult,
    threshold: u8,
) -> Result<(), DiffError> {
    // Collect indices of deleted and added files
    let deleted: Vec<usize> = result
        .files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.status == FileStatus::Deleted)
        .map(|(i, _)| i)
        .collect();

    let added: Vec<usize> = result
        .files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.status == FileStatus::Added)
        .map(|(i, _)| i)
        .collect();

    if deleted.is_empty() || added.is_empty() {
        return Ok(());
    }

    // Phase 1: Exact rename detection (same OID)
    let mut matched_deleted = vec![false; deleted.len()];
    let mut matched_added = vec![false; added.len()];
    let mut renames: Vec<(usize, usize, u8)> = Vec::new(); // (deleted_idx, added_idx, similarity)

    for (di, &del_idx) in deleted.iter().enumerate() {
        if matched_deleted[di] {
            continue;
        }
        let del_oid = match result.files[del_idx].old_oid {
            Some(oid) => oid,
            None => continue,
        };

        for (ai, &add_idx) in added.iter().enumerate() {
            if matched_added[ai] {
                continue;
            }
            let add_oid = match result.files[add_idx].new_oid {
                Some(oid) => oid,
                None => continue,
            };

            if del_oid == add_oid {
                // Exact match
                matched_deleted[di] = true;
                matched_added[ai] = true;
                renames.push((del_idx, add_idx, 100));
                break;
            }
        }
    }

    // Phase 2: Fuzzy rename detection (similarity scoring)
    if threshold < 100 {
        for (di, &del_idx) in deleted.iter().enumerate() {
            if matched_deleted[di] {
                continue;
            }

            let del_oid = match result.files[del_idx].old_oid {
                Some(oid) => oid,
                None => continue,
            };

            let old_data = match read_blob(odb, &del_oid) {
                Ok(data) => data,
                Err(_) => continue,
            };

            let mut best_score: u8 = 0;
            let mut best_ai: Option<usize> = None;

            for (ai, &add_idx) in added.iter().enumerate() {
                if matched_added[ai] {
                    continue;
                }

                let add_oid = match result.files[add_idx].new_oid {
                    Some(oid) => oid,
                    None => continue,
                };

                let new_data = match read_blob(odb, &add_oid) {
                    Ok(data) => data,
                    Err(_) => continue,
                };

                let score = similarity_score(&old_data, &new_data);
                if score >= threshold && score > best_score {
                    best_score = score;
                    best_ai = Some(ai);
                }
            }

            if let Some(ai) = best_ai {
                matched_deleted[di] = true;
                matched_added[ai] = true;
                renames.push((del_idx, added[ai], best_score));
            }
        }
    }

    // Apply renames: convert matched pairs
    for (del_idx, add_idx, sim) in renames {
        let old_path = result.files[del_idx].old_path.clone();
        let old_mode = result.files[del_idx].old_mode;
        let old_oid = result.files[del_idx].old_oid;

        let add_file = &mut result.files[add_idx];
        add_file.status = FileStatus::Renamed;
        add_file.old_path = old_path;
        add_file.old_mode = old_mode;
        add_file.old_oid = old_oid;
        add_file.similarity = Some(sim);

        // Mark the deleted entry for removal
        result.files[del_idx].status = FileStatus::Modified; // sentinel
        result.files[del_idx].similarity = Some(255); // mark for removal
    }

    // Remove the consumed deleted entries
    result.files.retain(|f| f.similarity != Some(255));

    Ok(())
}

/// Run copy detection on a DiffResult.
pub fn detect_copies(
    odb: &ObjectDatabase,
    result: &mut DiffResult,
    threshold: u8,
    all_files: &[FileDiff],
) -> Result<(), DiffError> {
    let added: Vec<usize> = result
        .files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.status == FileStatus::Added)
        .map(|(i, _)| i)
        .collect();

    if added.is_empty() {
        return Ok(());
    }

    for &add_idx in &added {
        let add_oid = match result.files[add_idx].new_oid {
            Some(oid) => oid,
            None => continue,
        };

        let new_data = match read_blob(odb, &add_oid) {
            Ok(data) => data,
            Err(_) => continue,
        };

        let mut best_score: u8 = 0;
        let mut best_source: Option<&FileDiff> = None;

        for source in all_files {
            let src_oid = match source.old_oid.or(source.new_oid) {
                Some(oid) => oid,
                None => continue,
            };

            if src_oid == add_oid {
                // Exact copy
                best_score = 100;
                best_source = Some(source);
                break;
            }

            let src_data = match read_blob(odb, &src_oid) {
                Ok(data) => data,
                Err(_) => continue,
            };

            let score = similarity_score(&src_data, &new_data);
            if score >= threshold && score > best_score {
                best_score = score;
                best_source = Some(source);
            }
        }

        if let Some(source) = best_source {
            let add_file = &mut result.files[add_idx];
            add_file.status = FileStatus::Copied;
            add_file.old_path = source.new_path.clone().or_else(|| source.old_path.clone());
            add_file.old_mode = source.new_mode.or(source.old_mode);
            add_file.old_oid = source.new_oid.or(source.old_oid);
            add_file.similarity = Some(best_score);
        }
    }

    Ok(())
}

/// Compute similarity score between two byte sequences (0-100).
///
/// Uses a simple delta-based metric matching C git's approach:
/// similarity = max(0, (base_size - delta_size) * 100 / base_size)
pub fn similarity_score(old: &[u8], new: &[u8]) -> u8 {
    if old.is_empty() && new.is_empty() {
        return 100;
    }
    if old.is_empty() || new.is_empty() {
        return 0;
    }

    let base_size = old.len().max(new.len());
    let delta_size = edit_distance_approx(old, new);

    if delta_size >= base_size {
        0
    } else {
        ((base_size - delta_size) * 100 / base_size) as u8
    }
}

/// Fast approximate edit distance using line-level comparison.
/// Returns approximate number of bytes that differ.
fn edit_distance_approx(old: &[u8], new: &[u8]) -> usize {
    use crate::algorithm::{split_lines, line_hash};
    use std::collections::HashMap;

    let old_lines = split_lines(old);
    let new_lines = split_lines(new);

    // Count lines in old
    let mut old_counts: HashMap<u64, usize> = HashMap::new();
    let mut old_sizes: HashMap<u64, usize> = HashMap::new();
    for line in &old_lines {
        let h = line_hash(line);
        *old_counts.entry(h).or_insert(0) += 1;
        old_sizes.entry(h).or_insert(line.len());
    }

    // Subtract matching lines from new
    let mut unmatched_bytes = 0usize;
    for line in &new_lines {
        let h = line_hash(line);
        if let Some(count) = old_counts.get_mut(&h) {
            if *count > 0 {
                *count -= 1;
                continue;
            }
        }
        unmatched_bytes += line.len();
    }

    // Add remaining unmatched old lines
    for (&h, &count) in &old_counts {
        if count > 0 {
            let line_size = old_sizes.get(&h).copied().unwrap_or(1);
            unmatched_bytes += count * line_size;
        }
    }

    unmatched_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_identical() {
        assert_eq!(similarity_score(b"hello\nworld\n", b"hello\nworld\n"), 100);
    }

    #[test]
    fn similarity_completely_different() {
        assert_eq!(similarity_score(b"aaa\nbbb\n", b"xxx\nyyy\n"), 0);
    }

    #[test]
    fn similarity_empty() {
        assert_eq!(similarity_score(b"", b""), 100);
        assert_eq!(similarity_score(b"hello", b""), 0);
        assert_eq!(similarity_score(b"", b"hello"), 0);
    }

    #[test]
    fn similarity_partial() {
        let old = b"line1\nline2\nline3\nline4\n";
        let new = b"line1\nline2\nline3\nnewline\n";
        let score = similarity_score(old, new);
        // 3 of 4 lines match, so similarity should be significant
        assert!(score > 30, "score {} should be > 30", score);
    }
}

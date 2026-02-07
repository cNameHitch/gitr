//! Compute deltas between objects.
//!
//! Implements a simplified version of git's diff-delta algorithm.
//! The algorithm builds a hash table of fixed-size blocks from the source,
//! then scans the target looking for matching blocks. Matching regions
//! become copy instructions; non-matching regions become insert instructions.

use std::collections::HashMap;

use super::{encode_copy, encode_insert, write_varint};

/// Block size for the rolling hash (must be a power of 2 for efficiency).
const BLOCK_SIZE: usize = 16;

/// Compute a delta that transforms `source` into `target`.
///
/// Returns the raw delta byte stream that can be applied with `apply_delta`.
pub fn compute_delta(source: &[u8], target: &[u8]) -> Vec<u8> {
    let mut delta = Vec::new();

    // Write source and target sizes
    delta.extend_from_slice(&write_varint(source.len()));
    delta.extend_from_slice(&write_varint(target.len()));

    if target.is_empty() {
        return delta;
    }

    // Build index of source blocks
    let index = build_block_index(source);

    let mut tpos = 0;
    let mut pending_insert: Vec<u8> = Vec::new();

    while tpos < target.len() {
        let remaining = target.len() - tpos;

        // Try to find a matching block in the source
        if remaining >= BLOCK_SIZE {
            let block = &target[tpos..tpos + BLOCK_SIZE];
            if let Some(&src_offset) = index.get(block) {
                // Found a match! Extend it as far as possible
                let match_len = extend_match(source, src_offset, target, tpos);

                // Flush pending insert
                flush_insert(&mut delta, &mut pending_insert);

                // Emit copy instruction
                emit_copy(&mut delta, src_offset, match_len);
                tpos += match_len;
                continue;
            }
        }

        // No match — accumulate as insert
        pending_insert.push(target[tpos]);
        tpos += 1;

        // Flush inserts in chunks of 127 (max insert size)
        if pending_insert.len() == 127 {
            flush_insert(&mut delta, &mut pending_insert);
        }
    }

    // Flush remaining insert
    flush_insert(&mut delta, &mut pending_insert);

    delta
}

/// Build a hash map from BLOCK_SIZE chunks of source to their offsets.
fn build_block_index(source: &[u8]) -> HashMap<&[u8], usize> {
    let mut index = HashMap::new();
    if source.len() < BLOCK_SIZE {
        return index;
    }
    // Step by BLOCK_SIZE for non-overlapping blocks (faster indexing)
    for offset in (0..=source.len() - BLOCK_SIZE).step_by(BLOCK_SIZE) {
        let block = &source[offset..offset + BLOCK_SIZE];
        // First occurrence wins (don't overwrite)
        index.entry(block).or_insert(offset);
    }
    index
}

/// Extend a match between source[src_off..] and target[tgt_off..] as far as possible.
fn extend_match(source: &[u8], src_off: usize, target: &[u8], tgt_off: usize) -> usize {
    let max_len = std::cmp::min(source.len() - src_off, target.len() - tgt_off);
    let mut len = BLOCK_SIZE;
    while len < max_len && source[src_off + len] == target[tgt_off + len] {
        len += 1;
    }
    len
}

/// Flush pending insert bytes as one or more insert instructions.
fn flush_insert(delta: &mut Vec<u8>, pending: &mut Vec<u8>) {
    while !pending.is_empty() {
        let chunk_len = std::cmp::min(pending.len(), 127);
        let chunk: Vec<u8> = pending.drain(..chunk_len).collect();
        delta.extend_from_slice(&encode_insert(&chunk));
    }
}

/// Emit a copy instruction, splitting into multiple if needed (max copy size = 0xffffff).
fn emit_copy(delta: &mut Vec<u8>, offset: usize, mut size: usize) {
    let mut off = offset;
    while size > 0 {
        let chunk = std::cmp::min(size, 0x00ff_ffff); // max 24-bit size
        delta.extend_from_slice(&encode_copy(off as u64, chunk));
        off += chunk;
        size -= chunk;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::apply::apply_delta;

    #[test]
    fn identical_objects() {
        let data = b"Hello, World! This is a test of delta compression.";
        let delta = compute_delta(data, data);
        let result = apply_delta(data, &delta).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn completely_different() {
        let source = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let target = b"BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
        let delta = compute_delta(source, target);
        let result = apply_delta(source, &delta).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn empty_target() {
        let source = b"something";
        let target = b"";
        let delta = compute_delta(source, target);
        let result = apply_delta(source, &delta).unwrap();
        assert_eq!(result, target.as_slice());
    }

    #[test]
    fn empty_source() {
        let source = b"";
        let target = b"new content here";
        let delta = compute_delta(source, target);
        let result = apply_delta(source, &delta).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn prepend_data() {
        // Source has a block that's reused in target, just with a prefix
        let source = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let mut target = b"PREPENDED_".to_vec();
        target.extend_from_slice(source);
        let delta = compute_delta(source, &target);
        let result = apply_delta(source, &delta).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn append_data() {
        let source = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let mut target = source.to_vec();
        target.extend_from_slice(b"_APPENDED");
        let delta = compute_delta(source, &target);
        let result = apply_delta(source, &delta).unwrap();
        assert_eq!(result, target);
    }

    #[test]
    fn large_similar_objects() {
        // Simulate two versions of a file with minor changes
        let source: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
        let mut target = source.clone();
        // Change a few bytes in the middle
        target[2048] = 0xFF;
        target[2049] = 0xFE;
        target[2050] = 0xFD;

        let delta = compute_delta(&source, &target);
        let result = apply_delta(&source, &delta).unwrap();
        assert_eq!(result, target);

        // Delta should be smaller than the full target
        assert!(delta.len() < target.len());
    }
}

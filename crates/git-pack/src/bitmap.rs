//! Bitmap index for fast reachability queries.
//!
//! Bitmap indexes store pre-computed reachability bitmaps for selected commits.
//! Each bit in the bitmap corresponds to an object in the pack index (by position).
//! If bit N is set, object N is reachable from that commit.
//!
//! The bitmap uses EWAH (Enhanced Word-Aligned Hybrid) compression.
//!
//! File format:
//! ```text
//! Header: "BITM" (4) | version (2) | flags (2) | num_entries (4) | checksum (20)
//! Commit entries: [commit_pos (4) | xor_offset (1) | flags (1) | bitmap (EWAH)]*
//! ```

use std::path::{Path, PathBuf};

use git_hash::HashAlgorithm;
use memmap2::Mmap;

use crate::PackError;

/// Bitmap index header signature.
const BITMAP_SIGNATURE: &[u8; 4] = b"BITM";

/// A bitmap index for fast reachability queries.
#[allow(dead_code)]
pub struct BitmapIndex {
    data: Mmap,
    num_entries: u32,
    hash_algo: HashAlgorithm,
    /// Offset where bitmap entries start.
    entries_offset: usize,
    bitmap_path: PathBuf,
    /// Index positions of commits that have bitmaps.
    commit_positions: Vec<u32>,
}

impl BitmapIndex {
    /// Open a bitmap index file.
    pub fn open(bitmap_path: impl AsRef<Path>) -> Result<Self, PackError> {
        let bitmap_path = bitmap_path.as_ref().to_path_buf();
        let file = std::fs::File::open(&bitmap_path)?;
        let data = unsafe { Mmap::map(&file)? };

        let hash_algo = HashAlgorithm::Sha1;
        let hash_len = hash_algo.digest_len();

        // Minimum: header(4) + version(2) + flags(2) + num_entries(4) + checksum(hash_len)
        let min_size = 4 + 2 + 2 + 4 + hash_len;
        if data.len() < min_size {
            return Err(PackError::InvalidIndex("bitmap file too small".into()));
        }

        // Validate signature
        if &data[0..4] != BITMAP_SIGNATURE {
            return Err(PackError::InvalidIndex("bad bitmap signature".into()));
        }

        let version = u16::from_be_bytes([data[4], data[5]]);
        if version != 1 {
            return Err(PackError::InvalidIndex(format!(
                "unsupported bitmap version {version}"
            )));
        }

        let _flags = u16::from_be_bytes([data[6], data[7]]);
        let num_entries = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        // The pack checksum follows
        let entries_offset = 12 + hash_len;

        // Pre-scan to find commit positions (simplified — just read the position fields)
        let mut commit_positions = Vec::with_capacity(num_entries as usize);
        let mut pos = entries_offset;

        for _ in 0..num_entries {
            if pos + 6 > data.len() {
                break;
            }
            let commit_pos = u32::from_be_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
            ]);
            commit_positions.push(commit_pos);

            // Skip: commit_pos(4) + xor_offset(1) + flags(1)
            pos += 6;

            // Skip 4 EWAH bitmaps (commits, trees, blobs, tags)
            for _ in 0..4 {
                pos = skip_ewah_bitmap(&data, pos)?;
            }
        }

        Ok(Self {
            data,
            num_entries,
            hash_algo,
            entries_offset,
            bitmap_path,
            commit_positions,
        })
    }

    /// Check if a bitmap exists for the commit at the given index position.
    pub fn has_bitmap_for_position(&self, index_position: u32) -> bool {
        self.commit_positions.contains(&index_position)
    }

    /// Get the number of commit entries with bitmaps.
    pub fn num_entries(&self) -> u32 {
        self.num_entries
    }

    /// Get the reachable object positions for a commit at the given index position.
    ///
    /// Returns a set of index positions of reachable objects, or None if
    /// no bitmap exists for this commit.
    pub fn reachable_positions(&self, index_position: u32) -> Result<Option<Vec<u32>>, PackError> {
        let entry_idx = match self.commit_positions.iter().position(|&p| p == index_position) {
            Some(idx) => idx,
            None => return Ok(None),
        };

        // Navigate to the correct entry
        let mut pos = self.entries_offset;
        for _ in 0..entry_idx {
            if pos + 6 > self.data.len() {
                return Err(PackError::InvalidIndex("truncated bitmap entry".into()));
            }
            pos += 6; // commit_pos + xor_offset + flags
            for _ in 0..4 {
                pos = skip_ewah_bitmap(&self.data, pos)?;
            }
        }

        // We're at the target entry
        pos += 6; // Skip commit_pos + xor_offset + flags

        // Read all 4 bitmaps and combine them
        let mut result = Vec::new();
        for _ in 0..4 {
            let (bits, new_pos) = decode_ewah_bitmap(&self.data, pos)?;
            for bit_pos in bits {
                result.push(bit_pos);
            }
            pos = new_pos;
        }

        result.sort_unstable();
        result.dedup();
        Ok(Some(result))
    }
}

/// Skip an EWAH bitmap, returning the position after it.
fn skip_ewah_bitmap(data: &[u8], pos: usize) -> Result<usize, PackError> {
    if pos + 8 > data.len() {
        return Err(PackError::InvalidIndex("truncated EWAH header".into()));
    }

    // EWAH header: bit_count(4) + word_count(4)
    let word_count = u32::from_be_bytes([
        data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
    ]) as usize;

    // Each word is 8 bytes
    let bitmap_size = 8 + word_count * 8;
    Ok(pos + bitmap_size)
}

/// Decode an EWAH compressed bitmap into a list of set bit positions.
fn decode_ewah_bitmap(data: &[u8], pos: usize) -> Result<(Vec<u32>, usize), PackError> {
    if pos + 8 > data.len() {
        return Err(PackError::InvalidIndex("truncated EWAH header".into()));
    }

    let bit_count = u32::from_be_bytes([
        data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
    ]);
    let word_count = u32::from_be_bytes([
        data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
    ]) as usize;

    let mut bits = Vec::new();
    let mut bit_pos: u32 = 0;
    let mut wpos = pos + 8;

    let mut words_remaining = word_count;

    while words_remaining > 0 {
        if wpos + 8 > data.len() {
            return Err(PackError::InvalidIndex("truncated EWAH data".into()));
        }

        // Read run-length header word
        let rlw = u64::from_be_bytes([
            data[wpos], data[wpos + 1], data[wpos + 2], data[wpos + 3],
            data[wpos + 4], data[wpos + 5], data[wpos + 6], data[wpos + 7],
        ]);
        wpos += 8;
        words_remaining -= 1;

        // RLW format: bit 0 = fill bit, bits 1-32 = run length, bits 33-63 = literal count
        let fill_bit = (rlw & 1) != 0;
        let run_length = ((rlw >> 1) & 0xFFFF_FFFF) as u32;
        let literal_count = (rlw >> 33) as u32;

        // Process fill run
        if fill_bit {
            for _ in 0..run_length * 64 {
                if bit_pos < bit_count {
                    bits.push(bit_pos);
                }
                bit_pos += 1;
            }
        } else {
            bit_pos += run_length * 64;
        }

        // Process literal words
        for _ in 0..literal_count {
            if wpos + 8 > data.len() || words_remaining == 0 {
                return Err(PackError::InvalidIndex("truncated EWAH literal".into()));
            }
            let word = u64::from_be_bytes([
                data[wpos], data[wpos + 1], data[wpos + 2], data[wpos + 3],
                data[wpos + 4], data[wpos + 5], data[wpos + 6], data[wpos + 7],
            ]);
            wpos += 8;
            words_remaining -= 1;

            for bit in 0..64 {
                if word & (1u64 << bit) != 0 && bit_pos < bit_count {
                    bits.push(bit_pos);
                }
                bit_pos += 1;
            }
        }
    }

    Ok((bits, wpos))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Build a minimal synthetic bitmap file for testing.
    fn build_test_bitmap(
        commit_positions: &[u32],
        reachable_bits: &[Vec<u64>], // 4 EWAH bitmaps per commit (commits, trees, blobs, tags)
    ) -> Vec<u8> {
        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(BITMAP_SIGNATURE);
        buf.extend_from_slice(&1u16.to_be_bytes()); // version
        buf.extend_from_slice(&0u16.to_be_bytes()); // flags
        buf.extend_from_slice(&(commit_positions.len() as u32).to_be_bytes());

        // Fake pack checksum
        buf.extend_from_slice(&[0u8; 20]);

        // Entries
        for (i, &commit_pos) in commit_positions.iter().enumerate() {
            buf.extend_from_slice(&commit_pos.to_be_bytes());
            buf.push(0); // xor_offset
            buf.push(0); // flags

            // 4 EWAH bitmaps
            let base_idx = i * 4;
            for j in 0..4 {
                let bitmap_word = if base_idx + j < reachable_bits.len() {
                    &reachable_bits[base_idx + j]
                } else {
                    &vec![]
                };
                write_ewah_bitmap(&mut buf, bitmap_word);
            }
        }

        buf
    }

    /// Write a simple EWAH bitmap (all literals, no runs).
    fn write_ewah_bitmap(buf: &mut Vec<u8>, words: &[u64]) {
        if words.is_empty() {
            // bit_count = 0, word_count = 1 (just the RLW)
            buf.extend_from_slice(&0u32.to_be_bytes());
            buf.extend_from_slice(&1u32.to_be_bytes());
            // RLW: fill=0, run=0, literals=0
            buf.extend_from_slice(&0u64.to_be_bytes());
            return;
        }

        let bit_count = words.len() as u32 * 64;
        let word_count = 1 + words.len() as u32; // RLW + literals

        buf.extend_from_slice(&bit_count.to_be_bytes());
        buf.extend_from_slice(&word_count.to_be_bytes());

        // RLW: fill=0, run=0, literal_count=words.len()
        let rlw: u64 = (words.len() as u64) << 33;
        buf.extend_from_slice(&rlw.to_be_bytes());

        for &word in words {
            buf.extend_from_slice(&word.to_be_bytes());
        }
    }

    #[test]
    fn open_and_query_bitmap() {
        let dir = tempfile::tempdir().unwrap();

        // One commit at position 0, with blob at position 1 reachable
        let bitmap_data = build_test_bitmap(
            &[0],
            &[
                vec![0b1], // commits bitmap: bit 0 set
                vec![],    // trees bitmap: empty
                vec![0b10], // blobs bitmap: bit 1 set
                vec![],    // tags bitmap: empty
            ],
        );

        let path = dir.path().join("test.bitmap");
        std::fs::write(&path, &bitmap_data).unwrap();

        let bm = BitmapIndex::open(&path).unwrap();
        assert_eq!(bm.num_entries(), 1);
        assert!(bm.has_bitmap_for_position(0));
        assert!(!bm.has_bitmap_for_position(99));

        let reachable = bm.reachable_positions(0).unwrap().unwrap();
        assert!(reachable.contains(&0)); // commit itself
        assert!(reachable.contains(&1)); // reachable blob
    }

    #[test]
    fn no_bitmap_for_unknown_position() {
        let dir = tempfile::tempdir().unwrap();
        let bitmap_data = build_test_bitmap(&[5], &[vec![], vec![], vec![], vec![]]);

        let path = dir.path().join("test.bitmap");
        std::fs::write(&path, &bitmap_data).unwrap();

        let bm = BitmapIndex::open(&path).unwrap();
        assert_eq!(bm.reachable_positions(99).unwrap(), None);
    }
}

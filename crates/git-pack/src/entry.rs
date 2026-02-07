//! Pack entry header parsing.

use crate::{PackEntryType, PackError};
use git_hash::{HashAlgorithm, ObjectId};

/// A raw entry read from a packfile (before delta resolution).
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub entry_type: PackEntryType,
    pub uncompressed_size: usize,
    /// Offset to the start of compressed data in the pack.
    pub data_offset: u64,
    /// Number of bytes consumed by the header.
    pub header_size: usize,
}

/// Parse a pack entry header starting at the given position in `data`.
///
/// Returns the entry metadata and the number of bytes consumed.
/// `entry_offset` is the absolute offset of this entry in the pack file
/// (needed for OFS_DELTA base offset computation).
pub fn parse_entry_header(data: &[u8], entry_offset: u64) -> Result<PackEntry, PackError> {
    if data.is_empty() {
        return Err(PackError::CorruptEntry(entry_offset));
    }

    let mut pos = 0;
    let c = data[pos];
    pos += 1;

    // First byte: bits 6-4 = type, bits 3-0 = lower 4 bits of size
    let type_num = (c >> 4) & 0x07;
    let mut size: u64 = (c & 0x0f) as u64;
    let mut shift = 4;

    // Continue reading size bytes while MSB is set
    let mut byte = c;
    while byte & 0x80 != 0 {
        if pos >= data.len() {
            return Err(PackError::CorruptEntry(entry_offset));
        }
        byte = data[pos];
        pos += 1;
        size |= ((byte & 0x7f) as u64) << shift;
        shift += 7;
    }

    // Parse type-specific extra data
    let entry_type = match type_num {
        1 => PackEntryType::Commit,
        2 => PackEntryType::Tree,
        3 => PackEntryType::Blob,
        4 => PackEntryType::Tag,
        6 => {
            // OFS_DELTA: variable-length negative offset
            if pos >= data.len() {
                return Err(PackError::CorruptEntry(entry_offset));
            }
            let mut c = data[pos];
            pos += 1;
            let mut base_offset = (c & 0x7f) as u64;
            while c & 0x80 != 0 {
                if pos >= data.len() {
                    return Err(PackError::CorruptEntry(entry_offset));
                }
                base_offset += 1;
                c = data[pos];
                pos += 1;
                base_offset = (base_offset << 7) + (c & 0x7f) as u64;
            }
            // base_offset is a negative offset from entry_offset
            if base_offset > entry_offset {
                return Err(PackError::CorruptEntry(entry_offset));
            }
            PackEntryType::OfsDelta {
                base_offset: entry_offset - base_offset,
            }
        }
        7 => {
            // REF_DELTA: 20-byte OID of base object
            let hash_len = HashAlgorithm::Sha1.digest_len();
            if pos + hash_len > data.len() {
                return Err(PackError::CorruptEntry(entry_offset));
            }
            let base_oid =
                ObjectId::from_bytes(&data[pos..pos + hash_len], HashAlgorithm::Sha1)
                    .map_err(|_| PackError::CorruptEntry(entry_offset))?;
            pos += hash_len;
            PackEntryType::RefDelta { base_oid }
        }
        _ => {
            return Err(PackError::CorruptEntry(entry_offset));
        }
    };

    Ok(PackEntry {
        entry_type,
        uncompressed_size: size as usize,
        data_offset: entry_offset + pos as u64,
        header_size: pos,
    })
}

/// Encode a pack entry header into bytes.
///
/// Returns the encoded header bytes. For OFS_DELTA and REF_DELTA, the
/// caller must append the delta base reference separately.
pub fn encode_entry_header(type_num: u8, size: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    let mut s = size;

    // First byte: type in bits 6-4, lower 4 bits of size
    let mut c = (type_num << 4) | (s & 0x0f) as u8;
    s >>= 4;

    while s > 0 {
        buf.push(c | 0x80);
        c = (s & 0x7f) as u8;
        s >>= 7;
    }
    buf.push(c);
    buf
}

/// Encode an OFS_DELTA negative offset.
pub fn encode_ofs_delta_offset(offset: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    let mut off = offset;

    buf.push((off & 0x7f) as u8);
    off >>= 7;
    while off > 0 {
        off -= 1;
        buf.push(0x80 | (off & 0x7f) as u8);
        off >>= 7;
    }
    buf.reverse();
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_base_object_header() {
        // Use encode_entry_header to get correct encoding, then parse it back
        let data = encode_entry_header(3, 100); // blob, size 100
        let entry = parse_entry_header(&data, 0).unwrap();
        assert_eq!(entry.entry_type, PackEntryType::Blob);
        assert_eq!(entry.uncompressed_size, 100);
        assert_eq!(entry.header_size, data.len());
        assert_eq!(entry.data_offset, data.len() as u64);
    }

    #[test]
    fn parse_commit_header_small_size() {
        // Commit type (1), size = 5
        // First byte: (1 << 4) | 5 = 0x15, no MSB
        let data = [0x15];
        let entry = parse_entry_header(&data, 0).unwrap();
        assert_eq!(entry.entry_type, PackEntryType::Commit);
        assert_eq!(entry.uncompressed_size, 5);
        assert_eq!(entry.header_size, 1);
    }

    #[test]
    fn encode_header_roundtrip() {
        let header = encode_entry_header(3, 100); // blob, size 100
        let entry = parse_entry_header(&header, 0).unwrap();
        assert_eq!(entry.entry_type, PackEntryType::Blob);
        assert_eq!(entry.uncompressed_size, 100);
    }

    #[test]
    fn encode_header_large_size() {
        let header = encode_entry_header(1, 1_000_000); // commit, 1MB
        let entry = parse_entry_header(&header, 0).unwrap();
        assert_eq!(entry.entry_type, PackEntryType::Commit);
        assert_eq!(entry.uncompressed_size, 1_000_000);
    }

    #[test]
    fn encode_ofs_delta_roundtrip() {
        for offset in [1u64, 127, 128, 255, 256, 1000, 100_000, 1_000_000] {
            let encoded = encode_ofs_delta_offset(offset);
            // Decode it back the same way the parser does
            let mut pos = 0;
            let mut c = encoded[pos];
            pos += 1;
            let mut decoded = (c & 0x7f) as u64;
            while c & 0x80 != 0 {
                decoded += 1;
                c = encoded[pos];
                pos += 1;
                decoded = (decoded << 7) + (c & 0x7f) as u64;
            }
            assert_eq!(decoded, offset, "roundtrip failed for offset {offset}");
        }
    }
}

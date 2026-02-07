//! Index file reading (v2/v3/v4).

use bstr::BString;
use git_hash::{HashAlgorithm, ObjectId};
use git_object::FileMode;

use crate::entry::{EntryFlags, IndexEntry, StatData};
use crate::extensions::tree::CacheTree;
use crate::extensions::{RawExtension, ResolveUndo};
use crate::{Index, IndexError, Stage};

/// Magic bytes at the start of every index file.
const INDEX_SIGNATURE: &[u8; 4] = b"DIRC";

/// Parse an index file from raw bytes.
pub fn parse_index(data: &[u8]) -> Result<Index, IndexError> {
    if data.len() < 12 {
        return Err(IndexError::InvalidHeader("index file too short".into()));
    }

    // Verify checksum first (last 20 bytes)
    verify_checksum(data)?;

    let mut cursor = 0;

    // Parse header
    let sig = &data[cursor..cursor + 4];
    if sig != INDEX_SIGNATURE {
        return Err(IndexError::InvalidHeader(format!(
            "bad signature: expected DIRC, got {:?}",
            sig
        )));
    }
    cursor += 4;

    let version = read_u32(&data[cursor..]);
    cursor += 4;

    if !(2..=4).contains(&version) {
        return Err(IndexError::UnsupportedVersion(version));
    }

    let entry_count = read_u32(&data[cursor..]) as usize;
    cursor += 4;

    // Parse entries
    let content_end = data.len() - 20; // exclude checksum
    let mut entries = Vec::with_capacity(entry_count);
    let mut prev_path = BString::default();

    for _ in 0..entry_count {
        let (entry, new_cursor) = parse_entry(data, cursor, version, &prev_path, content_end)?;
        prev_path = entry.path.clone();
        entries.push(entry);
        cursor = new_cursor;
    }

    // Parse extensions
    let mut cache_tree = None;
    let mut resolve_undo = None;
    let mut unknown_extensions = Vec::new();

    while cursor + 8 <= content_end {
        let sig = &data[cursor..cursor + 4];
        let ext_size = read_u32(&data[cursor + 4..]) as usize;
        cursor += 8;

        if cursor + ext_size > content_end {
            return Err(IndexError::InvalidExtension {
                sig: format!("{:?}", sig),
                reason: "extension data exceeds index bounds".into(),
            });
        }

        let ext_data = &data[cursor..cursor + ext_size];

        match sig {
            b"TREE" => {
                cache_tree = Some(CacheTree::parse(ext_data)?);
            }
            b"REUC" => {
                resolve_undo = Some(ResolveUndo::parse(ext_data)?);
            }
            _ => {
                // Preserve unknown extensions for round-trip
                let mut sig_arr = [0u8; 4];
                sig_arr.copy_from_slice(sig);
                unknown_extensions.push(RawExtension {
                    signature: sig_arr,
                    data: ext_data.to_vec(),
                });
            }
        }

        cursor += ext_size;
    }

    // Read checksum
    let checksum = ObjectId::from_bytes(&data[data.len() - 20..], HashAlgorithm::Sha1)
        .map_err(|_| IndexError::InvalidHeader("invalid checksum".into()))?;

    Ok(Index {
        version,
        entries,
        cache_tree,
        resolve_undo,
        unknown_extensions,
        _checksum: checksum,
    })
}

/// Offset of the flexible data portion in the on-disk cache entry struct.
/// This is: ctime(8) + mtime(8) + dev(4) + ino(4) + mode(4) + uid(4) + gid(4) + size(4) = 40 bytes.
const ONDISK_OFFSET_DATA: usize = 40;

/// SHA-1 hash size.
const SHA1_SIZE: usize = 20;

/// Calculate the on-disk entry size using C git's formula:
/// `((ONDISK_OFFSET_DATA + hash_size + flags_size + name_len + 8) & ~7)`
fn ondisk_entry_size(name_len: usize, has_extended_flags: bool) -> usize {
    let flags_size: usize = if has_extended_flags { 4 } else { 2 };
    let data_size = SHA1_SIZE + flags_size + name_len;
    (ONDISK_OFFSET_DATA + data_size + 8) & !7
}

/// Parse a single cache entry.
fn parse_entry(
    data: &[u8],
    start: usize,
    version: u32,
    prev_path: &BString,
    content_end: usize,
) -> Result<(IndexEntry, usize), IndexError> {
    let mut cursor = start;

    if cursor + 62 > content_end {
        return Err(IndexError::InvalidEntry {
            offset: start,
            reason: "entry too short".into(),
        });
    }

    // Stat data (40 bytes)
    let stat = StatData {
        ctime_secs: read_u32(&data[cursor..]),
        ctime_nsecs: read_u32(&data[cursor + 4..]),
        mtime_secs: read_u32(&data[cursor + 8..]),
        mtime_nsecs: read_u32(&data[cursor + 12..]),
        dev: read_u32(&data[cursor + 16..]),
        ino: read_u32(&data[cursor + 20..]),
        uid: read_u32(&data[cursor + 28..]),
        gid: read_u32(&data[cursor + 32..]),
        size: read_u32(&data[cursor + 36..]),
    };
    let mode_raw = read_u32(&data[cursor + 24..]);
    cursor += 40;

    // OID (20 bytes for SHA-1)
    let oid = ObjectId::from_bytes(&data[cursor..cursor + 20], HashAlgorithm::Sha1)
        .map_err(|_| IndexError::InvalidEntry {
            offset: start,
            reason: "invalid OID".into(),
        })?;
    cursor += 20;

    // Flags (16 bits)
    let flags_raw = read_u16(&data[cursor..]);
    cursor += 2;

    let assume_valid = (flags_raw & 0x8000) != 0;
    let extended_flag = (flags_raw & 0x4000) != 0;
    let stage_bits = ((flags_raw >> 12) & 0x03) as u8;
    let _name_len_field = (flags_raw & 0x0FFF) as usize;

    let stage = Stage::from_u8(stage_bits).map_err(|_| IndexError::InvalidEntry {
        offset: start,
        reason: format!("invalid stage: {stage_bits}"),
    })?;

    // Extended flags (v3+, only if extended_flag is set)
    let mut intent_to_add = false;
    let mut skip_worktree = false;

    if extended_flag {
        if version < 3 {
            return Err(IndexError::InvalidEntry {
                offset: start,
                reason: "extended flags in v2 index".into(),
            });
        }
        if cursor + 2 > content_end {
            return Err(IndexError::InvalidEntry {
                offset: start,
                reason: "truncated extended flags".into(),
            });
        }
        let ext_flags = read_u16(&data[cursor..]);
        cursor += 2;

        intent_to_add = (ext_flags & 0x2000) != 0;
        skip_worktree = (ext_flags & 0x4000) != 0;
    }

    // Path
    let path = if version == 4 {
        // v4: prefix compression (no padding)
        parse_v4_path(data, &mut cursor, prev_path, content_end)?
    } else {
        // v2/v3: NUL-terminated path
        let path_start = cursor;
        let nul_pos = data[path_start..content_end]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| IndexError::InvalidEntry {
                offset: start,
                reason: "missing NUL in path".into(),
            })?;
        let path = BString::from(&data[path_start..path_start + nul_pos]);
        let actual_name_len = nul_pos; // length of path bytes (without NUL)

        // Calculate total entry size using C git formula and advance cursor
        let entry_size = ondisk_entry_size(actual_name_len, extended_flag);
        cursor = start + entry_size;

        // Clamp to content_end
        if cursor > content_end {
            cursor = content_end;
        }

        path
    };

    let mode = FileMode::from_raw(mode_raw);
    let flags = EntryFlags {
        assume_valid,
        intent_to_add,
        skip_worktree,
    };

    let entry = IndexEntry {
        path,
        oid,
        mode,
        stage,
        stat,
        flags,
    };

    Ok((entry, cursor))
}

/// Parse v4 path with prefix compression.
fn parse_v4_path(
    data: &[u8],
    cursor: &mut usize,
    prev_path: &BString,
    content_end: usize,
) -> Result<BString, IndexError> {
    // Read prefix-strip length as variable-length integer
    let (strip_len, bytes_read) = read_varint(&data[*cursor..content_end]);
    *cursor += bytes_read;

    // Find NUL-terminated suffix
    let suffix_start = *cursor;
    let nul_pos = data[suffix_start..content_end]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| IndexError::InvalidEntry {
            offset: suffix_start,
            reason: "missing NUL in v4 path suffix".into(),
        })?;

    let suffix = &data[suffix_start..suffix_start + nul_pos];
    *cursor = suffix_start + nul_pos + 1; // past NUL, no padding in v4

    // Reconstruct path: keep prefix, append suffix
    let keep = prev_path.len().saturating_sub(strip_len);
    let mut path = BString::from(&prev_path[..keep]);
    path.extend_from_slice(suffix);

    Ok(path)
}

/// Read a variable-length integer (used by v4 path compression).
fn read_varint(data: &[u8]) -> (usize, usize) {
    let mut value: usize = 0;
    let mut shift = 0;
    let mut i = 0;

    loop {
        if i >= data.len() {
            break;
        }
        let byte = data[i];
        i += 1;

        value |= ((byte & 0x7F) as usize) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }

    (value, i)
}

/// Verify the SHA-1 checksum of the index file.
fn verify_checksum(data: &[u8]) -> Result<(), IndexError> {
    if data.len() < 20 {
        return Err(IndexError::ChecksumMismatch);
    }

    let content = &data[..data.len() - 20];
    let stored_checksum = &data[data.len() - 20..];

    let computed = git_hash::hasher::Hasher::digest(HashAlgorithm::Sha1, content)
        .map_err(|_| IndexError::ChecksumMismatch)?;

    if computed.as_bytes() != stored_checksum {
        return Err(IndexError::ChecksumMismatch);
    }

    Ok(())
}

fn read_u32(data: &[u8]) -> u32 {
    u32::from_be_bytes([data[0], data[1], data[2], data[3]])
}

fn read_u16(data: &[u8]) -> u16 {
    u16::from_be_bytes([data[0], data[1]])
}

//! Commit-graph file parsing.

use std::path::Path;

use git_hash::ObjectId;
use memmap2::Mmap;

use super::*;

/// Open and parse a commit-graph file.
pub(crate) fn open_commit_graph(path: &Path) -> Result<CommitGraph, RevWalkError> {
    let file = std::fs::File::open(path).map_err(RevWalkError::Io)?;
    let data = unsafe { Mmap::map(&file) }.map_err(RevWalkError::Io)?;

    if data.len() < 8 {
        return Err(RevWalkError::InvalidCommitGraph("file too small".into()));
    }

    // Verify signature
    if &data[0..4] != COMMIT_GRAPH_SIGNATURE {
        return Err(RevWalkError::InvalidCommitGraph(
            "invalid signature".into(),
        ));
    }

    // Version byte
    let version = data[4];
    if version != 1 {
        return Err(RevWalkError::InvalidCommitGraph(format!(
            "unsupported version: {}",
            version
        )));
    }

    // Hash version: 1 = SHA-1 (20 bytes), 2 = SHA-256 (32 bytes)
    let hash_version = data[5];
    let hash_len = match hash_version {
        1 => 20,
        2 => 32,
        _ => {
            return Err(RevWalkError::InvalidCommitGraph(format!(
                "unsupported hash version: {}",
                hash_version
            )))
        }
    };

    // Number of chunks
    let num_chunks = data[6] as usize;

    // Parse chunk table of contents (starts at offset 8).
    // Each entry: 4-byte chunk ID + 8-byte offset.
    let toc_start = 8;
    let toc_entry_size = 12;

    if data.len() < toc_start + (num_chunks + 1) * toc_entry_size {
        return Err(RevWalkError::InvalidCommitGraph(
            "truncated chunk TOC".into(),
        ));
    }

    let mut oid_fanout_offset: Option<usize> = None;
    let mut oid_lookup_offset: Option<usize> = None;
    let mut commit_data_offset: Option<usize> = None;
    let mut extra_edges_offset: Option<usize> = None;

    for i in 0..num_chunks {
        let entry_offset = toc_start + i * toc_entry_size;
        let chunk_id = u32::from_be_bytes([
            data[entry_offset],
            data[entry_offset + 1],
            data[entry_offset + 2],
            data[entry_offset + 3],
        ]);
        let offset = u64::from_be_bytes([
            data[entry_offset + 4],
            data[entry_offset + 5],
            data[entry_offset + 6],
            data[entry_offset + 7],
            data[entry_offset + 8],
            data[entry_offset + 9],
            data[entry_offset + 10],
            data[entry_offset + 11],
        ]) as usize;

        match chunk_id {
            CHUNK_OID_FANOUT => oid_fanout_offset = Some(offset),
            CHUNK_OID_LOOKUP => oid_lookup_offset = Some(offset),
            CHUNK_COMMIT_DATA => commit_data_offset = Some(offset),
            CHUNK_EXTRA_EDGES => extra_edges_offset = Some(offset),
            _ => {} // Unknown chunks are ignored per spec.
        }
    }

    let oid_fanout_offset = oid_fanout_offset.ok_or_else(|| {
        RevWalkError::InvalidCommitGraph("missing OID Fanout chunk".into())
    })?;
    let oid_lookup_offset = oid_lookup_offset.ok_or_else(|| {
        RevWalkError::InvalidCommitGraph("missing OID Lookup chunk".into())
    })?;
    let commit_data_offset = commit_data_offset.ok_or_else(|| {
        RevWalkError::InvalidCommitGraph("missing Commit Data chunk".into())
    })?;

    // Read num_commits from the last entry of the fanout table (256 entries * 4 bytes each).
    let fanout_last = oid_fanout_offset + 255 * 4;
    if data.len() < fanout_last + 4 {
        return Err(RevWalkError::InvalidCommitGraph(
            "truncated fanout table".into(),
        ));
    }
    let num_commits = u32::from_be_bytes([
        data[fanout_last],
        data[fanout_last + 1],
        data[fanout_last + 2],
        data[fanout_last + 3],
    ]);

    Ok(CommitGraph {
        data,
        num_commits,
        oid_lookup_offset,
        commit_data_offset,
        extra_edges_offset,
        hash_len,
    })
}

/// Look up a commit in the graph by OID using binary search.
pub(crate) fn lookup_commit(graph: &CommitGraph, oid: &ObjectId) -> Option<CommitGraphEntry> {
    let pos = find_oid_position(graph, oid)?;
    read_commit_data(graph, pos)
}

/// Get the OID at a given position index.
pub(crate) fn oid_at_position(graph: &CommitGraph, pos: u32) -> Option<ObjectId> {
    let offset = graph.oid_lookup_offset + (pos as usize) * graph.hash_len;
    if offset + graph.hash_len > graph.data.len() {
        return None;
    }
    let bytes = &graph.data[offset..offset + graph.hash_len];
    let algo = if graph.hash_len == 20 {
        git_hash::HashAlgorithm::Sha1
    } else {
        git_hash::HashAlgorithm::Sha256
    };
    ObjectId::from_bytes(bytes, algo).ok()
}

/// Binary search for an OID in the lookup table.
fn find_oid_position(graph: &CommitGraph, oid: &ObjectId) -> Option<u32> {
    let hash_bytes = oid.as_bytes();
    let hash_len = graph.hash_len;

    // Use first byte for fanout narrowing.
    let _first_byte = hash_bytes[0] as usize;

    // Read fanout bounds.
    // The fanout table is at oid_lookup_offset - 256*4 (actually it's a separate chunk).
    // We need to find the fanout offset. Since we stored oid_lookup and commit_data,
    // the fanout is at oid_lookup_offset - (num_commits * hash_len would be after lookup...)
    // Actually, the fanout table is a separate chunk. We need its offset too.
    // For now, we do a linear scan of the OID lookup table.
    // TODO: Use fanout for O(log n) binary search.

    let mut lo: u32 = 0;
    let mut hi: u32 = graph.num_commits;

    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let offset = graph.oid_lookup_offset + (mid as usize) * hash_len;
        if offset + hash_len > graph.data.len() {
            return None;
        }
        let entry_bytes = &graph.data[offset..offset + hash_len];

        match entry_bytes.cmp(hash_bytes) {
            std::cmp::Ordering::Equal => return Some(mid),
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
        }
    }

    None
}

/// Read commit data at the given position.
fn read_commit_data(graph: &CommitGraph, pos: u32) -> Option<CommitGraphEntry> {
    let hash_len = graph.hash_len;
    // Each commit data entry is: hash_len (tree OID) + 4 (parent1) + 4 (parent2)
    //   + 4 (generation + top bits of date) + 4 (bottom 32 bits of date)
    let entry_size = hash_len + 16;
    let offset = graph.commit_data_offset + (pos as usize) * entry_size;

    if offset + entry_size > graph.data.len() {
        return None;
    }

    // Tree OID
    let tree_bytes = &graph.data[offset..offset + hash_len];
    let algo = if hash_len == 20 {
        git_hash::HashAlgorithm::Sha1
    } else {
        git_hash::HashAlgorithm::Sha256
    };
    let tree_oid = ObjectId::from_bytes(tree_bytes, algo).ok()?;

    // Parent 1 (4 bytes, big-endian index or PARENT_NONE)
    let p1_offset = offset + hash_len;
    let parent1 = u32::from_be_bytes([
        graph.data[p1_offset],
        graph.data[p1_offset + 1],
        graph.data[p1_offset + 2],
        graph.data[p1_offset + 3],
    ]);

    // Parent 2 (4 bytes)
    let p2_offset = p1_offset + 4;
    let parent2 = u32::from_be_bytes([
        graph.data[p2_offset],
        graph.data[p2_offset + 1],
        graph.data[p2_offset + 2],
        graph.data[p2_offset + 3],
    ]);

    // Generation + date top bits (4 bytes)
    let gen_date_offset = p2_offset + 4;
    let gen_date = u32::from_be_bytes([
        graph.data[gen_date_offset],
        graph.data[gen_date_offset + 1],
        graph.data[gen_date_offset + 2],
        graph.data[gen_date_offset + 3],
    ]);

    // Date bottom 32 bits
    let date_low_offset = gen_date_offset + 4;
    let date_low = u32::from_be_bytes([
        graph.data[date_low_offset],
        graph.data[date_low_offset + 1],
        graph.data[date_low_offset + 2],
        graph.data[date_low_offset + 3],
    ]);

    // Generation number is top 30 bits of gen_date.
    let generation = gen_date >> 2;

    // Commit time: top 2 bits from gen_date + bottom 32 bits from date_low.
    let date_high = ((gen_date & 0x3) as u64) << 32;
    let commit_time = (date_high | date_low as u64) as i64;

    // Resolve parent OIDs.
    const PARENT_NONE: u32 = 0x7000_0000;
    const PARENT_EXTRA_EDGE: u32 = 0x8000_0000;

    let mut parent_oids = Vec::new();

    if parent1 != PARENT_NONE {
        if let Some(oid) = graph.oid_at(parent1) {
            parent_oids.push(oid);
        }
    }

    if parent2 != PARENT_NONE {
        if parent2 & PARENT_EXTRA_EDGE != 0 {
            // Octopus merge: follow the extra edge list.
            let extra_idx = (parent2 & !PARENT_EXTRA_EDGE) as usize;
            if let Some(extra_offset) = graph.extra_edges_offset {
                let mut idx = extra_idx;
                loop {
                    let edge_offset = extra_offset + idx * 4;
                    if edge_offset + 4 > graph.data.len() {
                        break;
                    }
                    let edge_val = u32::from_be_bytes([
                        graph.data[edge_offset],
                        graph.data[edge_offset + 1],
                        graph.data[edge_offset + 2],
                        graph.data[edge_offset + 3],
                    ]);
                    let is_last = edge_val & 0x8000_0000 != 0;
                    let parent_idx = edge_val & 0x7FFF_FFFF;
                    if let Some(oid) = graph.oid_at(parent_idx) {
                        parent_oids.push(oid);
                    }
                    if is_last {
                        break;
                    }
                    idx += 1;
                }
            }
        } else if let Some(oid) = graph.oid_at(parent2) {
            parent_oids.push(oid);
        }
    }

    Some(CommitGraphEntry {
        tree_oid,
        parent_oids,
        generation,
        commit_time,
    })
}

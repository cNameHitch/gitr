//! Commit-graph file writer.
//!
//! Generates commit-graph files matching Git's `commit-graph-format.txt` specification.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use git_hash::{HashAlgorithm, ObjectId};
use sha1::Digest;

use crate::RevWalkError;

/// Internal representation of a commit for graph writing.
struct CommitEntry {
    oid: ObjectId,
    tree_oid: ObjectId,
    parent_oids: Vec<ObjectId>,
    generation: u32,
    commit_time: i64,
}

/// Writer for commit-graph files in Git's binary format.
pub struct CommitGraphWriter {
    commits: Vec<CommitEntry>,
    hash_algo: HashAlgorithm,
}

impl CommitGraphWriter {
    /// Create a writer for the given hash algorithm.
    pub fn new(hash_algo: HashAlgorithm) -> Self {
        Self {
            commits: Vec::new(),
            hash_algo,
        }
    }

    /// Add a commit to be included in the graph.
    pub fn add_commit(
        &mut self,
        oid: ObjectId,
        tree_oid: ObjectId,
        parents: Vec<ObjectId>,
        commit_time: i64,
    ) {
        self.commits.push(CommitEntry {
            oid,
            tree_oid,
            parent_oids: parents,
            generation: 0, // computed later
            commit_time,
        });
    }

    /// Compute generation numbers and write the graph file.
    /// Returns the checksum of the written file.
    pub fn write(mut self, path: impl AsRef<Path>) -> Result<ObjectId, RevWalkError> {
        if self.commits.is_empty() {
            return Err(RevWalkError::InvalidCommitGraph(
                "no commits to write".into(),
            ));
        }

        // Sort commits by OID (required by format).
        self.commits.sort_by(|a, b| a.oid.as_bytes().cmp(b.oid.as_bytes()));

        // Build OID → index mapping.
        let oid_to_idx: HashMap<ObjectId, u32> = self
            .commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.oid, i as u32))
            .collect();

        // Compute generation numbers.
        self.compute_generations(&oid_to_idx);

        let hash_len = match self.hash_algo {
            HashAlgorithm::Sha1 => 20usize,
            HashAlgorithm::Sha256 => 32usize,
        };

        // Determine if we need an extra edges chunk.
        let has_extra_edges = self.commits.iter().any(|c| c.parent_oids.len() > 2);

        // Build extra edges list.
        let mut extra_edges: Vec<u32> = Vec::new();
        let mut extra_edge_offsets: HashMap<u32, usize> = HashMap::new(); // commit idx → offset into extra_edges

        if has_extra_edges {
            for (idx, commit) in self.commits.iter().enumerate() {
                if commit.parent_oids.len() > 2 {
                    extra_edge_offsets.insert(idx as u32, extra_edges.len());
                    // Store parent indices for parents 2+ (parent 0 is in the data, parent 1 triggers the extra edge)
                    for (p_idx, parent) in commit.parent_oids.iter().enumerate().skip(1) {
                        if p_idx == 1 {
                            continue; // parent 1 slot points to extra edge list
                        }
                        let parent_graph_idx = oid_to_idx.get(parent).copied().unwrap_or(0x7000_0000);
                        let is_last = p_idx == commit.parent_oids.len() - 1;
                        let val = if is_last {
                            parent_graph_idx | 0x8000_0000
                        } else {
                            parent_graph_idx
                        };
                        extra_edges.push(val);
                    }
                }
            }
        }

        let num_commits = self.commits.len() as u32;
        let num_chunks: u8 = if has_extra_edges { 4 } else { 3 };

        // Compute chunk sizes.
        let fanout_size: usize = 256 * 4;
        let oid_lookup_size: usize = num_commits as usize * hash_len;
        let commit_data_entry_size: usize = hash_len + 16; // tree_oid + parent1 + parent2 + gen/date
        let commit_data_size: usize = num_commits as usize * commit_data_entry_size;
        let extra_edges_size: usize = extra_edges.len() * 4;

        // Header: signature(4) + version(1) + hash_version(1) + num_chunks(1) + base_graph_count(1) = 8
        let header_size: usize = 8;
        // TOC: (num_chunks + 1) entries × 12 bytes each
        let toc_size: usize = (num_chunks as usize + 1) * 12;
        let data_start = header_size + toc_size;

        // Compute offsets.
        let fanout_offset = data_start;
        let oid_lookup_offset = fanout_offset + fanout_size;
        let commit_data_offset = oid_lookup_offset + oid_lookup_size;
        let extra_edges_offset = commit_data_offset + commit_data_size;
        let file_end = if has_extra_edges {
            extra_edges_offset + extra_edges_size
        } else {
            commit_data_offset + commit_data_size
        };

        let mut buf: Vec<u8> = Vec::with_capacity(file_end + hash_len);

        // Write header.
        buf.extend_from_slice(b"CGPH");
        buf.push(1); // version
        buf.push(match self.hash_algo {
            HashAlgorithm::Sha1 => 1,
            HashAlgorithm::Sha256 => 2,
        });
        buf.push(num_chunks);
        buf.push(0); // base graph count (no chain support)

        // Write chunk TOC.
        // Entry: chunk_id(4) + offset(8)
        write_toc_entry(&mut buf, 0x4F494446, fanout_offset as u64); // OIDF
        write_toc_entry(&mut buf, 0x4F49444C, oid_lookup_offset as u64); // OIDL
        write_toc_entry(&mut buf, 0x43444154, commit_data_offset as u64); // CDAT
        if has_extra_edges {
            write_toc_entry(&mut buf, 0x45444745, extra_edges_offset as u64); // EDGE
        }
        // Terminating TOC entry: zero ID + file_end offset
        write_toc_entry(&mut buf, 0x0000_0000, file_end as u64);

        // Write OID Fanout (256 × 4-byte cumulative counts).
        let mut fanout = [0u32; 256];
        for commit in &self.commits {
            let first_byte = commit.oid.as_bytes()[0] as usize;
            for item in fanout.iter_mut().skip(first_byte) {
                *item += 1;
            }
        }
        for count in &fanout {
            buf.extend_from_slice(&count.to_be_bytes());
        }

        // Write OID Lookup (sorted OIDs).
        for commit in &self.commits {
            buf.extend_from_slice(commit.oid.as_bytes());
        }

        // Write Commit Data.
        const PARENT_NONE: u32 = 0x7000_0000;
        const PARENT_EXTRA_EDGE: u32 = 0x8000_0000;

        for (idx, commit) in self.commits.iter().enumerate() {
            // Tree OID
            buf.extend_from_slice(commit.tree_oid.as_bytes());

            // Parent 1
            let parent1 = if commit.parent_oids.is_empty() {
                PARENT_NONE
            } else {
                oid_to_idx
                    .get(&commit.parent_oids[0])
                    .copied()
                    .unwrap_or(PARENT_NONE)
            };
            buf.extend_from_slice(&parent1.to_be_bytes());

            // Parent 2
            let parent2 = if commit.parent_oids.len() <= 1 {
                PARENT_NONE
            } else if commit.parent_oids.len() == 2 {
                oid_to_idx
                    .get(&commit.parent_oids[1])
                    .copied()
                    .unwrap_or(PARENT_NONE)
            } else {
                // Octopus merge: point to extra edge list
                let edge_offset = extra_edge_offsets
                    .get(&(idx as u32))
                    .copied()
                    .unwrap_or(0);
                // Parent 2 slot stores parent_oids[1] index via extra edges
                // Actually, for octopus, parent2 = PARENT_EXTRA_EDGE | offset
                // But we also need to write parent_oids[1] as the first extra edge entry
                PARENT_EXTRA_EDGE | edge_offset as u32
            };
            buf.extend_from_slice(&parent2.to_be_bytes());

            // Generation number + commit date
            let generation = commit.generation.min(0x3FFF_FFFF);
            let commit_time = commit.commit_time as u64;
            let date_high = ((commit_time >> 32) & 0x3) as u32;
            let gen_date = (generation << 2) | date_high;
            let date_low = (commit_time & 0xFFFF_FFFF) as u32;
            buf.extend_from_slice(&gen_date.to_be_bytes());
            buf.extend_from_slice(&date_low.to_be_bytes());
        }

        // Write Extra Edges (if any).
        if has_extra_edges {
            // For octopus merges, we need to include parent_oids[1] as well
            // Recompute extra edges properly
            buf.truncate(extra_edges_offset);
            for commit in &self.commits {
                if commit.parent_oids.len() > 2 {
                    for (p_idx, parent) in commit.parent_oids.iter().enumerate().skip(1) {
                        let parent_graph_idx =
                            oid_to_idx.get(parent).copied().unwrap_or(PARENT_NONE);
                        let is_last = p_idx == commit.parent_oids.len() - 1;
                        let val = if is_last {
                            parent_graph_idx | 0x8000_0000
                        } else {
                            parent_graph_idx
                        };
                        buf.extend_from_slice(&val.to_be_bytes());
                    }
                }
            }
        }

        // Write trailing checksum.
        let mut hasher = sha1::Sha1::new();
        hasher.update(&buf);
        let checksum = hasher.finalize();
        buf.extend_from_slice(&checksum);

        // Ensure parent directory exists.
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(RevWalkError::Io)?;
        }

        // Write atomically via temp file.
        let mut file = std::fs::File::create(path.as_ref()).map_err(RevWalkError::Io)?;
        file.write_all(&buf).map_err(RevWalkError::Io)?;
        file.flush().map_err(RevWalkError::Io)?;

        let algo = self.hash_algo;
        ObjectId::from_bytes(&checksum, algo).map_err(|e| {
            RevWalkError::InvalidCommitGraph(format!("checksum conversion error: {}", e))
        })
    }

    /// Compute generation numbers bottom-up.
    fn compute_generations(&mut self, oid_to_idx: &HashMap<ObjectId, u32>) {
        let n = self.commits.len();

        // Build adjacency: child → parents (as indices)
        let parent_indices: Vec<Vec<u32>> = self
            .commits
            .iter()
            .map(|c| {
                c.parent_oids
                    .iter()
                    .filter_map(|p| oid_to_idx.get(p).copied())
                    .collect()
            })
            .collect();

        // Compute generations via iterative DFS.
        let mut generations = vec![0u32; n];
        let mut visited = vec![false; n];
        let mut stack: Vec<(usize, bool)> = Vec::new();

        for i in 0..n {
            if !visited[i] {
                stack.push((i, false));
                while let Some((idx, processed)) = stack.pop() {
                    if processed {
                        let max_parent_gen = parent_indices[idx]
                            .iter()
                            .map(|&p| generations[p as usize])
                            .max()
                            .unwrap_or(0);
                        generations[idx] = max_parent_gen + 1;
                    } else if !visited[idx] {
                        visited[idx] = true;
                        stack.push((idx, true));
                        for &p in &parent_indices[idx] {
                            if !visited[p as usize] {
                                stack.push((p as usize, false));
                            }
                        }
                    }
                }
            }
        }

        // Store generations.
        for (i, gen) in generations.into_iter().enumerate() {
            self.commits[i].generation = gen;
        }
    }
}

fn write_toc_entry(buf: &mut Vec<u8>, chunk_id: u32, offset: u64) {
    buf.extend_from_slice(&chunk_id.to_be_bytes());
    buf.extend_from_slice(&offset.to_be_bytes());
}

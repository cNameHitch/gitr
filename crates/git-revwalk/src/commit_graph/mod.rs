//! Commit-graph file reader for accelerated commit access.
//!
//! The commit-graph file provides fast access to commit metadata (parents,
//! generation numbers, commit dates) without parsing pack objects.
//!
//! Format: see Documentation/technical/commit-graph-format.txt in git source.

mod parse;
pub mod write;

use std::path::Path;

use git_hash::ObjectId;
use git_repository::Repository;
use memmap2::Mmap;

use crate::RevWalkError;

/// A parsed commit-graph file providing fast commit access.
pub struct CommitGraph {
    /// Memory-mapped commit-graph data.
    data: Mmap,
    /// Number of commits in the graph.
    num_commits: u32,
    /// Offset to the OID Fanout chunk (256 × 4-byte cumulative counts).
    oid_fanout_offset: usize,
    /// Offset to the OID Lookup chunk.
    oid_lookup_offset: usize,
    /// Offset to the Commit Data chunk.
    commit_data_offset: usize,
    /// Offset to the Extra Edge List chunk (for octopus merges).
    extra_edges_offset: Option<usize>,
    /// OID hash length (20 for SHA-1, 32 for SHA-256).
    hash_len: usize,
}

/// An entry from the commit-graph.
#[derive(Debug, Clone)]
pub struct CommitGraphEntry {
    /// OID of the root tree.
    pub tree_oid: ObjectId,
    /// Parent indices (into the commit-graph) or OIDs resolved.
    pub parent_oids: Vec<ObjectId>,
    /// Generation number (topological level + 1). 0 means unknown.
    pub generation: u32,
    /// Commit timestamp (seconds since epoch).
    pub commit_time: i64,
}

/// Maximum generation number for V1 format.
#[allow(dead_code)]
const GENERATION_NUMBER_V1_MAX: u32 = 0x3FFF_FFFF;

/// Commit-graph file signature: "CGPH"
const COMMIT_GRAPH_SIGNATURE: &[u8; 4] = b"CGPH";

/// Chunk IDs
const CHUNK_OID_FANOUT: u32 = 0x4F494446; // "OIDF"
const CHUNK_OID_LOOKUP: u32 = 0x4F49444C; // "OIDL"
const CHUNK_COMMIT_DATA: u32 = 0x43444154; // "CDAT"
const CHUNK_EXTRA_EDGES: u32 = 0x45444745; // "EDGE"

impl CommitGraph {
    /// Open a commit-graph file from a path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RevWalkError> {
        parse::open_commit_graph(path.as_ref())
    }

    /// Try to open the commit-graph from a repository.
    /// Returns Ok(graph) if found, or an error if the file doesn't exist or is invalid.
    pub fn open_from_repo(repo: &Repository) -> Result<Self, RevWalkError> {
        // Try single-file commit-graph first
        let objects_dir = repo.odb().objects_dir();
        let single_path = objects_dir.join("info").join("commit-graph");
        if single_path.exists() {
            return Self::open(&single_path);
        }

        // Try chain of commit-graph files
        let chain_dir = objects_dir.join("info").join("commit-graphs");
        if chain_dir.is_dir() {
            // Read the chain file to find the latest graph
            let chain_file = chain_dir.join("commit-graph-chain");
            if chain_file.exists() {
                let content = std::fs::read_to_string(&chain_file)
                    .map_err(RevWalkError::Io)?;
                // The last line is the most recent graph
                if let Some(hash) = content.lines().last() {
                    let hash = hash.trim();
                    let graph_path = chain_dir.join(format!("graph-{}.graph", hash));
                    if graph_path.exists() {
                        return Self::open(&graph_path);
                    }
                }
            }
        }

        Err(RevWalkError::InvalidCommitGraph(
            "no commit-graph found".into(),
        ))
    }

    /// Look up a commit in the graph by OID.
    pub fn lookup(&self, oid: &ObjectId) -> Option<CommitGraphEntry> {
        parse::lookup_commit(self, oid)
    }

    /// Fast existence check without full entry parsing.
    pub fn contains(&self, oid: &ObjectId) -> bool {
        parse::find_oid_position(self, oid).is_some()
    }

    /// Validate checksum integrity of the commit-graph file.
    pub fn verify(&self) -> Result<(), RevWalkError> {
        use sha1::Digest;

        if self.data.len() < self.hash_len {
            return Err(RevWalkError::InvalidCommitGraph(
                "file too small for checksum".into(),
            ));
        }

        let content_len = self.data.len() - self.hash_len;
        let stored_checksum = &self.data[content_len..];

        let mut hasher = sha1::Sha1::new();
        hasher.update(&self.data[..content_len]);
        let computed = hasher.finalize();

        if computed.as_slice() != stored_checksum {
            return Err(RevWalkError::InvalidCommitGraph(
                "checksum mismatch".into(),
            ));
        }

        Ok(())
    }

    /// Get the number of commits in the graph.
    pub fn num_commits(&self) -> u32 {
        self.num_commits
    }

    /// Get the OID at a given position index.
    fn oid_at(&self, pos: u32) -> Option<ObjectId> {
        parse::oid_at_position(self, pos)
    }
}

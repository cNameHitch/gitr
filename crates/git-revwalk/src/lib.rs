//! Revision walking: commit traversal, revision ranges, merge-base computation,
//! commit-graph acceleration, pretty-printing, and object listing.
//!
//! This crate provides the core `RevWalk` iterator for traversing commit history
//! in various orders (chronological, topological, author-date, reverse), revision
//! range parsing (`A..B`, `A...B`, `^A B`), merge-base computation via the paint
//! algorithm, optional commit-graph acceleration, pretty-print formatting for
//! `git log`, and reachable object enumeration for pack generation.

mod walk;
mod range;
mod merge_base;
mod commit_graph;
mod pretty;
mod graph_draw;
mod objects;
mod filter;

pub use walk::{RevWalk, SortOrder, WalkOptions};
pub use range::{RevisionRange, resolve_revision};
pub use merge_base::{merge_base, merge_base_one, is_ancestor};
pub use commit_graph::{CommitGraph, CommitGraphEntry};
pub use commit_graph::write::CommitGraphWriter;
pub use pretty::{format_commit, format_builtin, FormatOptions, BuiltinFormat};
pub use graph_draw::GraphDrawer;
pub use objects::list_objects;
pub use filter::ObjectFilter;

use git_hash::ObjectId;

/// Errors produced by revision walking operations.
#[derive(Debug, thiserror::Error)]
pub enum RevWalkError {
    #[error("invalid revision: {0}")]
    InvalidRevision(String),

    #[error("commit not found: {0}")]
    CommitNotFound(ObjectId),

    #[error("object is not a commit: {0}")]
    NotACommit(ObjectId),

    #[error("invalid commit-graph: {0}")]
    InvalidCommitGraph(String),

    #[error("no merge base found")]
    NoMergeBase,

    #[error(transparent)]
    Odb(#[from] git_odb::OdbError),

    #[error(transparent)]
    Ref(#[from] git_ref::RefError),

    #[error(transparent)]
    Repo(#[from] git_repository::RepoError),

    #[error(transparent)]
    Object(#[from] git_object::ObjectError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

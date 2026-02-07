//! Index extensions: TREE, REUC, UNTR, and unknown/raw.

pub mod tree;
pub mod resolve_undo;
pub mod untracked;

use bstr::BString;
use git_hash::ObjectId;
use git_object::FileMode;

/// Raw unknown extension (preserved for round-trip).
#[derive(Debug, Clone)]
pub struct RawExtension {
    pub signature: [u8; 4],
    pub data: Vec<u8>,
}

/// Resolve-undo extension (REUC).
#[derive(Debug, Clone)]
pub struct ResolveUndo {
    pub entries: Vec<ResolveUndoEntry>,
}

/// A single resolve-undo entry.
#[derive(Debug, Clone)]
pub struct ResolveUndoEntry {
    pub path: BString,
    pub modes: [Option<FileMode>; 3], // base, ours, theirs
    pub oids: [Option<ObjectId>; 3],
}

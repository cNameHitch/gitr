//! Subtree merge strategy.
//!
//! Matches trees that are at different levels in the hierarchy and adjusts
//! the tree structure before delegating to ORT for the actual merge.

use git_hash::ObjectId;
use git_repository::Repository;

use crate::strategy::ort::OrtStrategy;
use crate::strategy::MergeStrategy;
use crate::{MergeError, MergeOptions, MergeResult};

/// The subtree merge strategy.
///
/// Delegates to ORT after adjusting tree levels. For now, this behaves
/// the same as ORT (subtree shift detection is a future enhancement).
pub struct SubtreeStrategy;

impl MergeStrategy for SubtreeStrategy {
    fn merge(
        &self,
        repo: &mut Repository,
        ours: &ObjectId,
        theirs: &ObjectId,
        base: &ObjectId,
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError> {
        // TODO: Implement subtree shift detection.
        // For now, delegate directly to ORT.
        let ort = OrtStrategy;
        ort.merge(repo, ours, theirs, base, options)
    }
}

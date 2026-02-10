//! Pluggable merge strategies.
//!
//! Provides the [`MergeStrategy`] trait and implementations for ORT (default),
//! ours, and subtree strategies.

pub mod octopus;
pub mod ort;
pub mod ours;
pub mod subtree;

use git_hash::ObjectId;
use git_repository::Repository;

use crate::{MergeError, MergeOptions, MergeResult};

/// Trait for merge strategies.
///
/// Each strategy takes the two commit tips and their merge base, and produces
/// a [`MergeResult`] that is either clean (with a new tree) or conflicted.
pub trait MergeStrategy {
    /// Perform the merge.
    ///
    /// - `ours`: Our commit (current branch tip).
    /// - `theirs`: Their commit (branch being merged).
    /// - `base`: Common ancestor commit.
    fn merge(
        &self,
        repo: &mut Repository,
        ours: &ObjectId,
        theirs: &ObjectId,
        base: &ObjectId,
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError>;
}

/// Dispatch to the appropriate strategy based on options.
pub fn dispatch_merge(
    repo: &mut Repository,
    ours: &ObjectId,
    theirs: &ObjectId,
    base: &ObjectId,
    options: &MergeOptions,
) -> Result<MergeResult, MergeError> {
    use crate::MergeStrategyType;

    match options.strategy {
        MergeStrategyType::Ort | MergeStrategyType::Recursive => {
            let strategy = ort::OrtStrategy;
            strategy.merge(repo, ours, theirs, base, options)
        }
        MergeStrategyType::Ours => {
            let strategy = ours::OursStrategy;
            strategy.merge(repo, ours, theirs, base, options)
        }
        MergeStrategyType::Subtree => {
            let strategy = subtree::SubtreeStrategy;
            strategy.merge(repo, ours, theirs, base, options)
        }
        MergeStrategyType::Octopus => {
            let strategy = octopus::OctopusStrategy;
            strategy.merge(repo, ours, theirs, base, options)
        }
    }
}

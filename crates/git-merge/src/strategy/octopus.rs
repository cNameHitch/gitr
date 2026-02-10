//! Octopus merge strategy.
//!
//! Merges 3+ branches simultaneously. Fails if any merge produces conflicts.

use git_hash::ObjectId;
use git_repository::Repository;

use crate::{MergeError, MergeOptions, MergeResult};
use super::MergeStrategy;

pub struct OctopusStrategy;

impl MergeStrategy for OctopusStrategy {
    fn merge(
        &self,
        repo: &mut Repository,
        ours: &ObjectId,
        theirs: &ObjectId,
        base: &ObjectId,
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError> {
        // Octopus merge for 2 heads falls back to ORT
        let ort = super::ort::OrtStrategy;
        let result = ort.merge(repo, ours, theirs, base, options)?;

        if !result.is_clean {
            return Err(MergeError::Conflict {
                path: bstr::BString::from("octopus merge failed: conflict detected"),
            });
        }

        Ok(result)
    }
}

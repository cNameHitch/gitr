//! "Ours" merge strategy.
//!
//! Always produces a merge result that is identical to the current branch's
//! tree, ignoring the other side entirely.

use git_hash::ObjectId;
use git_object::Object;
use git_repository::Repository;

use crate::strategy::MergeStrategy;
use crate::{MergeError, MergeOptions, MergeResult};

/// The "ours" strategy — always take our tree.
pub struct OursStrategy;

impl MergeStrategy for OursStrategy {
    fn merge(
        &self,
        repo: &mut Repository,
        ours_commit: &ObjectId,
        _theirs_commit: &ObjectId,
        _base_commit: &ObjectId,
        _options: &MergeOptions,
    ) -> Result<MergeResult, MergeError> {
        let odb = repo.odb();

        // Read our commit to get our tree.
        let obj = odb
            .read(ours_commit)?
            .ok_or(MergeError::ObjectNotFound(*ours_commit))?;

        let tree_oid = match obj {
            Object::Commit(c) => c.tree,
            other => {
                return Err(MergeError::UnexpectedObjectType {
                    oid: *ours_commit,
                    expected: "commit",
                    actual: other.object_type().to_string(),
                })
            }
        };

        Ok(MergeResult::clean(tree_oid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ours_strategy_type() {
        // Verify we can construct the strategy.
        let _strategy = OursStrategy;
    }
}

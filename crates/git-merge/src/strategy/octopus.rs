//! Octopus merge strategy.
//!
//! Merges 3+ branches simultaneously. Fails if any merge produces conflicts.
//! Uses iterative approach: merge heads one at a time into an accumulated result,
//! aborting the entire operation on any conflict.

use git_hash::ObjectId;
use git_object::Object;
use git_repository::Repository;

use crate::{MergeError, MergeOptions, MergeResult};
use super::MergeStrategy;

pub struct OctopusStrategy;

impl OctopusStrategy {
    /// Merge multiple heads using the octopus strategy.
    ///
    /// Starting from `ours` (current HEAD), merges each head in `additional_heads`
    /// one at a time. If any step produces a conflict, the entire operation is
    /// aborted (octopus never resolves conflicts, matching git).
    ///
    /// Returns a `MergeResult` whose tree is the final accumulated tree.
    /// The caller is responsible for creating the merge commit with N+1 parents.
    pub fn merge_multi(
        &self,
        repo: &mut Repository,
        ours: &ObjectId,
        additional_heads: &[ObjectId],
        bases: &[ObjectId],
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError> {
        if additional_heads.is_empty() {
            return Err(MergeError::InvalidPatch(
                "octopus merge requires at least one additional head".into(),
            ));
        }

        // For a single additional head, fall back to ORT
        if additional_heads.len() == 1 {
            let base = bases.first().copied().unwrap_or(ObjectId::NULL_SHA1);
            return self.merge(repo, ours, &additional_heads[0], &base, options);
        }

        // Iterative merge: start with ours tree as the accumulated result
        let ort = super::ort::OrtStrategy;
        let mut accumulated_tree = Self::get_tree_oid(repo, ours)?;

        for (i, head) in additional_heads.iter().enumerate() {
            // Find a merge base between accumulated result and next head
            // Use the provided base if available, otherwise use NULL as base
            let base = if i < bases.len() {
                bases[i]
            } else {
                // Try to find a merge base between ours and this head
                match git_revwalk::merge_base_one(repo, ours, head) {
                    Ok(Some(b)) => b,
                    _ => ObjectId::NULL_SHA1,
                }
            };

            // We need to create a virtual commit pointing to the accumulated tree
            // to use the ORT merge. Instead, we use a workaround: write the
            // accumulated tree as a temporary tree object and use it directly.
            // The ORT strategy resolves commits to trees internally, so we need
            // to work with the merge at the tree level.

            // For the first iteration, ours is the real commit; for subsequent
            // iterations we need to use the accumulated tree. Since ORT takes
            // commit OIDs and resolves them to trees, we create a temporary
            // commit object for the accumulated state.
            let ours_for_merge = if i == 0 {
                *ours
            } else {
                // Create a temporary commit with the accumulated tree
                Self::create_temp_commit(repo, &accumulated_tree)?
            };

            let result = ort.merge(repo, &ours_for_merge, head, &base, options)?;

            if !result.is_clean {
                return Err(MergeError::Conflict {
                    path: bstr::BString::from(format!(
                        "octopus merge failed: conflict merging head {}",
                        head.to_hex()
                    )),
                });
            }

            // Update accumulated tree
            if let Some(tree_oid) = result.tree {
                accumulated_tree = tree_oid;
            }
        }

        Ok(MergeResult {
            tree: Some(accumulated_tree),
            is_clean: true,
            conflicts: Vec::new(),
            message: None,
        })
    }

    /// Get the tree OID from a commit.
    fn get_tree_oid(repo: &Repository, commit_oid: &ObjectId) -> Result<ObjectId, MergeError> {
        let obj = repo
            .odb()
            .read(commit_oid)?
            .ok_or(MergeError::ObjectNotFound(*commit_oid))?;
        match obj {
            Object::Commit(c) => Ok(c.tree),
            _ => Err(MergeError::UnexpectedObjectType {
                oid: *commit_oid,
                expected: "commit",
                actual: obj.object_type().to_string(),
            }),
        }
    }

    /// Create a temporary commit object pointing to the given tree.
    fn create_temp_commit(
        repo: &Repository,
        tree_oid: &ObjectId,
    ) -> Result<ObjectId, MergeError> {
        let sig = git_utils::date::Signature {
            name: bstr::BString::from("gitr"),
            email: bstr::BString::from("gitr@temp"),
            date: git_utils::date::GitDate::now(),
        };
        let commit = git_object::Commit {
            tree: *tree_oid,
            parents: Vec::new(),
            author: sig.clone(),
            committer: sig,
            message: bstr::BString::from("octopus merge temporary"),
            encoding: None,
            gpgsig: None,
            extra_headers: Vec::new(),
        };
        let oid = repo.odb().write(&Object::Commit(commit))?;
        Ok(oid)
    }
}

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

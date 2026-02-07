//! Revert implementation.
//!
//! Reverse-applies a commit's changes by using the commit as the merge base
//! and its parent as "theirs", performing a three-way merge.

use git_hash::ObjectId;
use git_object::Object;
use git_repository::Repository;

use crate::strategy::dispatch_merge;
use crate::{MergeError, MergeOptions, MergeResult};

/// Revert a commit on the current branch.
///
/// Uses the commit itself as the merge base and its first parent as "theirs",
/// effectively applying the inverse of the commit's changes.
pub fn revert(
    repo: &mut Repository,
    commit_oid: &ObjectId,
    options: &MergeOptions,
) -> Result<MergeResult, MergeError> {
    let odb = repo.odb();

    // Read the commit to revert.
    let obj = odb
        .read(commit_oid)?
        .ok_or(MergeError::ObjectNotFound(*commit_oid))?;

    let commit = match obj {
        Object::Commit(c) => c,
        other => {
            return Err(MergeError::UnexpectedObjectType {
                oid: *commit_oid,
                expected: "commit",
                actual: other.object_type().to_string(),
            })
        }
    };

    // The commit's first parent is what we want to merge towards.
    let parent = commit
        .parents
        .first()
        .ok_or(MergeError::NoMergeBase)?;

    // "Ours" is the current HEAD.
    let head_oid = repo
        .head_oid()?
        .ok_or(MergeError::NoMergeBase)?;

    // Perform the merge: base=commit, ours=HEAD, theirs=parent
    // This reverses the commit's changes.
    let mut result = dispatch_merge(repo, &head_oid, parent, commit_oid, options)?;

    // Set the revert commit message.
    result.message = Some(format!(
        "Revert \"{}\"\n\nThis reverts commit {}.",
        commit.message.to_string().lines().next().unwrap_or(""),
        commit_oid.to_hex()
    ));

    Ok(result)
}

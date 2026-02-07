//! Cherry-pick implementation.
//!
//! Applies a single commit's changes onto the current branch by treating
//! the commit's parent as the merge base and performing a three-way merge.

use git_hash::ObjectId;
use git_object::Object;
use git_repository::Repository;

use crate::strategy::dispatch_merge;
use crate::{MergeError, MergeOptions, MergeResult};

/// Cherry-pick a commit onto the current branch.
///
/// Uses the commit's first parent as the merge base and the commit itself
/// as "theirs", performing a three-way merge against the current HEAD.
pub fn cherry_pick(
    repo: &mut Repository,
    commit_oid: &ObjectId,
    options: &MergeOptions,
) -> Result<MergeResult, MergeError> {
    let odb = repo.odb();

    // Read the commit to cherry-pick.
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

    // The merge base is the commit's first parent.
    let base = commit
        .parents
        .first()
        .ok_or(MergeError::NoMergeBase)?;

    // "Ours" is the current HEAD.
    let head_oid = repo
        .head_oid()?
        .ok_or(MergeError::NoMergeBase)?;

    // Perform the merge: base=parent, ours=HEAD, theirs=commit
    let mut result = dispatch_merge(repo, &head_oid, commit_oid, base, options)?;

    // Set the commit message from the cherry-picked commit.
    result.message = Some(commit.message.to_string());

    Ok(result)
}

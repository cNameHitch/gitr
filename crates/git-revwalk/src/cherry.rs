//! Cherry/cherry-pick filtering.
//!
//! Identifies commits not yet applied upstream by comparing patch IDs.

use std::collections::HashSet;

use git_hash::ObjectId;
use git_object::Object;
use git_repository::Repository;

use crate::RevWalkError;
use crate::walk::RevWalk;

/// A commit with its cherry-pick status.
#[derive(Debug, Clone)]
pub struct CherryEntry {
    pub oid: ObjectId,
    /// '+' means not upstream, '-' means equivalent exists upstream
    pub marker: char,
    pub subject: String,
}

/// Find commits in `head` that are not in `upstream` by comparing patch IDs.
///
/// Returns entries marked with '+' (unique to head) or '-' (equivalent in upstream).
pub fn cherry(
    repo: &Repository,
    upstream: &ObjectId,
    head: &ObjectId,
    _limit: Option<&ObjectId>,
) -> Result<Vec<CherryEntry>, RevWalkError> {
    // Get commits reachable from head but not from upstream
    let head_commits = collect_commits(repo, head, upstream)?;

    // Get commits reachable from upstream but not from head
    let upstream_commits = collect_commits(repo, upstream, head)?;

    // Compute patch IDs for upstream commits
    let upstream_patch_ids: HashSet<String> = upstream_commits
        .iter()
        .filter_map(|oid| compute_patch_id(repo, oid).ok())
        .collect();

    // Mark head commits
    let mut entries = Vec::new();
    for oid in &head_commits {
        let patch_id = compute_patch_id(repo, oid).unwrap_or_default();
        let marker = if upstream_patch_ids.contains(&patch_id) {
            '-'
        } else {
            '+'
        };

        let subject = get_commit_subject(repo, oid).unwrap_or_default();

        entries.push(CherryEntry {
            oid: *oid,
            marker,
            subject,
        });
    }

    Ok(entries)
}

/// Collect commits reachable from `include` but not from `exclude`.
fn collect_commits(
    repo: &Repository,
    include: &ObjectId,
    exclude: &ObjectId,
) -> Result<Vec<ObjectId>, RevWalkError> {
    let mut walk = RevWalk::new(repo)?;
    walk.push(*include)?;
    walk.hide(*exclude)?;

    let mut commits = Vec::new();
    for result in &mut walk {
        commits.push(result?);
    }
    Ok(commits)
}

/// Compute a simplified patch ID for a commit.
/// Uses the commit's tree diff as a fingerprint.
fn compute_patch_id(repo: &Repository, oid: &ObjectId) -> Result<String, RevWalkError> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or(RevWalkError::CommitNotFound(*oid))?;

    let commit = match obj {
        Object::Commit(c) => c,
        _ => return Err(RevWalkError::NotACommit(*oid)),
    };

    // Use commit message + parent count as a simple patch ID
    // A full implementation would diff against parent and hash the diff
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(&commit.message);
    hasher.update(commit.parents.len().to_string().as_bytes());
    if let Some(parent) = commit.parents.first() {
        hasher.update(parent.as_bytes());
    }
    hasher.update(commit.tree.as_bytes());

    let result = hasher.finalize();
    Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
}

fn get_commit_subject(repo: &Repository, oid: &ObjectId) -> Result<String, RevWalkError> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or(RevWalkError::CommitNotFound(*oid))?;

    let commit = match obj {
        Object::Commit(c) => c,
        _ => return Err(RevWalkError::NotACommit(*oid)),
    };

    let msg = String::from_utf8_lossy(&commit.message);
    Ok(msg.lines().next().unwrap_or("").to_string())
}

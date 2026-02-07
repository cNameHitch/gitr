//! Object listing: enumerate all objects reachable from a set of commits.
//!
//! Used by pack generation, fetch/clone, and garbage collection.

use std::collections::HashSet;

use git_hash::ObjectId;
use git_object::{Object, Tree};
use git_repository::Repository;

use crate::filter::ObjectFilter;
use crate::RevWalkError;

/// List all objects reachable from the given commits, excluding objects
/// reachable from the excluded set.
///
/// Returns OIDs of all commits, trees, and blobs reachable from `include`
/// but not from `exclude`.
pub fn list_objects(
    repo: &Repository,
    include: &[ObjectId],
    exclude: &[ObjectId],
    filter: Option<&ObjectFilter>,
) -> Result<Vec<ObjectId>, RevWalkError> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    // First, collect all objects reachable from excluded commits.
    let mut excluded_objects = HashSet::new();
    for oid in exclude {
        collect_reachable(repo, oid, &mut excluded_objects, None)?;
    }

    // Now collect all objects reachable from included commits,
    // skipping anything in the excluded set.
    for oid in include {
        collect_reachable_filtered(
            repo,
            oid,
            &mut seen,
            &excluded_objects,
            filter,
            &mut result,
        )?;
    }

    Ok(result)
}

/// Collect all objects reachable from a commit (commits, trees, blobs).
fn collect_reachable(
    repo: &Repository,
    start: &ObjectId,
    seen: &mut HashSet<ObjectId>,
    filter: Option<&ObjectFilter>,
) -> Result<(), RevWalkError> {
    let mut stack = vec![*start];

    while let Some(oid) = stack.pop() {
        if !seen.insert(oid) {
            continue;
        }

        let obj = match repo.odb().read(&oid)? {
            Some(obj) => obj,
            None => continue,
        };

        match obj {
            Object::Commit(commit) => {
                // Walk the tree.
                stack.push(commit.tree);
                // Walk parents.
                for parent in &commit.parents {
                    stack.push(*parent);
                }
            }
            Object::Tree(tree) => {
                collect_tree_objects(&tree, &oid, repo, seen, &mut stack, filter)?;
            }
            Object::Blob(_) => {
                // Blob is a leaf; nothing more to walk.
            }
            Object::Tag(tag) => {
                // Peel the tag.
                stack.push(tag.target);
            }
        }
    }

    Ok(())
}

/// Collect reachable objects with exclusion filtering.
fn collect_reachable_filtered(
    repo: &Repository,
    start: &ObjectId,
    seen: &mut HashSet<ObjectId>,
    excluded: &HashSet<ObjectId>,
    filter: Option<&ObjectFilter>,
    result: &mut Vec<ObjectId>,
) -> Result<(), RevWalkError> {
    let mut stack = vec![*start];

    while let Some(oid) = stack.pop() {
        if !seen.insert(oid) || excluded.contains(&oid) {
            continue;
        }

        let obj = match repo.odb().read(&oid)? {
            Some(obj) => obj,
            None => continue,
        };

        match &obj {
            Object::Commit(commit) => {
                result.push(oid);
                stack.push(commit.tree);
                for parent in &commit.parents {
                    stack.push(*parent);
                }
            }
            Object::Tree(tree) => {
                result.push(oid);
                for entry in &tree.entries {
                    let entry_oid = entry.oid;
                    if excluded.contains(&entry_oid) || seen.contains(&entry_oid) {
                        continue;
                    }

                    // Apply filters.
                    if let Some(filter) = filter {
                        if !filter.should_include(repo, &entry_oid, entry.mode)? {
                            continue;
                        }
                    }

                    stack.push(entry_oid);
                }
            }
            Object::Blob(_) => {
                // Apply blob size filter.
                if let Some(filter) = filter {
                    if filter.should_include(repo, &oid, git_object::FileMode::Regular)? {
                        result.push(oid);
                    }
                } else {
                    result.push(oid);
                }
            }
            Object::Tag(tag) => {
                result.push(oid);
                stack.push(tag.target);
            }
        }
    }

    Ok(())
}

/// Process tree entries, pushing children onto the stack.
fn collect_tree_objects(
    tree: &Tree,
    _tree_oid: &ObjectId,
    _repo: &Repository,
    _seen: &mut HashSet<ObjectId>,
    stack: &mut Vec<ObjectId>,
    _filter: Option<&ObjectFilter>,
) -> Result<(), RevWalkError> {
    for entry in &tree.entries {
        stack.push(entry.oid);
    }
    Ok(())
}

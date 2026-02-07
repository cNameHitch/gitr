//! Merge base computation using the paint algorithm.
//!
//! The paint algorithm works by marking commits reachable from each input with
//! different "colors" (flags). When a commit is painted with both colors, it's
//! a common ancestor. The lowest common ancestors are the merge bases.

use std::collections::{BinaryHeap, HashMap, HashSet};

use git_hash::ObjectId;
use git_object::Object;
use git_repository::Repository;

use crate::RevWalkError;

/// Paint flags for the merge-base algorithm.
const PARENT1: u8 = 1;
const PARENT2: u8 = 2;
const STALE: u8 = 4;

/// Entry in the paint queue.
struct PaintEntry {
    oid: ObjectId,
    #[allow(dead_code)]
    flags: u8,
    date: i64,
}

impl PartialEq for PaintEntry {
    fn eq(&self, other: &Self) -> bool {
        self.oid == other.oid
    }
}

impl Eq for PaintEntry {}

impl PartialOrd for PaintEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PaintEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Max-heap by date (newest first).
        self.date.cmp(&other.date)
    }
}

/// Find all merge bases of two commits.
///
/// Returns the lowest common ancestor(s) — commits reachable from both `a` and `b`
/// that are not ancestors of any other common ancestor.
pub fn merge_base(
    repo: &Repository,
    a: &ObjectId,
    b: &ObjectId,
) -> Result<Vec<ObjectId>, RevWalkError> {
    if a == b {
        return Ok(vec![*a]);
    }

    let results = paint_down_to_common(repo, a, b)?;

    // Remove redundant bases: if base X is an ancestor of base Y, drop X.
    remove_redundant(repo, results)
}

/// Find the single best merge base of two commits.
pub fn merge_base_one(
    repo: &Repository,
    a: &ObjectId,
    b: &ObjectId,
) -> Result<Option<ObjectId>, RevWalkError> {
    let bases = merge_base(repo, a, b)?;
    Ok(bases.into_iter().next())
}

/// Check if `ancestor` is an ancestor of `descendant`.
pub fn is_ancestor(
    repo: &Repository,
    ancestor: &ObjectId,
    descendant: &ObjectId,
) -> Result<bool, RevWalkError> {
    if ancestor == descendant {
        return Ok(true);
    }

    let bases = merge_base(repo, ancestor, descendant)?;
    Ok(bases.contains(ancestor))
}

/// Paint algorithm: walk down from both commits, painting flags.
fn paint_down_to_common(
    repo: &Repository,
    a: &ObjectId,
    b: &ObjectId,
) -> Result<Vec<ObjectId>, RevWalkError> {
    let mut flags: HashMap<ObjectId, u8> = HashMap::new();
    let mut queue: BinaryHeap<PaintEntry> = BinaryHeap::new();
    let mut results: Vec<ObjectId> = Vec::new();

    // Seed the queue with both commits.
    let commit_a = read_commit(repo, a)?;
    let commit_b = read_commit(repo, b)?;

    flags.insert(*a, PARENT1);
    flags.insert(*b, PARENT2);

    queue.push(PaintEntry {
        oid: *a,
        flags: PARENT1,
        date: commit_a.committer.date.timestamp,
    });
    queue.push(PaintEntry {
        oid: *b,
        flags: PARENT2,
        date: commit_b.committer.date.timestamp,
    });

    while let Some(entry) = queue.pop() {
        let current_flags = *flags.get(&entry.oid).unwrap_or(&0);

        if current_flags & STALE != 0 {
            continue;
        }

        // If this commit has been painted with both colors, it's a common ancestor.
        if current_flags & (PARENT1 | PARENT2) == (PARENT1 | PARENT2) {
            // Mark as stale so we don't process further.
            flags.insert(entry.oid, current_flags | STALE);
            results.push(entry.oid);

            // Mark all remaining queue entries as stale if they're already common.
            // Continue processing to find all common ancestors.
            if !queue_has_nonstale(&queue, &flags) {
                break;
            }
            continue;
        }

        // Read parents and propagate flags.
        let commit = read_commit(repo, &entry.oid)?;
        for parent in &commit.parents {
            let parent_flags = flags.entry(*parent).or_insert(0);
            let new_flags = *parent_flags | current_flags;
            if new_flags != *parent_flags {
                *parent_flags = new_flags;
                if let Ok(parent_commit) = read_commit(repo, parent) {
                    queue.push(PaintEntry {
                        oid: *parent,
                        flags: new_flags,
                        date: parent_commit.committer.date.timestamp,
                    });
                }
            }
        }
    }

    Ok(results)
}

/// Check if the queue has any non-stale entries.
fn queue_has_nonstale(queue: &BinaryHeap<PaintEntry>, flags: &HashMap<ObjectId, u8>) -> bool {
    queue.iter().any(|entry| {
        let f = flags.get(&entry.oid).copied().unwrap_or(0);
        f & STALE == 0
    })
}

/// Remove redundant bases: if X is an ancestor of Y, drop X (keep only Y).
fn remove_redundant(
    repo: &Repository,
    bases: Vec<ObjectId>,
) -> Result<Vec<ObjectId>, RevWalkError> {
    if bases.len() <= 1 {
        return Ok(bases);
    }

    let mut to_remove: HashSet<usize> = HashSet::new();

    for i in 0..bases.len() {
        if to_remove.contains(&i) {
            continue;
        }
        for j in (i + 1)..bases.len() {
            if to_remove.contains(&j) {
                continue;
            }
            if is_ancestor_direct(repo, &bases[i], &bases[j])? {
                // bases[i] is ancestor of bases[j], drop bases[i]
                to_remove.insert(i);
                break;
            } else if is_ancestor_direct(repo, &bases[j], &bases[i])? {
                // bases[j] is ancestor of bases[i], drop bases[j]
                to_remove.insert(j);
            }
        }
    }

    Ok(bases
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !to_remove.contains(idx))
        .map(|(_, oid)| oid)
        .collect())
}

/// Direct ancestor check using BFS (doesn't call merge_base to avoid recursion).
fn is_ancestor_direct(
    repo: &Repository,
    ancestor: &ObjectId,
    descendant: &ObjectId,
) -> Result<bool, RevWalkError> {
    if ancestor == descendant {
        return Ok(true);
    }

    let mut queue = std::collections::VecDeque::new();
    let mut visited = HashSet::new();
    queue.push_back(*descendant);
    visited.insert(*descendant);

    while let Some(current) = queue.pop_front() {
        if current == *ancestor {
            return Ok(true);
        }
        if let Ok(commit) = read_commit(repo, &current) {
            for parent in &commit.parents {
                if visited.insert(*parent) {
                    queue.push_back(*parent);
                }
            }
        }
    }

    Ok(false)
}

fn read_commit(
    repo: &Repository,
    oid: &ObjectId,
) -> Result<git_object::Commit, RevWalkError> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or(RevWalkError::CommitNotFound(*oid))?;
    match obj {
        Object::Commit(c) => Ok(c),
        _ => Err(RevWalkError::NotACommit(*oid)),
    }
}

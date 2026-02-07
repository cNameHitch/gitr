//! Revision range parsing: A..B, A...B, ^A B.

use git_hash::ObjectId;
use git_ref::{RefName, RefStore};
use git_repository::Repository;

use crate::RevWalkError;

/// A parsed revision range.
#[derive(Debug, Clone)]
pub struct RevisionRange {
    /// Commits to include (positive references).
    pub include: Vec<ObjectId>,
    /// Commits to exclude (negative references).
    pub exclude: Vec<ObjectId>,
    /// Whether this is a symmetric difference (A...B).
    pub symmetric: bool,
}

impl RevisionRange {
    /// Parse a revision range specification.
    ///
    /// Supported formats:
    /// - `A..B` — commits reachable from B but not A
    /// - `A...B` — symmetric difference (reachable from either but not both)
    /// - `^A B` — exclude A, include B (handled at a higher level)
    /// - `A` — single revision (include only)
    pub fn parse(repo: &Repository, spec: &str) -> Result<Self, RevWalkError> {
        let spec = spec.trim();

        // Check for A...B (symmetric difference) — must check before A..B
        if let Some((left, right)) = spec.split_once("...") {
            let a = resolve_revision(repo, left.trim())?;
            let b = resolve_revision(repo, right.trim())?;

            // Symmetric difference: find merge base and exclude it.
            // Include both A and B, exclude their merge base(s).
            let bases = crate::merge_base::merge_base(repo, &a, &b)?;

            return Ok(Self {
                include: vec![a, b],
                exclude: bases,
                symmetric: true,
            });
        }

        // Check for A..B (asymmetric)
        if let Some((left, right)) = spec.split_once("..") {
            let a = resolve_revision(repo, left.trim())?;
            let b = resolve_revision(repo, right.trim())?;

            return Ok(Self {
                include: vec![b],
                exclude: vec![a],
                symmetric: false,
            });
        }

        // Check for ^A (exclude)
        if let Some(rest) = spec.strip_prefix('^') {
            let oid = resolve_revision(repo, rest.trim())?;
            return Ok(Self {
                include: vec![],
                exclude: vec![oid],
                symmetric: false,
            });
        }

        // Single revision
        let oid = resolve_revision(repo, spec)?;
        Ok(Self {
            include: vec![oid],
            exclude: vec![],
            symmetric: false,
        })
    }
}

/// Resolve a revision string to an ObjectId.
///
/// Supports:
/// - Full 40-char hex OID
/// - Short hex prefix (unambiguous)
/// - Ref names (HEAD, refs/heads/main, branch names)
/// - Special: HEAD, HEAD~N, HEAD^N
pub fn resolve_revision(repo: &Repository, rev: &str) -> Result<ObjectId, RevWalkError> {
    let rev = rev.trim();
    if rev.is_empty() {
        return Err(RevWalkError::InvalidRevision("empty revision".into()));
    }

    // Handle tilde and caret suffixes: REV~N, REV^N
    if let Some((base, suffix)) = split_revision_suffix(rev) {
        let base_oid = resolve_revision(repo, base)?;
        return apply_revision_suffix(repo, &base_oid, suffix);
    }

    // Try as hex OID (full or prefix)
    if rev.len() >= 4 && rev.chars().all(|c| c.is_ascii_hexdigit()) {
        if rev.len() == 40 || rev.len() == 64 {
            if let Ok(oid) = ObjectId::from_hex(rev) {
                return Ok(oid);
            }
        }
        // Try prefix resolution
        if let Ok(oid) = repo.odb().resolve_prefix(rev) {
            return Ok(oid);
        }
    }

    // Try as ref name
    // First try exact ref name
    if let Ok(Some(oid)) = resolve_ref_to_oid(repo, rev) {
        return Ok(oid);
    }

    // Try with refs/heads/ prefix
    if let Ok(Some(oid)) = resolve_ref_to_oid(repo, &format!("refs/heads/{}", rev)) {
        return Ok(oid);
    }

    // Try with refs/tags/ prefix
    if let Ok(Some(oid)) = resolve_ref_to_oid(repo, &format!("refs/tags/{}", rev)) {
        return Ok(oid);
    }

    // Try with refs/remotes/ prefix
    if let Ok(Some(oid)) = resolve_ref_to_oid(repo, &format!("refs/remotes/{}", rev)) {
        return Ok(oid);
    }

    // Try with refs/ prefix
    if let Ok(Some(oid)) = resolve_ref_to_oid(repo, &format!("refs/{}", rev)) {
        return Ok(oid);
    }

    Err(RevWalkError::InvalidRevision(format!(
        "cannot resolve '{}'",
        rev
    )))
}

fn resolve_ref_to_oid(repo: &Repository, name: &str) -> Result<Option<ObjectId>, RevWalkError> {
    let ref_name = RefName::new(name).map_err(RevWalkError::Ref)?;
    Ok(repo.refs().resolve_to_oid(&ref_name)?)
}

/// Split a revision string into base and suffix (tilde/caret).
/// Returns None if there's no suffix.
fn split_revision_suffix(rev: &str) -> Option<(&str, &str)> {
    // Check for ^{type} peeling suffix first
    if let Some(brace_start) = rev.rfind("^{") {
        if rev.ends_with('}') && brace_start > 0 {
            return Some((&rev[..brace_start], &rev[brace_start..]));
        }
    }

    // Find the last ~ or ^ that's followed by optional digits
    for (i, c) in rev.char_indices().rev() {
        if (c == '~' || c == '^') && i > 0 {
            let suffix = &rev[i..];
            let base = &rev[..i];
            // Verify suffix is valid: ~, ~N, ^, ^N
            let rest = &suffix[1..];
            if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()) {
                return Some((base, suffix));
            }
        }
    }
    None
}

/// Apply a tilde or caret suffix to an OID.
fn apply_revision_suffix(
    repo: &Repository,
    oid: &ObjectId,
    suffix: &str,
) -> Result<ObjectId, RevWalkError> {
    // Handle ^{type} peeling syntax first
    if suffix.starts_with("^{") && suffix.ends_with('}') {
        let target_type = &suffix[2..suffix.len() - 1];
        return peel_to_type(repo, oid, target_type);
    }

    let (op, count_str) = suffix.split_at(1);
    let count: usize = if count_str.is_empty() {
        1
    } else {
        count_str
            .parse()
            .map_err(|_| RevWalkError::InvalidRevision(format!("invalid suffix: {}", suffix)))?
    };

    match op {
        "~" => {
            // ~N means follow first-parent N times
            let mut current = *oid;
            for _ in 0..count {
                let commit = read_commit(repo, &current)?;
                current = commit
                    .first_parent()
                    .copied()
                    .ok_or(RevWalkError::InvalidRevision(format!(
                        "{}~{} goes past root",
                        oid, count
                    )))?;
            }
            Ok(current)
        }
        "^" => {
            // ^N means Nth parent (^0 means the commit itself)
            if count == 0 {
                return Ok(*oid);
            }
            let commit = read_commit(repo, oid)?;
            commit
                .parents
                .get(count - 1)
                .copied()
                .ok_or(RevWalkError::InvalidRevision(format!(
                    "{}^{} has no such parent",
                    oid, count
                )))
        }
        _ => Err(RevWalkError::InvalidRevision(format!(
            "unknown suffix: {}",
            suffix
        ))),
    }
}

/// Peel an object to the specified type.
///
/// Supports: "tree", "commit", "blob", "tag", "" (recursive peel until non-tag).
fn peel_to_type(
    repo: &Repository,
    oid: &ObjectId,
    target_type: &str,
) -> Result<ObjectId, RevWalkError> {
    let mut current_oid = *oid;

    for _ in 0..512 {
        let obj = repo
            .odb()
            .read(&current_oid)?
            .ok_or(RevWalkError::CommitNotFound(current_oid))?;

        match target_type {
            "" => {
                // Recursive peel: follow tags until non-tag
                match obj {
                    git_object::Object::Tag(tag) => {
                        current_oid = tag.target;
                        continue;
                    }
                    _ => return Ok(current_oid),
                }
            }
            "commit" => match obj {
                git_object::Object::Commit(_) => return Ok(current_oid),
                git_object::Object::Tag(tag) => {
                    current_oid = tag.target;
                    continue;
                }
                _ => {
                    return Err(RevWalkError::InvalidRevision(format!(
                        "{} cannot be peeled to commit",
                        oid
                    )));
                }
            },
            "tree" => match obj {
                git_object::Object::Tree(_) => return Ok(current_oid),
                git_object::Object::Commit(c) => return Ok(c.tree),
                git_object::Object::Tag(tag) => {
                    current_oid = tag.target;
                    continue;
                }
                _ => {
                    return Err(RevWalkError::InvalidRevision(format!(
                        "{} cannot be peeled to tree",
                        oid
                    )));
                }
            },
            "blob" => match obj {
                git_object::Object::Blob(_) => return Ok(current_oid),
                git_object::Object::Tag(tag) => {
                    current_oid = tag.target;
                    continue;
                }
                _ => {
                    return Err(RevWalkError::InvalidRevision(format!(
                        "{} cannot be peeled to blob",
                        oid
                    )));
                }
            },
            "tag" => match obj {
                git_object::Object::Tag(_) => return Ok(current_oid),
                _ => {
                    return Err(RevWalkError::InvalidRevision(format!(
                        "{} cannot be peeled to tag",
                        oid
                    )));
                }
            },
            _ => {
                return Err(RevWalkError::InvalidRevision(format!(
                    "unknown peel type: {}",
                    target_type
                )));
            }
        }
    }

    Err(RevWalkError::InvalidRevision(format!(
        "peeling {} exceeded depth limit",
        oid
    )))
}

fn read_commit(repo: &Repository, oid: &ObjectId) -> Result<git_object::Commit, RevWalkError> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or(RevWalkError::CommitNotFound(*oid))?;
    match obj {
        git_object::Object::Commit(c) => Ok(c),
        _ => Err(RevWalkError::NotACommit(*oid)),
    }
}

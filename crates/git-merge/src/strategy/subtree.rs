//! Subtree merge strategy.
//!
//! Matches trees that are at different levels in the hierarchy and adjusts
//! the tree structure before delegating to ORT for the actual merge.
//!
//! Shift detection: finds which subdirectory in "ours" best matches the root
//! of "theirs", shifts all paths in "theirs" under that prefix, then runs ORT.

use bstr::BString;
use git_hash::ObjectId;
use git_object::{FileMode, Object, Tree, TreeEntry};
use git_repository::Repository;

use crate::strategy::ort::OrtStrategy;
use crate::strategy::MergeStrategy;
use crate::{MergeError, MergeOptions, MergeResult};

/// Minimum match score for automatic subtree detection.
/// Set low to match git's permissive behavior — even a single overlapping
/// entry name is enough when there's a clear best-matching subdirectory.
const AUTO_DETECT_THRESHOLD: f64 = 0.01;

/// The subtree merge strategy.
///
/// Detects which subdirectory in the "ours" tree corresponds to the root of the
/// "theirs" tree, shifts "theirs" paths under that prefix, then delegates to ORT.
pub struct SubtreeStrategy;

impl MergeStrategy for SubtreeStrategy {
    fn merge(
        &self,
        repo: &mut Repository,
        ours: &ObjectId,
        theirs: &ObjectId,
        base: &ObjectId,
        options: &MergeOptions,
    ) -> Result<MergeResult, MergeError> {
        // Check for explicit --subtree=<prefix> in strategy options
        let explicit_prefix = options.strategy_options.iter().find_map(|opt| {
            opt.strip_prefix("subtree=").map(|s| s.to_string())
        });

        let prefix = if let Some(p) = explicit_prefix {
            // Ensure prefix ends with /
            if p.ends_with('/') { p } else { format!("{}/", p) }
        } else {
            // Auto-detect: find best matching subdirectory
            let ours_tree_oid = Self::get_tree_oid(repo, ours)?;
            let theirs_tree_oid = Self::get_tree_oid(repo, theirs)?;
            Self::detect_subtree_prefix(repo, &ours_tree_oid, &theirs_tree_oid)?
        };

        // Shift "theirs" tree: create a new commit where all paths are under the prefix
        let theirs_tree_oid = Self::get_tree_oid(repo, theirs)?;
        let shifted_tree = Self::shift_tree(repo, &theirs_tree_oid, &prefix)?;

        // Create a temporary commit with the shifted tree
        let shifted_commit = Self::create_temp_commit(repo, &shifted_tree, theirs)?;

        // Also shift the base tree if it's not NULL
        let shifted_base = if !base.is_null() {
            let base_tree_oid = Self::get_tree_oid(repo, base)?;
            let shifted_base_tree = Self::shift_tree(repo, &base_tree_oid, &prefix)?;
            Self::create_temp_commit(repo, &shifted_base_tree, base)?
        } else {
            *base
        };

        // Delegate to ORT with shifted theirs
        let ort = OrtStrategy;
        ort.merge(repo, ours, &shifted_commit, &shifted_base, options)
    }
}

impl SubtreeStrategy {
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

    /// Read a tree object from ODB.
    fn read_tree(repo: &Repository, tree_oid: &ObjectId) -> Result<Tree, MergeError> {
        let obj = repo
            .odb()
            .read(tree_oid)?
            .ok_or(MergeError::ObjectNotFound(*tree_oid))?;
        match obj {
            Object::Tree(t) => Ok(t),
            _ => Err(MergeError::UnexpectedObjectType {
                oid: *tree_oid,
                expected: "tree",
                actual: obj.object_type().to_string(),
            }),
        }
    }

    /// Auto-detect the subtree prefix by finding the subdirectory in "ours"
    /// that best matches the root of "theirs".
    ///
    /// Scores each subdirectory by counting matching entries, returns the one
    /// with the highest score above the threshold.
    fn detect_subtree_prefix(
        repo: &Repository,
        ours_tree_oid: &ObjectId,
        theirs_tree_oid: &ObjectId,
    ) -> Result<String, MergeError> {
        let ours_tree = Self::read_tree(repo, ours_tree_oid)?;
        let theirs_tree = Self::read_tree(repo, theirs_tree_oid)?;

        if theirs_tree.entries.is_empty() {
            return Err(MergeError::InvalidPatch(
                "subtree merge: theirs tree is empty".into(),
            ));
        }

        // Collect theirs entry names for fast lookup
        let theirs_names: std::collections::HashSet<&[u8]> = theirs_tree
            .entries
            .iter()
            .map(|e| e.name.as_ref())
            .collect();
        let theirs_count = theirs_names.len() as f64;

        let mut best_score = 0.0_f64;
        let mut best_prefix = String::new();

        // Iterate subdirectories in "ours"
        for entry in &ours_tree.entries {
            if !entry.mode.is_tree() {
                continue;
            }

            let sub_tree = Self::read_tree(repo, &entry.oid)?;
            let matching = sub_tree
                .entries
                .iter()
                .filter(|e| theirs_names.contains(e.name.as_slice()))
                .count() as f64;

            let score = matching / theirs_count;

            if score > best_score {
                best_score = score;
                best_prefix = String::from_utf8_lossy(&entry.name).to_string();
            }
        }

        if best_score < AUTO_DETECT_THRESHOLD {
            return Err(MergeError::InvalidPatch(
                "subtree merge: could not auto-detect subtree prefix (no subdirectory matches well enough)".into(),
            ));
        }

        Ok(format!("{}/", best_prefix))
    }

    /// Shift all paths in a tree under the given prefix.
    ///
    /// Creates a new tree where the original tree's contents are nested inside
    /// a subtree at the given prefix path.
    fn shift_tree(
        repo: &Repository,
        tree_oid: &ObjectId,
        prefix: &str,
    ) -> Result<ObjectId, MergeError> {
        // Parse the prefix into path components (e.g., "lib/vendor/" -> ["lib", "vendor"])
        let components: Vec<&str> = prefix
            .trim_end_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if components.is_empty() {
            return Ok(*tree_oid);
        }

        // Build from inside out: the innermost tree is the original tree
        let mut current_oid = *tree_oid;

        for component in components.iter().rev() {
            // Create a wrapper tree with a single subdirectory entry
            let wrapper = Tree {
                entries: vec![TreeEntry {
                    mode: FileMode::Tree,
                    name: BString::from(*component),
                    oid: current_oid,
                }],
            };
            current_oid = repo.odb().write(&Object::Tree(wrapper))?;
        }

        Ok(current_oid)
    }

    /// Create a temporary commit with the given tree, copying metadata from the original.
    fn create_temp_commit(
        repo: &Repository,
        tree_oid: &ObjectId,
        original_commit: &ObjectId,
    ) -> Result<ObjectId, MergeError> {
        let obj = repo
            .odb()
            .read(original_commit)?
            .ok_or(MergeError::ObjectNotFound(*original_commit))?;
        let orig = match obj {
            Object::Commit(c) => c,
            _ => {
                return Err(MergeError::UnexpectedObjectType {
                    oid: *original_commit,
                    expected: "commit",
                    actual: obj.object_type().to_string(),
                })
            }
        };

        let commit = git_object::Commit {
            tree: *tree_oid,
            parents: orig.parents.clone(),
            author: orig.author.clone(),
            committer: orig.committer.clone(),
            message: orig.message.clone(),
            encoding: orig.encoding.clone(),
            gpgsig: orig.gpgsig.clone(),
            extra_headers: orig.extra_headers.clone(),
        };
        let oid = repo.odb().write(&Object::Commit(commit))?;
        Ok(oid)
    }
}

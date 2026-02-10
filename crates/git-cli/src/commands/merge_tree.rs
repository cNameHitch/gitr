use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice};
use clap::Args;
use git_hash::ObjectId;
use git_object::{FileMode, Object, Tree, TreeEntry};
use git_revwalk::resolve_revision;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct MergeTreeArgs {
    /// Write the resulting tree object to the object database
    #[arg(long)]
    pub write_tree: bool,

    /// Show only conflicting file names
    #[arg(long)]
    pub name_only: bool,

    /// Use NUL as line terminator
    #[arg(short = 'z')]
    pub nul_terminated: bool,

    /// Base tree-ish (common ancestor)
    pub base: String,

    /// First branch tree-ish
    pub branch1: String,

    /// Second branch tree-ish
    pub branch2: String,
}

pub fn run(args: &MergeTreeArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Resolve all three to tree OIDs
    let base_tree = resolve_to_tree(&repo, &args.base)?;
    let branch1_tree = resolve_to_tree(&repo, &args.branch1)?;
    let branch2_tree = resolve_to_tree(&repo, &args.branch2)?;

    // Flatten all three trees into path maps
    let base_map = flatten_tree(repo.odb(), &base_tree, &BString::from(""))?;
    let b1_map = flatten_tree(repo.odb(), &branch1_tree, &BString::from(""))?;
    let b2_map = flatten_tree(repo.odb(), &branch2_tree, &BString::from(""))?;

    // Collect all paths
    let mut all_paths: Vec<BString> = Vec::new();
    for key in base_map.keys() {
        if !all_paths.contains(key) {
            all_paths.push(key.clone());
        }
    }
    for key in b1_map.keys() {
        if !all_paths.contains(key) {
            all_paths.push(key.clone());
        }
    }
    for key in b2_map.keys() {
        if !all_paths.contains(key) {
            all_paths.push(key.clone());
        }
    }
    all_paths.sort();

    let line_end = if args.nul_terminated { "\0" } else { "\n" };

    let mut conflicts: Vec<BString> = Vec::new();
    let mut merged_entries: Vec<(BString, ObjectId, FileMode)> = Vec::new();

    for path in &all_paths {
        let base_entry = base_map.get(path);
        let b1_entry = b1_map.get(path);
        let b2_entry = b2_map.get(path);

        match (base_entry, b1_entry, b2_entry) {
            // All three are the same -- no change
            (Some(base), Some(b1), Some(b2)) if base.0 == b1.0 && base.0 == b2.0 => {
                merged_entries.push((path.clone(), base.0, base.1));
            }
            // Only branch1 changed
            (Some(base), Some(b1), Some(b2)) if base.0 == b2.0 => {
                merged_entries.push((path.clone(), b1.0, b1.1));
            }
            // Only branch2 changed
            (Some(base), Some(b1), Some(b2)) if base.0 == b1.0 => {
                merged_entries.push((path.clone(), b2.0, b2.1));
            }
            // Both changed identically
            (Some(_base), Some(b1), Some(b2)) if b1.0 == b2.0 => {
                merged_entries.push((path.clone(), b1.0, b1.1));
            }
            // Both changed differently -- conflict
            (Some(_base), Some(_b1), Some(_b2)) => {
                conflicts.push(path.clone());
                // In merged tree, we could pick either side; for write-tree we skip conflicted
            }
            // File added in both branches with same content
            (None, Some(b1), Some(b2)) if b1.0 == b2.0 => {
                merged_entries.push((path.clone(), b1.0, b1.1));
            }
            // File added in both branches with different content -- conflict
            (None, Some(_b1), Some(_b2)) => {
                conflicts.push(path.clone());
            }
            // File only in branch1 (added in branch1, or deleted in branch2)
            (None, Some(b1), None) => {
                merged_entries.push((path.clone(), b1.0, b1.1));
            }
            (Some(_base), Some(_b1), None) => {
                // Deleted in branch2, still in branch1: modify/delete conflict
                conflicts.push(path.clone());
            }
            // File only in branch2 (added in branch2, or deleted in branch1)
            (None, None, Some(b2)) => {
                merged_entries.push((path.clone(), b2.0, b2.1));
            }
            (Some(_base), None, Some(_b2)) => {
                // Deleted in branch1, still in branch2: modify/delete conflict
                conflicts.push(path.clone());
            }
            // Deleted in both -- fine, skip
            (Some(_), None, None) => {}
            // Not in base and not in either branch -- shouldn't happen
            (None, None, None) => {}
        }
    }

    let has_conflicts = !conflicts.is_empty();

    if args.name_only {
        for path in &conflicts {
            write!(out, "{}{}", path.to_str_lossy(), line_end)?;
        }
    } else {
        // Show conflict information
        for path in &conflicts {
            let base_oid = base_map
                .get(path)
                .map(|e| e.0.to_hex())
                .unwrap_or_else(|| ObjectId::NULL_SHA1.to_hex());
            let b1_oid = b1_map
                .get(path)
                .map(|e| e.0.to_hex())
                .unwrap_or_else(|| ObjectId::NULL_SHA1.to_hex());
            let b2_oid = b2_map
                .get(path)
                .map(|e| e.0.to_hex())
                .unwrap_or_else(|| ObjectId::NULL_SHA1.to_hex());
            let b1_mode = b1_map.get(path).map(|e| e.1).unwrap_or(FileMode::Regular);
            let b2_mode = b2_map.get(path).map(|e| e.1).unwrap_or(FileMode::Regular);

            write!(
                out,
                "CONFLICT (content): Merge conflict in {}{}",
                path.to_str_lossy(),
                line_end
            )?;
            write!(
                out,
                "  base:    {} {}{}",
                base_oid, path.to_str_lossy(), line_end
            )?;
            write!(
                out,
                "  branch1: {} {:o} {}{}",
                b1_oid,
                b1_mode.raw(),
                path.to_str_lossy(),
                line_end
            )?;
            write!(
                out,
                "  branch2: {} {:o} {}{}",
                b2_oid,
                b2_mode.raw(),
                path.to_str_lossy(),
                line_end
            )?;
        }
    }

    if args.write_tree && !has_conflicts {
        let tree_oid = build_merged_tree(&mut repo, &merged_entries)?;
        writeln!(out, "{}", tree_oid.to_hex())?;
    } else if args.write_tree && has_conflicts {
        let stderr = io::stderr();
        let mut err = stderr.lock();
        writeln!(
            err,
            "error: merge has conflicts, cannot write tree"
        )?;
    }

    if has_conflicts {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Resolve a revision string to a tree OID (handles commits and trees).
fn resolve_to_tree(repo: &git_repository::Repository, spec: &str) -> Result<ObjectId> {
    let oid = resolve_revision(repo, spec)?;
    let obj = repo
        .odb()
        .read(&oid)?
        .ok_or_else(|| anyhow::anyhow!("object {} not found", oid.to_hex()))?;

    match obj {
        Object::Commit(c) => Ok(c.tree),
        Object::Tree(_) => Ok(oid),
        _ => bail!("{} is not a tree-ish", spec),
    }
}

/// Flatten a tree recursively into a map of path -> (oid, mode).
fn flatten_tree(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &BString,
) -> Result<std::collections::BTreeMap<BString, (ObjectId, FileMode)>> {
    let mut map = std::collections::BTreeMap::new();

    let obj = odb
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree {} not found", tree_oid.to_hex()))?;

    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree, got {}", obj.object_type()),
    };

    for entry in &tree.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.extend_from_slice(b"/");
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            let sub = flatten_tree(odb, &entry.oid, &path)?;
            map.extend(sub);
        } else {
            map.insert(path, (entry.oid, entry.mode));
        }
    }

    Ok(map)
}

/// Build a tree object from merged entries and write it to the ODB.
fn build_merged_tree(
    repo: &mut git_repository::Repository,
    entries: &[(BString, ObjectId, FileMode)],
) -> Result<ObjectId> {
    // Group by top-level directory
    let mut top_blobs: Vec<TreeEntry> = Vec::new();
    let mut subdirs: std::collections::BTreeMap<BString, Vec<(BString, ObjectId, FileMode)>> =
        std::collections::BTreeMap::new();

    for (path, oid, mode) in entries {
        let path_str = path.to_str_lossy();
        if let Some(slash) = path_str.find('/') {
            let dir = BString::from(&path_str[..slash]);
            let rest = BString::from(&path_str[slash + 1..]);
            subdirs
                .entry(dir)
                .or_default()
                .push((rest, *oid, *mode));
        } else {
            top_blobs.push(TreeEntry {
                mode: *mode,
                name: path.clone(),
                oid: *oid,
            });
        }
    }

    let mut tree_entries = top_blobs;

    for (dir_name, sub_entries) in &subdirs {
        let sub_tree_oid = build_merged_tree(repo, sub_entries)?;
        tree_entries.push(TreeEntry {
            mode: FileMode::Tree,
            name: dir_name.clone(),
            oid: sub_tree_oid,
        });
    }

    tree_entries.sort_by(TreeEntry::cmp_entries);

    let tree = Tree {
        entries: tree_entries,
    };
    let oid = repo.odb().write(&Object::Tree(tree))?;
    Ok(oid)
}

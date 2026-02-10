use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_index::{EntryFlags, Index, IndexEntry, Stage, StatData};
use git_object::{FileMode, Object};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct ReadTreeArgs {
    /// Perform a merge (1-tree: reset, 2-tree: merge, 3-tree: 3-way merge)
    #[arg(short = 'm')]
    merge: bool,

    /// Update the working tree after read-tree
    #[arg(short = 'u')]
    update: bool,

    /// Same as -u, also discard untracked files under new directories
    #[arg(long)]
    reset: bool,

    /// Read tree into subtree at <prefix>/
    #[arg(long, value_name = "prefix")]
    prefix: Option<String>,

    /// Empty the index before reading the tree
    #[arg(long)]
    empty: bool,

    /// Don't update the working tree (for use with -m)
    #[arg(short = 'i')]
    index_only: bool,

    /// Be verbose about what is being done
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Tree-ish to read (1-3 depending on merge mode)
    #[arg(value_name = "tree-ish")]
    tree_ish: Vec<String>,
}

pub fn run(args: &ReadTreeArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if args.tree_ish.is_empty() && !args.empty {
        bail!("fatal: you must specify at least one tree-ish");
    }

    // Validate merge mode tree-ish count
    if args.merge && args.tree_ish.len() > 3 {
        bail!("fatal: at most 3 tree-ish arguments allowed with -m");
    }

    // Handle --empty: just clear the index
    if args.empty {
        let new_index = Index::new();
        repo.set_index(new_index);
        repo.write_index()?;
        if args.verbose {
            writeln!(err, "Emptied the index")?;
        }
        return Ok(0);
    }

    // Resolve tree-ish arguments to tree OIDs
    let mut tree_oids = Vec::new();
    for tree_arg in &args.tree_ish {
        let oid = git_revwalk::resolve_revision(&repo, tree_arg)?;
        let tree_oid = resolve_to_tree(&repo, &oid)?;
        tree_oids.push(tree_oid);
    }

    // Build prefix path (ensure trailing /)
    let prefix = args.prefix.as_deref().map(|p| {
        let p = p.trim_end_matches('/');
        if p.is_empty() {
            String::new()
        } else {
            format!("{}/", p)
        }
    });

    let mut new_index = Index::new();

    if !args.merge {
        // Simple read-tree: read the first (only) tree into the index
        if tree_oids.len() != 1 {
            bail!("fatal: exactly one tree-ish required without -m");
        }
        read_tree_into_index(
            repo.odb(),
            &tree_oids[0],
            prefix.as_deref().unwrap_or(""),
            &mut new_index,
            args.verbose,
            &mut err,
        )?;
    } else {
        match tree_oids.len() {
            1 => {
                // 1-tree merge: reset index to this tree
                read_tree_into_index(
                    repo.odb(),
                    &tree_oids[0],
                    prefix.as_deref().unwrap_or(""),
                    &mut new_index,
                    args.verbose,
                    &mut err,
                )?;
            }
            2 => {
                // 2-tree merge: compare current index with old tree, apply new tree
                // Simplified: read the second tree
                read_tree_into_index(
                    repo.odb(),
                    &tree_oids[1],
                    prefix.as_deref().unwrap_or(""),
                    &mut new_index,
                    args.verbose,
                    &mut err,
                )?;
            }
            3 => {
                // 3-way merge: ancestor, ours, theirs
                // Write conflict entries for differing paths
                three_way_merge(
                    repo.odb(),
                    &tree_oids[0],
                    &tree_oids[1],
                    &tree_oids[2],
                    prefix.as_deref().unwrap_or(""),
                    &mut new_index,
                    args.verbose,
                    &mut err,
                )?;
            }
            _ => bail!("fatal: too many tree-ish arguments"),
        }
    }

    repo.set_index(new_index);
    repo.write_index()?;

    // Optionally update working tree
    let should_update_wt = (args.update || args.reset) && !args.index_only;
    if should_update_wt {
        if let Some(wt) = repo.work_tree() {
            let wt = wt.to_path_buf();
            // Collect entries first to avoid borrow conflict between index and odb
            let entries: Vec<_> = {
                let index = repo.index()?;
                index.iter().map(|e| (e.path.clone(), e.oid)).collect()
            };
            for (path, oid) in &entries {
                let file_path = wt.join(path.to_string());
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                // Read blob and write to working tree
                if let Some(Object::Blob(blob)) = repo.odb().read(oid)? {
                    std::fs::write(&file_path, &blob.data)?;
                    if args.verbose {
                        writeln!(err, "Checking out {}", path)?;
                    }
                }
            }
        }
    }

    Ok(0)
}

/// Resolve an OID to a tree OID (dereference commits).
fn resolve_to_tree(
    repo: &git_repository::Repository,
    oid: &ObjectId,
) -> Result<ObjectId> {
    match repo.odb().read(oid)? {
        Some(Object::Tree(_)) => Ok(*oid),
        Some(Object::Commit(commit)) => Ok(commit.tree),
        Some(other) => bail!("not a tree or commit: {} is a {}", oid.to_hex(), other.object_type()),
        None => bail!("object not found: {}", oid.to_hex()),
    }
}

/// Read a tree recursively into the index.
fn read_tree_into_index(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &str,
    index: &mut Index,
    verbose: bool,
    err: &mut impl Write,
) -> Result<()> {
    let tree = match odb.read(tree_oid)? {
        Some(Object::Tree(t)) => t,
        _ => bail!("not a tree: {}", tree_oid.to_hex()),
    };

    for entry in tree.iter() {
        let name = String::from_utf8_lossy(&entry.name).to_string();
        let full_path = if prefix.is_empty() {
            name
        } else {
            format!("{}{}", prefix, name)
        };

        if entry.mode.is_tree() {
            // Recurse into subtree
            read_tree_into_index(
                odb,
                &entry.oid,
                &format!("{}/", full_path),
                index,
                verbose,
                err,
            )?;
        } else {
            if verbose {
                writeln!(err, "{:06o} {} {}\t{}", entry.mode.raw(), entry.oid.to_hex(), 0, full_path)?;
            }
            let idx_entry = IndexEntry {
                path: BString::from(full_path.as_bytes()),
                oid: entry.oid,
                mode: entry.mode,
                stage: Stage::Normal,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            };
            index.add(idx_entry);
        }
    }

    Ok(())
}

/// Collect all blob entries from a tree into a flat map.
fn collect_tree_entries(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &str,
) -> Result<std::collections::BTreeMap<String, (ObjectId, FileMode)>> {
    let mut entries = std::collections::BTreeMap::new();
    collect_tree_entries_recursive(odb, tree_oid, prefix, &mut entries)?;
    Ok(entries)
}

fn collect_tree_entries_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &str,
    entries: &mut std::collections::BTreeMap<String, (ObjectId, FileMode)>,
) -> Result<()> {
    let tree = match odb.read(tree_oid)? {
        Some(Object::Tree(t)) => t,
        _ => bail!("not a tree: {}", tree_oid.to_hex()),
    };

    for entry in tree.iter() {
        let name = String::from_utf8_lossy(&entry.name).to_string();
        let full_path = if prefix.is_empty() {
            name
        } else {
            format!("{}{}", prefix, name)
        };

        if entry.mode.is_tree() {
            collect_tree_entries_recursive(odb, &entry.oid, &format!("{}/", full_path), entries)?;
        } else {
            entries.insert(full_path, (entry.oid, entry.mode));
        }
    }

    Ok(())
}

/// Perform a 3-way merge of three trees into the index.
#[allow(clippy::too_many_arguments)]
fn three_way_merge(
    odb: &git_odb::ObjectDatabase,
    ancestor_oid: &ObjectId,
    ours_oid: &ObjectId,
    theirs_oid: &ObjectId,
    prefix: &str,
    index: &mut Index,
    verbose: bool,
    err: &mut impl Write,
) -> Result<()> {
    let ancestor = collect_tree_entries(odb, ancestor_oid, prefix)?;
    let ours = collect_tree_entries(odb, ours_oid, prefix)?;
    let theirs = collect_tree_entries(odb, theirs_oid, prefix)?;

    // Collect all unique paths
    let mut all_paths = std::collections::BTreeSet::new();
    for key in ancestor.keys() {
        all_paths.insert(key.clone());
    }
    for key in ours.keys() {
        all_paths.insert(key.clone());
    }
    for key in theirs.keys() {
        all_paths.insert(key.clone());
    }

    for path in &all_paths {
        let a = ancestor.get(path);
        let o = ours.get(path);
        let t = theirs.get(path);

        match (a, o, t) {
            // All three agree
            (Some((a_oid, _)), Some((o_oid, o_mode)), Some((t_oid, _)))
                if a_oid == o_oid && o_oid == t_oid =>
            {
                let entry = IndexEntry {
                    path: BString::from(path.as_bytes()),
                    oid: *o_oid,
                    mode: *o_mode,
                    stage: Stage::Normal,
                    stat: StatData::default(),
                    flags: EntryFlags::default(),
                };
                index.add(entry);
            }
            // Ours and theirs agree (both changed same way)
            (_, Some((o_oid, o_mode)), Some((t_oid, _))) if o_oid == t_oid => {
                let entry = IndexEntry {
                    path: BString::from(path.as_bytes()),
                    oid: *o_oid,
                    mode: *o_mode,
                    stage: Stage::Normal,
                    stat: StatData::default(),
                    flags: EntryFlags::default(),
                };
                index.add(entry);
            }
            // Only ours changed from ancestor
            (Some((a_oid, _)), Some((o_oid, o_mode)), Some((t_oid, _)))
                if a_oid == t_oid && a_oid != o_oid =>
            {
                let entry = IndexEntry {
                    path: BString::from(path.as_bytes()),
                    oid: *o_oid,
                    mode: *o_mode,
                    stage: Stage::Normal,
                    stat: StatData::default(),
                    flags: EntryFlags::default(),
                };
                index.add(entry);
            }
            // Only theirs changed from ancestor
            (Some((a_oid, _)), Some((o_oid, _)), Some((t_oid, t_mode)))
                if a_oid == o_oid && a_oid != t_oid =>
            {
                let entry = IndexEntry {
                    path: BString::from(path.as_bytes()),
                    oid: *t_oid,
                    mode: *t_mode,
                    stage: Stage::Normal,
                    stat: StatData::default(),
                    flags: EntryFlags::default(),
                };
                index.add(entry);
            }
            // Conflict: both sides changed differently
            _ => {
                if verbose {
                    writeln!(err, "CONFLICT (content): Merge conflict in {}", path)?;
                }
                // Write stage entries for conflict
                if let Some((a_oid, a_mode)) = a {
                    index.add(IndexEntry {
                        path: BString::from(path.as_bytes()),
                        oid: *a_oid,
                        mode: *a_mode,
                        stage: Stage::Base,
                        stat: StatData::default(),
                        flags: EntryFlags::default(),
                    });
                }
                if let Some((o_oid, o_mode)) = o {
                    index.add(IndexEntry {
                        path: BString::from(path.as_bytes()),
                        oid: *o_oid,
                        mode: *o_mode,
                        stage: Stage::Ours,
                        stat: StatData::default(),
                        flags: EntryFlags::default(),
                    });
                }
                if let Some((t_oid, t_mode)) = t {
                    index.add(IndexEntry {
                        path: BString::from(path.as_bytes()),
                        oid: *t_oid,
                        mode: *t_mode,
                        stage: Stage::Theirs,
                        stat: StatData::default(),
                        flags: EntryFlags::default(),
                    });
                }
            }
        }
    }

    Ok(())
}

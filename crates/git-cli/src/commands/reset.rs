use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice, ByteVec};
use clap::Args;
use git_hash::ObjectId;
use git_index::{Index, IndexEntry, Stage, StatData, EntryFlags};
use git_object::{FileMode, Object};
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct ResetArgs {
    /// Only move HEAD (keep index and working tree)
    #[arg(long)]
    soft: bool,

    /// Move HEAD and reset index (keep working tree) - this is the default
    #[arg(long)]
    mixed: bool,

    /// Move HEAD, reset index and working tree
    #[arg(long)]
    hard: bool,

    /// Reset to merge state
    #[arg(long)]
    merge: bool,

    /// Commit to reset to
    #[arg(default_value = "HEAD")]
    commit: String,

    /// Paths to reset (unstage specific files)
    #[arg(last = true)]
    paths: Vec<String>,
}

pub fn run(args: &ResetArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    // If paths are given, this is a path-based reset (unstage files)
    if !args.paths.is_empty() {
        return reset_paths(&mut repo, &args.commit, &args.paths);
    }

    let target_oid = git_revwalk::resolve_revision(&repo, &args.commit)?;

    // Read the target commit's tree
    let obj = repo.odb().read(&target_oid)?
        .ok_or_else(|| anyhow::anyhow!("object {} not found", target_oid.to_hex()))?;
    let tree_oid = match &obj {
        Object::Commit(c) => c.tree,
        _ => bail!("expected commit, got {}", obj.object_type()),
    };

    // Determine mode
    let is_hard = args.hard;
    let is_soft = args.soft;
    // Default is mixed

    if !is_soft {
        // Reset index from tree
        let work_tree = repo.work_tree().map(|p| p.to_path_buf());
        let mut new_index = Index::new();
        build_index_from_tree(repo.odb(), &tree_oid, &BString::from(""), &mut new_index)?;
        let index_path = repo.git_dir().join("index");
        new_index.write_to(&index_path)?;
        repo.set_index(new_index);

        if is_hard {
            // Also reset working tree
            if let Some(ref wt) = work_tree {
                checkout_tree_to_worktree(repo.odb(), &tree_oid, wt)?;
            }
        }
    }

    // Move HEAD
    update_head(&repo, &target_oid)?;

    // Clean up merge state files
    let git_dir = repo.git_dir();
    let _ = std::fs::remove_file(git_dir.join("MERGE_HEAD"));
    let _ = std::fs::remove_file(git_dir.join("MERGE_MSG"));
    let _ = std::fs::remove_file(git_dir.join("MERGE_MODE"));

    Ok(0)
}

fn reset_paths(
    repo: &mut git_repository::Repository,
    commit: &str,
    paths: &[String],
) -> Result<i32> {
    let target_oid = git_revwalk::resolve_revision(repo, commit)?;
    let obj = repo.odb().read(&target_oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found"))?;
    let tree_oid = match &obj {
        Object::Commit(c) => c.tree,
        _ => bail!("expected commit"),
    };

    for path in paths {
        let rel = BString::from(path.as_bytes());
        // Find the blob in the target tree
        if let Some((oid, mode)) = find_blob_in_tree(repo.odb(), &tree_oid, &rel)? {
            let entry = IndexEntry {
                path: rel.clone(),
                oid,
                mode,
                stage: Stage::Normal,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            };
            let index = repo.index_mut()?;
            index.add(entry);
        } else {
            let index = repo.index_mut()?;
            index.remove(rel.as_ref(), Stage::Normal);
        }
    }
    repo.write_index()?;
    Ok(0)
}

pub(crate) fn update_head(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let head = RefName::new(BString::from("HEAD"))?;
    match repo.refs().resolve(&head)? {
        Some(git_ref::Reference::Symbolic { target, .. }) => {
            repo.refs().write_ref(&target, oid)?;
        }
        _ => {
            repo.refs().write_ref(&head, oid)?;
        }
    }
    Ok(())
}

pub(crate) fn build_index_from_tree(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &BString,
    index: &mut Index,
) -> Result<()> {
    let obj = odb.read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree"),
    };

    for entry in tree.iter() {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.push_byte(b'/');
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            build_index_from_tree(odb, &entry.oid, &path, index)?;
        } else {
            index.add(IndexEntry {
                path,
                oid: entry.oid,
                mode: entry.mode,
                stage: Stage::Normal,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            });
        }
    }
    Ok(())
}

pub(crate) fn checkout_tree_to_worktree(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    work_tree: &Path,
) -> Result<()> {
    checkout_recursive(odb, tree_oid, work_tree, &BString::from(""))
}

fn checkout_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    work_tree: &Path,
    prefix: &BString,
) -> Result<()> {
    let obj = odb.read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree"),
    };

    for entry in tree.iter() {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.push_byte(b'/');
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            let dir = work_tree.join(path.to_str_lossy().as_ref());
            std::fs::create_dir_all(&dir)?;
            checkout_recursive(odb, &entry.oid, work_tree, &path)?;
        } else {
            let file_path = work_tree.join(path.to_str_lossy().as_ref());
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let blob = odb.read(&entry.oid)?
                .ok_or_else(|| anyhow::anyhow!("blob not found"))?;
            let data = match blob {
                Object::Blob(b) => b.data,
                _ => bail!("expected blob"),
            };
            std::fs::write(&file_path, &data)?;
            #[cfg(unix)]
            if entry.mode == FileMode::Executable {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    Ok(())
}

fn find_blob_in_tree(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    path: &BString,
) -> Result<Option<(ObjectId, FileMode)>> {
    let obj = odb.read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree"),
    };
    let parts: Vec<&[u8]> = path.split(|&b| b == b'/').collect();
    find_recursive(odb, &tree, &parts)
}

fn find_recursive(
    odb: &git_odb::ObjectDatabase,
    tree: &git_object::Tree,
    parts: &[&[u8]],
) -> Result<Option<(ObjectId, FileMode)>> {
    if parts.is_empty() {
        return Ok(None);
    }
    for entry in tree.iter() {
        if entry.name.as_slice() == parts[0] {
            if parts.len() == 1 {
                return Ok(Some((entry.oid, entry.mode)));
            }
            if entry.mode.is_tree() {
                let obj = odb.read(&entry.oid)?
                    .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
                let subtree = match obj {
                    Object::Tree(t) => t,
                    _ => bail!("expected tree"),
                };
                return find_recursive(odb, &subtree, &parts[1..]);
            }
        }
    }
    Ok(None)
}

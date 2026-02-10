use std::path::PathBuf;

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_index::{EntryFlags, IndexEntry, Stage, StatData};
use git_object::{FileMode, Object};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RestoreArgs {
    /// Restore the index (unstage)
    #[arg(short = 'S', long)]
    pub staged: bool,

    /// Restore the working tree (default when --staged not given)
    #[arg(short = 'W', long)]
    pub worktree: bool,

    /// Specify a commit to restore from
    #[arg(short, long, value_name = "tree-ish")]
    pub source: Option<String>,

    /// Overlay mode (keep files not in source)
    #[arg(long)]
    pub overlay: bool,

    /// No overlay mode (remove files not in source)
    #[arg(long)]
    pub no_overlay: bool,

    /// Conflict style (merge, diff3)
    #[arg(long)]
    pub conflict: Option<String>,

    /// Use our version for unmerged files
    #[arg(long)]
    pub ours: bool,

    /// Use their version for unmerged files
    #[arg(long)]
    pub theirs: bool,

    /// Read pathspecs from file
    #[arg(long)]
    pub pathspec_from_file: Option<PathBuf>,

    /// Interactively select hunks to restore (stub)
    #[arg(short = 'p', long)]
    pub patch: bool,

    /// Files to restore
    #[arg(required = true)]
    pub files: Vec<String>,
}

impl RestoreArgs {
    /// Construct RestoreArgs from a list of file paths (used by checkout).
    pub fn from_paths(paths: Vec<String>) -> Self {
        RestoreArgs {
            source: None,
            staged: false,
            worktree: true,
            overlay: false,
            no_overlay: false,
            conflict: None,
            ours: false,
            theirs: false,
            pathspec_from_file: None,
            patch: false,
            files: paths,
        }
    }
}

pub fn run(args: &RestoreArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    // Determine what to restore: if --staged, restore from source to index.
    // If --worktree (or neither flag), restore from index to worktree.
    let restore_staged = args.staged;
    let restore_worktree = args.worktree || !args.staged;

    if restore_staged {
        // Restore from source (default HEAD) to index
        let source_tree = resolve_source_tree(&repo, args.source.as_deref())?;

        for file in &args.files {
            let rel = BString::from(file.as_bytes());

            if let Some(tree_oid) = &source_tree {
                // Find blob in source tree
                if let Some((oid, mode)) = find_blob_in_tree(repo.odb(), tree_oid, &rel)? {
                    let metadata_opt = {
                        let fs_path = work_tree.join(file);
                        if fs_path.exists() {
                            Some(std::fs::symlink_metadata(&fs_path)?)
                        } else {
                            None
                        }
                    };
                    let entry = IndexEntry {
                        path: rel,
                        oid,
                        mode,
                        stage: Stage::Normal,
                        stat: metadata_opt
                            .map(|m| StatData::from_metadata(&m))
                            .unwrap_or_default(),
                        flags: EntryFlags::default(),
                    };
                    let index = repo.index_mut()?;
                    index.add(entry);
                } else {
                    // File not in source: remove from index
                    let index = repo.index_mut()?;
                    index.remove(rel.as_ref(), Stage::Normal);
                }
            } else {
                // Unborn branch: remove from index
                let index = repo.index_mut()?;
                index.remove(rel.as_ref(), Stage::Normal);
            }
        }
        repo.write_index()?;
    }

    if restore_worktree {
        // Restore from index to working tree
        for file in &args.files {
            let rel = BString::from(file.as_bytes());
            let fs_path = work_tree.join(file);

            let entry_data = {
                let index = repo.index()?;
                index
                    .get(rel.as_ref(), Stage::Normal)
                    .map(|e| (e.oid, e.mode))
            };

            if let Some((oid, mode)) = entry_data {
                let obj = repo
                    .odb()
                    .read(&oid)?
                    .ok_or_else(|| anyhow::anyhow!("object {} not found", oid.to_hex()))?;
                let data = match obj {
                    Object::Blob(b) => b.data,
                    _ => bail!("expected blob for {}", file),
                };

                if let Some(parent) = fs_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&fs_path, &data)?;

                #[cfg(unix)]
                if mode == FileMode::Executable {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(
                        &fs_path,
                        std::fs::Permissions::from_mode(0o755),
                    )?;
                }
            } else {
                // Not in index, nothing to restore
                bail!(
                    "pathspec '{}' did not match any file(s) known to git",
                    file
                );
            }
        }
    }

    Ok(0)
}

fn resolve_source_tree(
    repo: &git_repository::Repository,
    source: Option<&str>,
) -> Result<Option<ObjectId>> {
    let oid = if let Some(spec) = source {
        Some(git_revwalk::resolve_revision(repo, spec)?)
    } else {
        repo.head_oid()?
    };

    if let Some(oid) = oid {
        let obj = repo
            .odb()
            .read(&oid)?
            .ok_or_else(|| anyhow::anyhow!("object {} not found", oid.to_hex()))?;
        match obj {
            Object::Commit(c) => Ok(Some(c.tree)),
            Object::Tree(_) => Ok(Some(oid)),
            _ => bail!("not a tree-ish: {}", oid.to_hex()),
        }
    } else {
        Ok(None)
    }
}

fn find_blob_in_tree(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    path: &BString,
) -> Result<Option<(ObjectId, FileMode)>> {
    let obj = odb
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree {} not found", tree_oid.to_hex()))?;
    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree"),
    };

    let parts: Vec<&[u8]> = path.split(|&b| b == b'/').collect();
    find_in_tree_recursive(odb, &tree, &parts)
}

fn find_in_tree_recursive(
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
            // Recurse into subtree
            if entry.mode.is_tree() {
                let obj = odb
                    .read(&entry.oid)?
                    .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
                let subtree = match obj {
                    Object::Tree(t) => t,
                    _ => bail!("expected tree"),
                };
                return find_in_tree_recursive(odb, &subtree, &parts[1..]);
            }
        }
    }
    Ok(None)
}

use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice, ByteVec};
use clap::Args;
use git_hash::ObjectId;
use git_index::{Index, IndexEntry, Stage, StatData, EntryFlags};
use git_object::{FileMode, Object};
use git_ref::{RefName, RefStore};
use git_ref::reflog::{ReflogEntry, append_reflog_entry};

use crate::Cli;
use crate::interactive::{InteractiveHunkSelector, reverse_apply_hunks_to_content};
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

    /// Reset keeping local changes (like --hard but keeps uncommitted changes)
    #[arg(long)]
    keep: bool,

    /// Be quiet, only report errors
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Don't refresh the index after reset
    #[arg(short = 'N', long)]
    no_refresh: bool,

    /// Interactively select hunks to reset (stub)
    #[arg(short = 'p', long)]
    patch: bool,

    /// Commit to reset to, or paths to unstage
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

pub fn run(args: &ResetArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    if args.patch {
        let diff_result = git_diff::worktree::diff_head_to_index(&mut repo, &git_diff::DiffOptions::default())?;
        if diff_result.files.is_empty() {
            eprintln!("No staged changes.");
            return Ok(0);
        }
        let mut selector = InteractiveHunkSelector::new()?;
        let selected = selector.select_hunks(&diff_result)?;
        for file_diff in &selected.files {
            let path = file_diff.path();
            // Read current index content (the "new" side of head-to-index diff)
            let index_oid = {
                let index = repo.index()?;
                index.get(path.as_ref(), Stage::Normal).map(|e| e.oid)
            };
            let index_content = if let Some(oid) = index_oid {
                match repo.odb().read(&oid)? {
                    Some(Object::Blob(b)) => b.data.to_vec(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            let reverted = reverse_apply_hunks_to_content(&index_content, &file_diff.hunks);
            let blob = git_object::Blob { data: reverted };
            let new_oid = repo.odb().write(&Object::Blob(blob))?;
            let mode = file_diff.old_mode.unwrap_or(FileMode::Regular);
            let entry = IndexEntry {
                path: path.clone(),
                oid: new_oid,
                mode,
                stage: Stage::Normal,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            };
            let index = repo.index_mut()?;
            index.add(entry);
        }
        repo.write_index()?;
        return Ok(0);
    }

    // Parse args: disambiguate commit vs paths
    let (commit, paths) = parse_reset_args(&args.args, &repo);

    // If paths are given, this is a path-based reset (unstage files)
    if !paths.is_empty() {
        return reset_paths(&mut repo, &commit, &paths);
    }

    let target_oid = git_revwalk::resolve_revision(&repo, &commit)?;

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

    // Capture old HEAD before reset
    let old_head = repo.head_oid()?.unwrap_or(ObjectId::NULL_SHA1);

    // Move HEAD
    update_head(&repo, &target_oid)?;

    // Write reflog entry for HEAD
    {
        let sig = super::commit::get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", &repo)?;
        let entry = ReflogEntry {
            old_oid: old_head,
            new_oid: target_oid,
            identity: sig,
            message: BString::from(format!("reset: moving to {}", commit)),
        };
        let head_ref = RefName::new(BString::from("HEAD"))?;
        append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
    }

    // Clean up merge state files
    let git_dir = repo.git_dir();
    let _ = std::fs::remove_file(git_dir.join("MERGE_HEAD"));
    let _ = std::fs::remove_file(git_dir.join("MERGE_MSG"));
    let _ = std::fs::remove_file(git_dir.join("MERGE_MODE"));

    let stderr = io::stderr();
    let mut err = stderr.lock();

    if is_hard {
        // Show "HEAD is now at <short-hash> <subject>"
        let obj = repo.odb().read(&target_oid)?;
        if let Some(Object::Commit(c)) = obj {
            let hex = target_oid.to_hex();
            let short = &hex[..7.min(hex.len())];
            let summary = String::from_utf8_lossy(c.summary());
            writeln!(err, "HEAD is now at {} {}", short, summary)?;
        }
    } else if !is_soft {
        // Mixed reset: show unstaged changes
        let unstaged = git_diff::worktree::diff_index_to_worktree(&mut repo, &git_diff::DiffOptions::default())?;
        if !unstaged.files.is_empty() {
            writeln!(err, "Unstaged changes after reset:")?;
            for file in &unstaged.files {
                let code = file.status.as_char();
                writeln!(err, "{}\t{}", code, file.path().to_str_lossy())?;
            }
        }
    }

    Ok(0)
}

/// Parse reset arguments: disambiguate commit vs pathspecs.
/// Returns (commit_ref, paths).
fn parse_reset_args(args: &[String], repo: &git_repository::Repository) -> (String, Vec<String>) {
    if args.is_empty() {
        return ("HEAD".to_string(), Vec::new());
    }

    // If "--" is present, everything before is commit, everything after is paths
    if let Some(sep_pos) = args.iter().position(|a| a == "--") {
        let commit = if sep_pos > 0 {
            args[0].clone()
        } else {
            "HEAD".to_string()
        };
        let paths = args[sep_pos + 1..].to_vec();
        return (commit, paths);
    }

    // Try first arg as revision
    if git_revwalk::resolve_revision(repo, &args[0]).is_ok() {
        // First arg is a valid revision
        let paths = args[1..].to_vec();
        return (args[0].clone(), paths);
    }

    // First arg is not a revision — treat all as paths with implicit HEAD
    ("HEAD".to_string(), args.to_vec())
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

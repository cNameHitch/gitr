use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice};
use clap::{Args, Subcommand};
use git_hash::ObjectId;
use git_index::{Index, IndexEntry, Stage, StatData, EntryFlags};
use git_object::{Commit, FileMode, Object};
use git_ref::{RefName, RefStore};
use git_ref::reflog::{ReflogEntry, append_reflog_entry, read_reflog};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct StashArgs {
    #[command(subcommand)]
    command: Option<StashSubcommand>,
}

#[derive(Subcommand)]
pub enum StashSubcommand {
    /// Save changes to stash (default)
    Push {
        /// Message for the stash entry
        #[arg(short, long)]
        message: Option<String>,

        /// Include untracked files
        #[arg(short = 'u', long)]
        include_untracked: bool,
    },
    /// Restore the most recent stash
    Pop {
        /// Stash index to pop
        stash: Option<usize>,
    },
    /// Like pop, but don't remove from stash
    Apply {
        stash: Option<usize>,
    },
    /// List stash entries
    List,
    /// Show stash contents
    Show {
        stash: Option<usize>,
    },
    /// Drop a stash entry
    Drop {
        stash: Option<usize>,
    },
    /// Remove all stash entries
    Clear,
}

pub fn run(args: &StashArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        None | Some(StashSubcommand::Push { .. }) => {
            let (message, include_untracked) = match &args.command {
                Some(StashSubcommand::Push { message, include_untracked }) => {
                    (message.clone(), *include_untracked)
                }
                _ => (None, false),
            };
            stash_push(cli, message.as_deref(), include_untracked)
        }
        Some(StashSubcommand::Pop { stash }) => stash_pop(cli, stash.unwrap_or(0), true),
        Some(StashSubcommand::Apply { stash }) => stash_pop(cli, stash.unwrap_or(0), false),
        Some(StashSubcommand::List) => stash_list(cli),
        Some(StashSubcommand::Show { stash }) => stash_show(cli, stash.unwrap_or(0)),
        Some(StashSubcommand::Drop { stash }) => stash_drop(cli, stash.unwrap_or(0)),
        Some(StashSubcommand::Clear) => stash_clear(cli),
    }
}

fn stash_push(cli: &Cli, message: Option<&str>, include_untracked: bool) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let head_oid = repo.head_oid()?
        .ok_or_else(|| anyhow::anyhow!("cannot stash on an unborn branch"))?;

    let branch = repo.current_branch()?.unwrap_or_else(|| "(no branch)".to_string());

    // Get the HEAD commit for the message
    let head_obj = repo.odb().read(&head_oid)?
        .ok_or_else(|| anyhow::anyhow!("HEAD commit not found"))?;
    let head_commit = match &head_obj {
        Object::Commit(c) => c,
        _ => bail!("HEAD is not a commit"),
    };
    let head_summary = head_commit.summary().to_str_lossy().to_string();
    let head_tree = head_commit.tree;

    let work_tree = repo.work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?
        .to_path_buf();

    // Build tree from current index (staged state)
    let index_path = repo.git_dir().join("index");
    let current_index = Index::read_from(&index_path)?;
    let index_tree_oid = current_index.write_tree(repo.odb())?;

    // Build worktree tree: start with index entries, replace with worktree content
    let worktree_tree_oid = build_worktree_tree(&repo, &current_index, &work_tree, include_untracked)?;

    let sig = build_stash_signature(&repo)?;

    // Create index commit (represents staged changes)
    let index_commit = Commit {
        tree: index_tree_oid,
        parents: vec![head_oid],
        author: sig.clone(),
        committer: sig.clone(),
        message: BString::from(
            "index on ".to_string() + &branch + ": " + &head_oid.to_hex()[..7] + " " + &head_summary + "\n",
        ),
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
    };
    let index_commit_oid = repo.odb().write(&Object::Commit(index_commit))?;

    // Create the stash commit (tree = current worktree state, parents = [HEAD, index_commit])
    let default_msg = format!("WIP on {}: {} {}", branch, &head_oid.to_hex()[..7], head_summary);
    let custom_msg;
    let stash_msg: &str = match message {
        Some(m) => {
            custom_msg = format!("On {}: {}", branch, m);
            &custom_msg
        }
        None => &default_msg,
    };

    let mut parents = vec![head_oid, index_commit_oid];

    // If --include-untracked, create untracked files commit
    if include_untracked {
        if let Some(untracked_tree_oid) = build_untracked_tree(&repo, &current_index, &work_tree)? {
            let untracked_commit = Commit {
                tree: untracked_tree_oid,
                parents: vec![],
                author: sig.clone(),
                committer: sig.clone(),
                message: BString::from(
                    format!("untracked files on {}: {} {}\n", branch, &head_oid.to_hex()[..7], head_summary),
                ),
                encoding: None,
                gpgsig: None,
                extra_headers: Vec::new(),
            };
            let untracked_oid = repo.odb().write(&Object::Commit(untracked_commit))?;
            parents.push(untracked_oid);
        }
    }

    let stash_commit = Commit {
        tree: worktree_tree_oid,
        parents,
        author: sig.clone(),
        committer: sig.clone(),
        message: BString::from(format!("{}\n", stash_msg)),
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
    };
    let stash_oid = repo.odb().write(&Object::Commit(stash_commit))?;

    // Get old stash OID for reflog
    let stash_ref = RefName::new(BString::from("refs/stash"))?;
    let old_stash_oid = repo.refs().resolve_to_oid(&stash_ref)?
        .unwrap_or(git_hash::HashAlgorithm::Sha1.null_oid());

    // Update refs/stash
    repo.refs().write_ref(&stash_ref, &stash_oid)?;

    // Write reflog entry
    let reflog_entry = ReflogEntry {
        old_oid: old_stash_oid,
        new_oid: stash_oid,
        identity: sig,
        message: BString::from(stash_msg),
    };
    append_reflog_entry(repo.git_dir(), &stash_ref, &reflog_entry)?;

    // Reset working tree to HEAD
    let mut new_index = Index::new();
    super::reset::build_index_from_tree(repo.odb(), &head_tree, &BString::from(""), &mut new_index)?;
    new_index.write_to(&index_path)?;
    repo.set_index(new_index);

    // Actually write HEAD tree files to working tree
    super::reset::checkout_tree_to_worktree(repo.odb(), &head_tree, &work_tree)?;

    // Remove untracked files if --include-untracked
    if include_untracked {
        remove_untracked_files(&current_index, &work_tree)?;
    }

    writeln!(err, "Saved working directory and index state {}", stash_msg)?;
    Ok(0)
}

/// Build a tree object from the current worktree state.
///
/// Takes the current index and replaces entry OIDs with the actual worktree content.
fn build_worktree_tree(
    repo: &git_repository::Repository,
    index: &Index,
    work_tree: &Path,
    _include_untracked: bool,
) -> Result<ObjectId> {
    let mut worktree_index = Index::new();

    for entry in index.iter() {
        if entry.stage != Stage::Normal {
            continue;
        }
        let path_str = entry.path.to_str_lossy();
        let fs_path = work_tree.join(path_str.as_ref());

        if fs_path.exists() && fs_path.is_file() {
            // Read actual worktree content and hash it
            let data = std::fs::read(&fs_path)?;
            let blob = git_object::Blob { data: data.into() };
            let blob_oid = repo.odb().write(&Object::Blob(blob))?;
            let metadata = std::fs::symlink_metadata(&fs_path)?;
            let mode = file_mode_from_metadata(&metadata);
            worktree_index.add(IndexEntry {
                path: entry.path.clone(),
                oid: blob_oid,
                mode,
                stage: Stage::Normal,
                stat: StatData::from_metadata(&metadata),
                flags: EntryFlags::default(),
            });
        } else {
            // File deleted in worktree — skip it (don't include in stash tree)
            // Actually, include from index so stash captures what was staged
            worktree_index.add(IndexEntry {
                path: entry.path.clone(),
                oid: entry.oid,
                mode: entry.mode,
                stage: Stage::Normal,
                stat: entry.stat,
                flags: EntryFlags::default(),
            });
        }
    }

    Ok(worktree_index.write_tree(repo.odb())?)
}

/// Build a tree containing only untracked files.
fn build_untracked_tree(
    repo: &git_repository::Repository,
    index: &Index,
    work_tree: &Path,
) -> Result<Option<ObjectId>> {
    let tracked_paths: std::collections::HashSet<String> = index.iter()
        .map(|e| e.path.to_str_lossy().to_string())
        .collect();

    let mut untracked_index = Index::new();

    // Walk worktree for untracked files
    collect_untracked_files(work_tree, work_tree, &tracked_paths, &mut untracked_index, repo)?;

    if untracked_index.is_empty() {
        return Ok(None);
    }

    Ok(Some(untracked_index.write_tree(repo.odb())?))
}

fn collect_untracked_files(
    root: &Path,
    dir: &Path,
    tracked: &std::collections::HashSet<String>,
    index: &mut Index,
    repo: &git_repository::Repository,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap_or("");

        // Skip .git directory
        if name == ".git" {
            continue;
        }

        if path.is_dir() {
            collect_untracked_files(root, &path, tracked, index, repo)?;
        } else {
            let rel = path.strip_prefix(root)
                .map(|p| p.to_str().unwrap_or("").to_string())
                .unwrap_or_default();
            if !tracked.contains(&rel) {
                let data = std::fs::read(&path)?;
                let blob = git_object::Blob { data: data.into() };
                let blob_oid = repo.odb().write(&Object::Blob(blob))?;
                let metadata = std::fs::symlink_metadata(&path)?;
                let mode = file_mode_from_metadata(&metadata);
                index.add(IndexEntry {
                    path: BString::from(rel),
                    oid: blob_oid,
                    mode,
                    stage: Stage::Normal,
                    stat: StatData::from_metadata(&metadata),
                    flags: EntryFlags::default(),
                });
            }
        }
    }
    Ok(())
}

fn remove_untracked_files(index: &Index, work_tree: &Path) -> Result<()> {
    let tracked_paths: std::collections::HashSet<String> = index.iter()
        .map(|e| e.path.to_str_lossy().to_string())
        .collect();
    remove_untracked_in_dir(work_tree, work_tree, &tracked_paths)
}

fn remove_untracked_in_dir(
    root: &Path,
    dir: &Path,
    tracked: &std::collections::HashSet<String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap_or("");
        if name == ".git" {
            continue;
        }
        if path.is_dir() {
            remove_untracked_in_dir(root, &path, tracked)?;
        } else {
            let rel = path.strip_prefix(root)
                .map(|p| p.to_str().unwrap_or("").to_string())
                .unwrap_or_default();
            if !tracked.contains(&rel) {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

fn stash_pop(cli: &Cli, index: usize, drop: bool) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    let stash_ref = RefName::new(BString::from("refs/stash"))?;

    // Resolve stash@{index} via reflog
    let stash_oid = if index == 0 {
        repo.refs().resolve_to_oid(&stash_ref)?
            .ok_or_else(|| anyhow::anyhow!("No stash entries found."))?
    } else {
        git_ref::reflog::resolve_at_n(repo.git_dir(), &stash_ref, index)?
            .ok_or_else(|| anyhow::anyhow!("stash@{{{}}} not found", index))?
    };

    // Read stash commit
    let obj = repo.odb().read(&stash_oid)?
        .ok_or_else(|| anyhow::anyhow!("stash commit not found"))?;
    let stash_commit = match obj {
        Object::Commit(c) => c,
        _ => bail!("stash ref does not point to a commit"),
    };

    // Checkout the stash tree
    let work_tree = repo.work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?
        .to_path_buf();

    let mut new_index = Index::new();
    super::reset::build_index_from_tree(repo.odb(), &stash_commit.tree, &BString::from(""), &mut new_index)?;
    let index_path = repo.git_dir().join("index");
    new_index.write_to(&index_path)?;
    repo.set_index(new_index);

    super::reset::checkout_tree_to_worktree(repo.odb(), &stash_commit.tree, &work_tree)?;

    // If there's a 3rd parent (untracked files commit), restore those too
    if stash_commit.parents.len() >= 3 {
        let untracked_oid = stash_commit.parents[2];
        if let Some(obj) = repo.odb().read(&untracked_oid)? {
            if let Object::Commit(c) = obj {
                super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
            }
        }
    }

    if drop {
        // Remove stash ref
        repo.refs().delete_ref(&stash_ref)?;
        // Also clear the reflog
        let reflog_path = repo.git_dir().join("logs").join("refs").join("stash");
        let _ = std::fs::remove_file(&reflog_path);
        let stderr = io::stderr();
        let mut err = stderr.lock();
        writeln!(err, "Dropped refs/stash@{{0}} ({})", &stash_oid.to_hex()[..7])?;
    }

    Ok(0)
}

fn stash_list(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let stash_ref = RefName::new(BString::from("refs/stash"))?;
    let entries = read_reflog(repo.git_dir(), &stash_ref)?;

    for (i, entry) in entries.iter().enumerate() {
        writeln!(out, "stash@{{{}}}: {}", i, entry.message.to_str_lossy())?;
    }

    Ok(0)
}

fn stash_show(cli: &Cli, _index: usize) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let stash_ref = RefName::new(BString::from("refs/stash"))?;
    let stash_oid = repo.refs().resolve_to_oid(&stash_ref)?
        .ok_or_else(|| anyhow::anyhow!("No stash entries found."))?;

    writeln!(out, "stash@{{0}}: {}", stash_oid.to_hex())?;
    Ok(0)
}

fn stash_drop(cli: &Cli, index: usize) -> Result<i32> {
    if index != 0 {
        bail!("only stash@{{0}} is supported");
    }
    let repo = open_repo(cli)?;
    let stash_ref = RefName::new(BString::from("refs/stash"))?;
    let oid = repo.refs().resolve_to_oid(&stash_ref)?
        .ok_or_else(|| anyhow::anyhow!("No stash entries found."))?;
    repo.refs().delete_ref(&stash_ref)?;

    let stderr = io::stderr();
    let mut err = stderr.lock();
    writeln!(err, "Dropped refs/stash@{{0}} ({})", &oid.to_hex()[..7])?;
    Ok(0)
}

fn stash_clear(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stash_ref = RefName::new(BString::from("refs/stash"))?;
    if repo.refs().resolve(&stash_ref)?.is_some() {
        repo.refs().delete_ref(&stash_ref)?;
    }
    // Also clear the reflog
    let reflog_path = repo.git_dir().join("logs").join("refs").join("stash");
    let _ = std::fs::remove_file(&reflog_path);
    Ok(0)
}

fn build_stash_signature(repo: &git_repository::Repository) -> Result<Signature> {
    let name = std::env::var("GIT_COMMITTER_NAME")
        .or_else(|_| repo.config().get_string("user.name").ok().flatten().ok_or(std::env::VarError::NotPresent))
        .unwrap_or_else(|_| "Unknown".to_string());
    let email = std::env::var("GIT_COMMITTER_EMAIL")
        .or_else(|_| repo.config().get_string("user.email").ok().flatten().ok_or(std::env::VarError::NotPresent))
        .unwrap_or_else(|_| "unknown@unknown".to_string());
    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date: GitDate::now(),
    })
}

fn file_mode_from_metadata(meta: &std::fs::Metadata) -> FileMode {
    if meta.is_symlink() {
        FileMode::Symlink
    } else if meta.is_dir() {
        FileMode::Tree
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 != 0 {
                return FileMode::Executable;
            }
        }
        FileMode::Regular
    }
}

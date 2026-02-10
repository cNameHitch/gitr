use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice, ByteVec};
use clap::Args;
use git_hash::ObjectId;
use git_index::{Index, IndexEntry, Stage, StatData, EntryFlags};
use git_object::{FileMode, Object};
use git_ref::reflog::{append_reflog_entry, ReflogEntry};
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct SwitchArgs {
    /// Create a new branch and switch to it
    #[arg(long, value_name = "new-branch")]
    pub create: Option<String>,

    /// Create or reset a branch and switch to it
    #[arg(long, value_name = "new-branch")]
    pub force_create: Option<String>,

    /// Switch to a commit without creating a branch
    #[arg(long)]
    pub detach: bool,

    /// Force switch (discard local changes)
    #[arg(short, long)]
    pub force: bool,

    /// Branch or commit to switch to
    pub target: Option<String>,
}

pub fn run(args: &SwitchArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Capture current HEAD info for reflog
    let old_head_oid = repo.head_oid()?.unwrap_or(ObjectId::NULL_SHA1);
    let old_branch = repo.current_branch()?.unwrap_or_else(|| {
        let hex = old_head_oid.to_hex();
        hex[..7.min(hex.len())].to_string()
    });

    // Handle -c (create and switch)
    if let Some(ref new_branch) = args.create {
        let start = args.target.as_deref().unwrap_or("HEAD");
        let oid = git_revwalk::resolve_revision(&repo, start)?;

        let refname = RefName::new(BString::from(format!("refs/heads/{}", new_branch)))?;
        if repo.refs().resolve(&refname)?.is_some() {
            bail!("fatal: a branch named '{}' already exists", new_branch);
        }

        repo.refs().write_ref(&refname, &oid)?;
        switch_to_branch(&mut repo, new_branch, &oid, args.force)?;
        write_switch_reflog(&repo, old_head_oid, oid, &old_branch, new_branch)?;
        writeln!(err, "Switched to a new branch '{}'", new_branch)?;
        return Ok(0);
    }

    // Handle -C (force create and switch)
    if let Some(ref new_branch) = args.force_create {
        let start = args.target.as_deref().unwrap_or("HEAD");
        let oid = git_revwalk::resolve_revision(&repo, start)?;

        let refname = RefName::new(BString::from(format!("refs/heads/{}", new_branch)))?;
        repo.refs().write_ref(&refname, &oid)?;
        switch_to_branch(&mut repo, new_branch, &oid, args.force)?;
        write_switch_reflog(&repo, old_head_oid, oid, &old_branch, new_branch)?;
        writeln!(err, "Switched to a new branch '{}'", new_branch)?;
        return Ok(0);
    }

    let target = args.target.as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing branch or commit argument"))?;

    if args.detach {
        let oid = git_revwalk::resolve_revision(&repo, target)?;
        switch_to_detached(&mut repo, &oid, args.force)?;
        write_switch_reflog(&repo, old_head_oid, oid, &old_branch, target)?;
        writeln!(err, "HEAD is now at {} {}", &oid.to_hex()[..7], target)?;
        return Ok(0);
    }

    // Try to switch to an existing branch
    let refname = RefName::new(BString::from(format!("refs/heads/{}", target)))?;
    if let Some(reference) = repo.refs().resolve(&refname)? {
        let oid = reference.peel_to_oid(repo.refs())?;
        switch_to_branch(&mut repo, target, &oid, args.force)?;
        write_switch_reflog(&repo, old_head_oid, oid, &old_branch, target)?;
        writeln!(err, "Switched to branch '{}'", target)?;
        Ok(0)
    } else {
        bail!("fatal: invalid reference: {}", target);
    }
}

fn write_switch_reflog(
    repo: &git_repository::Repository,
    old_oid: ObjectId,
    new_oid: ObjectId,
    old_name: &str,
    new_name: &str,
) -> Result<()> {
    let sig = super::commit::get_signature(
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "GIT_COMMITTER_DATE",
        repo,
    )?;
    let entry = ReflogEntry {
        old_oid,
        new_oid,
        identity: sig,
        message: BString::from(format!("checkout: moving from {} to {}", old_name, new_name)),
    };
    let head_ref = RefName::new(BString::from("HEAD"))?;
    append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
    Ok(())
}

fn switch_to_branch(
    repo: &mut git_repository::Repository,
    branch: &str,
    oid: &ObjectId,
    _force: bool,
) -> Result<()> {
    // Update working tree
    checkout_commit(repo, oid)?;

    // Update HEAD to point to the branch
    let head = RefName::new(BString::from("HEAD"))?;
    let branch_ref = RefName::new(BString::from(format!("refs/heads/{}", branch)))?;
    repo.refs().write_symbolic_ref(&head, &branch_ref)?;

    Ok(())
}

fn switch_to_detached(
    repo: &mut git_repository::Repository,
    oid: &ObjectId,
    _force: bool,
) -> Result<()> {
    checkout_commit(repo, oid)?;

    let head = RefName::new(BString::from("HEAD"))?;
    repo.refs().write_ref(&head, oid)?;

    Ok(())
}

fn checkout_commit(repo: &mut git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("cannot switch in a bare repository"))?
        .to_path_buf();

    // Read the commit's tree
    let obj = repo.odb().read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object {} not found", oid.to_hex()))?;
    let tree_oid = match obj {
        Object::Commit(c) => c.tree,
        _ => bail!("expected commit, got {}", obj.object_type()),
    };

    // Build new index from tree and checkout files
    let mut new_entries = Vec::new();
    checkout_tree_recursive(repo.odb(), &tree_oid, &work_tree, &BString::from(""), &mut new_entries)?;

    // Clean up files from old index that aren't in new tree
    let old_paths: std::collections::HashSet<BString> = {
        let index = repo.index()?;
        index.iter().map(|e| e.path.clone()).collect()
    };

    let new_paths: std::collections::HashSet<BString> = new_entries.iter().map(|e| e.path.clone()).collect();

    for old_path in &old_paths {
        if !new_paths.contains(old_path) {
            let fs_path = work_tree.join(old_path.to_str_lossy().as_ref());
            if fs_path.exists() {
                let _ = std::fs::remove_file(&fs_path);
            }
        }
    }

    let mut new_index = Index::new();
    for entry in new_entries {
        new_index.add(entry);
    }
    let index_path = repo.git_dir().join("index");
    new_index.write_to(&index_path)?;
    repo.set_index(new_index);

    Ok(())
}

fn checkout_tree_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    work_tree: &Path,
    prefix: &BString,
    entries: &mut Vec<IndexEntry>,
) -> Result<()> {
    let obj = odb.read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree {} not found", tree_oid.to_hex()))?;
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
            let dir_path = work_tree.join(path.to_str_lossy().as_ref());
            std::fs::create_dir_all(&dir_path)?;
            checkout_tree_recursive(odb, &entry.oid, work_tree, &path, entries)?;
        } else {
            let file_path = work_tree.join(path.to_str_lossy().as_ref());
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let blob_obj = odb.read(&entry.oid)?
                .ok_or_else(|| anyhow::anyhow!("blob not found"))?;
            let data = match blob_obj {
                Object::Blob(b) => b.data,
                _ => bail!("expected blob"),
            };

            std::fs::write(&file_path, &data)?;

            #[cfg(unix)]
            if entry.mode == FileMode::Executable {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o755))?;
            }

            let metadata = std::fs::symlink_metadata(&file_path)?;
            entries.push(IndexEntry {
                path,
                oid: entry.oid,
                mode: entry.mode,
                stage: Stage::Normal,
                stat: StatData::from_metadata(&metadata),
                flags: EntryFlags::default(),
            });
        }
    }
    Ok(())
}

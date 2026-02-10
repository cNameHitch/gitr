use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_merge::MergeOptions;
use git_object::Object;
use git_ref::{RefName, RefStore};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct RevertArgs {
    /// Don't automatically commit
    #[arg(short = 'n', long)]
    no_commit: bool,

    /// Edit the commit message
    #[arg(short = 'e', long)]
    edit: bool,

    /// Use the auto-generated message without launching an editor
    #[arg(long)]
    no_edit: bool,

    /// Abort the current revert
    #[arg(long)]
    abort: bool,

    /// Continue the current revert
    #[arg(long, name = "continue")]
    continue_: bool,

    /// Skip the current commit
    #[arg(long)]
    skip: bool,

    /// Commits to revert
    commits: Vec<String>,
}

pub fn run(args: &RevertArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if args.abort {
        return handle_abort(&mut repo, &mut err);
    }

    if args.continue_ {
        return handle_continue(&mut repo, &mut out, &mut err);
    }

    if args.skip {
        return handle_skip(&mut repo, &mut out, &mut err);
    }

    if args.commits.is_empty() {
        anyhow::bail!("no commit specified for revert");
    }

    let options = MergeOptions::default();

    for rev in &args.commits {
        let commit_oid = git_revwalk::resolve_revision(&repo, rev)?;
        let result = revert_one(&mut repo, &commit_oid, &options, args.no_commit, &mut out, &mut err)?;
        if result != 0 {
            return Ok(result);
        }
    }

    Ok(0)
}

fn revert_one(
    repo: &mut git_repository::Repository,
    commit_oid: &ObjectId,
    options: &MergeOptions,
    no_commit: bool,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    let obj = repo
        .odb()
        .read(commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit not found: {}", commit_oid))?;
    let commit = match obj {
        Object::Commit(c) => c,
        _ => anyhow::bail!("not a commit: {}", commit_oid),
    };

    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    // Save ORIG_HEAD for abort
    fs::write(git_dir.join("ORIG_HEAD"), head_oid.to_hex())?;

    // Perform the revert
    let result = git_merge::revert::revert(repo, commit_oid, options)?;

    if result.is_clean {
        if no_commit {
            writeln!(out, "Revert applied (not committed)")?;
        } else if let Some(tree_oid) = result.tree {
            let summary = String::from_utf8_lossy(commit.summary());
            let message = result.message.unwrap_or_else(|| {
                format!(
                    "Revert \"{}\"\n\nThis reverts commit {}.\n",
                    summary,
                    commit_oid.to_hex()
                )
            });

            let new_oid = crate::commands::cherry_pick::create_commit_default(
                repo,
                &tree_oid,
                &[head_oid],
                &message,
            )?;

            update_head_to(repo, &new_oid)?;

            // Update worktree to match the new tree
            if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
                // Get old tree files to detect deletions
                let head_obj = repo.odb().read(&head_oid)?;
                let old_tree_oid = match head_obj {
                    Some(Object::Commit(c)) => Some(c.tree),
                    _ => None,
                };
                let old_files = if let Some(ref ot) = old_tree_oid {
                    collect_tree_files(repo.odb(), ot, "")?
                } else {
                    HashSet::new()
                };
                let new_files = collect_tree_files(repo.odb(), &tree_oid, "")?;

                // Remove files that were deleted
                for f in &old_files {
                    if !new_files.contains(f) {
                        let _ = fs::remove_file(work_tree.join(f));
                    }
                }

                super::reset::checkout_tree_to_worktree(repo.odb(), &tree_oid, &work_tree)?;
            }

            writeln!(
                out,
                "[{}] Revert \"{}\"",
                &new_oid.to_hex()[..7],
                summary
            )?;
        }

        // Clean up state
        let _ = fs::remove_file(git_dir.join("REVERT_HEAD"));
        let _ = fs::remove_file(git_dir.join("ORIG_HEAD"));
    } else {
        // Conflict
        fs::write(git_dir.join("REVERT_HEAD"), commit_oid.to_hex())?;

        let summary = String::from_utf8_lossy(commit.summary());
        let message = format!(
            "Revert \"{}\"\n\nThis reverts commit {}.\n",
            summary,
            commit_oid.to_hex()
        );
        fs::write(git_dir.join("MERGE_MSG"), &message)?;

        writeln!(
            err,
            "error: could not revert {}",
            &commit_oid.to_hex()[..7]
        )?;
        writeln!(
            err,
            "hint: After resolving conflicts, use 'git revert --continue'"
        )?;
        return Ok(1);
    }

    Ok(0)
}

fn handle_abort(
    repo: &mut git_repository::Repository,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();
    let orig_head_path = git_dir.join("ORIG_HEAD");

    if !orig_head_path.exists() {
        writeln!(err, "error: no revert in progress")?;
        return Ok(1);
    }

    let orig_hex = fs::read_to_string(&orig_head_path)?.trim().to_string();
    let orig_oid = ObjectId::from_hex(&orig_hex)?;

    update_head_to(repo, &orig_oid)?;

    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        let obj = repo.odb().read(&orig_oid)?;
        if let Some(Object::Commit(c)) = obj {
            super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
        }
    }

    // Clean up
    let _ = fs::remove_file(git_dir.join("REVERT_HEAD"));
    let _ = fs::remove_file(git_dir.join("ORIG_HEAD"));
    let _ = fs::remove_file(git_dir.join("MERGE_MSG"));

    Ok(0)
}

fn handle_continue(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    if !git_dir.join("REVERT_HEAD").exists() {
        writeln!(err, "error: no revert in progress")?;
        return Ok(1);
    }

    {
        let index = repo.index()?;
        let conflicts = index.conflicts();
        if !conflicts.is_empty() {
            writeln!(err, "error: unresolved conflicts remain")?;
            return Ok(1);
        }
    }

    // Read index from file directly to avoid borrow conflict
    // (repo.index() takes &mut self, write_tree needs repo.odb() which takes &self)
    let index_path = repo.git_dir().join("index");
    let index_for_tree = if index_path.exists() {
        git_index::Index::read_from(&index_path)?
    } else {
        git_index::Index::new()
    };
    let tree_oid = index_for_tree.write_tree(repo.odb())?;

    let msg_path = git_dir.join("MERGE_MSG");
    let message = if msg_path.exists() {
        fs::read_to_string(&msg_path)?
    } else {
        "revert".to_string()
    };

    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    let new_oid = crate::commands::cherry_pick::create_commit_default(repo, &tree_oid, &[head_oid], &message)?;
    update_head_to(repo, &new_oid)?;

    writeln!(out, "[{}] {}", &new_oid.to_hex()[..7], message.lines().next().unwrap_or(""))?;

    // Clean up
    let _ = fs::remove_file(git_dir.join("REVERT_HEAD"));
    let _ = fs::remove_file(git_dir.join("ORIG_HEAD"));
    let _ = fs::remove_file(git_dir.join("MERGE_MSG"));

    Ok(0)
}

fn handle_skip(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    if !git_dir.join("REVERT_HEAD").exists() {
        writeln!(err, "error: no revert in progress")?;
        return Ok(1);
    }

    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        let obj = repo.odb().read(&head_oid)?;
        if let Some(Object::Commit(c)) = obj {
            super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
        }
    }

    let _ = fs::remove_file(git_dir.join("REVERT_HEAD"));
    let _ = fs::remove_file(git_dir.join("MERGE_MSG"));

    writeln!(out, "Revert skipped")?;
    Ok(0)
}

fn update_head_to(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let head_ref = RefName::new(BString::from("HEAD"))?;
    let resolved = repo.refs().resolve(&head_ref)?;

    if let Some(git_ref::Reference::Symbolic { target, .. }) = resolved {
        repo.refs().write_ref(&target, oid)?;
    } else {
        repo.refs().write_ref(&head_ref, oid)?;
    }
    Ok(())
}

/// Collect all file paths in a tree (recursively).
fn collect_tree_files(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &str,
) -> Result<HashSet<String>> {
    let mut files = HashSet::new();
    let obj = odb.read(tree_oid)?;
    let tree = match obj {
        Some(Object::Tree(t)) => t,
        _ => return Ok(files),
    };

    for entry in &tree.entries {
        let name = String::from_utf8_lossy(&entry.name);
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        if entry.mode.is_tree() {
            let sub = collect_tree_files(odb, &entry.oid, &path)?;
            files.extend(sub);
        } else {
            files.insert(path);
        }
    }

    Ok(files)
}

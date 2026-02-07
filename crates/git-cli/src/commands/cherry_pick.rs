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
pub struct CherryPickArgs {
    /// Don't automatically commit
    #[arg(short = 'n', long)]
    no_commit: bool,

    /// Edit the commit message
    #[arg(short = 'e', long)]
    edit: bool,

    /// Abort the current cherry-pick
    #[arg(long)]
    abort: bool,

    /// Continue the current cherry-pick
    #[arg(long, name = "continue")]
    continue_: bool,

    /// Skip the current commit
    #[arg(long)]
    skip: bool,

    /// Commits to cherry-pick
    commits: Vec<String>,
}

pub fn run(args: &CherryPickArgs, cli: &Cli) -> Result<i32> {
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
        anyhow::bail!("no commit specified for cherry-pick");
    }

    let options = MergeOptions::default();

    for rev in &args.commits {
        let commit_oid = git_revwalk::resolve_revision(&repo, rev)?;
        let result = cherry_pick_one(&mut repo, &commit_oid, &options, args.no_commit, &mut out, &mut err)?;
        if result != 0 {
            return Ok(result);
        }
    }

    Ok(0)
}

fn cherry_pick_one(
    repo: &mut git_repository::Repository,
    commit_oid: &ObjectId,
    options: &MergeOptions,
    no_commit: bool,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    // Read the commit
    let obj = repo
        .odb()
        .read(commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit not found: {}", commit_oid))?;
    let commit = match obj {
        Object::Commit(c) => c,
        _ => anyhow::bail!("not a commit: {}", commit_oid),
    };

    // Get merge base (parent of the commit being cherry-picked)
    let _base_oid = commit
        .first_parent()
        .ok_or_else(|| anyhow::anyhow!("cannot cherry-pick a root commit"))?;

    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    // Save ORIG_HEAD for abort
    fs::write(git_dir.join("ORIG_HEAD"), head_oid.to_hex())?;

    // Perform the cherry-pick merge
    let result = git_merge::cherry_pick::cherry_pick(repo, commit_oid, options)?;

    if result.is_clean {
        if no_commit {
            // Leave changes staged but don't commit
            if let Some(tree_oid) = result.tree {
                update_index_from_tree(repo, &tree_oid)?;
            }
            writeln!(out, "Cherry-pick applied (not committed)")?;
        } else {
            // Create the commit
            if let Some(tree_oid) = result.tree {
                let message = result.message.unwrap_or_else(|| {
                    String::from_utf8_lossy(&commit.message).to_string()
                });

                let commit_oid_new = create_commit(
                    repo,
                    &tree_oid,
                    &[head_oid],
                    &message,
                    &commit.author,
                )?;

                // Update HEAD
                update_head_to(repo, &commit_oid_new)?;

                // Update worktree to match the new tree
                if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
                    super::reset::checkout_tree_to_worktree(repo.odb(), &tree_oid, &work_tree)?;
                }

                let summary = String::from_utf8_lossy(commit.summary());
                writeln!(
                    out,
                    "[{}] {}",
                    &commit_oid_new.to_hex()[..7],
                    summary
                )?;
            }
        }

        // Clean up state
        let _ = fs::remove_file(git_dir.join("CHERRY_PICK_HEAD"));
        let _ = fs::remove_file(git_dir.join("ORIG_HEAD"));
    } else {
        // Conflict
        fs::write(git_dir.join("CHERRY_PICK_HEAD"), commit_oid.to_hex())?;

        let message = result.message.unwrap_or_else(|| {
            String::from_utf8_lossy(&commit.message).to_string()
        });
        fs::write(git_dir.join("MERGE_MSG"), &message)?;

        writeln!(
            err,
            "error: could not apply {}",
            &commit_oid.to_hex()[..7]
        )?;
        writeln!(
            err,
            "hint: After resolving conflicts, use 'git cherry-pick --continue'"
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
        writeln!(err, "error: no cherry-pick in progress")?;
        return Ok(1);
    }

    let orig_hex = fs::read_to_string(&orig_head_path)?.trim().to_string();
    let orig_oid = ObjectId::from_hex(&orig_hex)?;

    update_head_to(repo, &orig_oid)?;

    // Checkout the original tree
    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        let obj = repo.odb().read(&orig_oid)?;
        if let Some(Object::Commit(c)) = obj {
            super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
        }
    }

    // Clean up
    let _ = fs::remove_file(git_dir.join("CHERRY_PICK_HEAD"));
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
    let cp_head_path = git_dir.join("CHERRY_PICK_HEAD");

    if !cp_head_path.exists() {
        writeln!(err, "error: no cherry-pick in progress")?;
        return Ok(1);
    }

    // Check for unresolved conflicts
    {
        let index = repo.index()?;
        let conflicts = index.conflicts();
        if !conflicts.is_empty() {
            writeln!(err, "error: unresolved conflicts remain")?;
            return Ok(1);
        }
    }

    // Build tree from index
    // Read index from file directly to avoid borrow conflict
    // (repo.index() takes &mut self, write_tree needs repo.odb() which takes &self)
    let index_path = repo.git_dir().join("index");
    let index_for_tree = if index_path.exists() {
        git_index::Index::read_from(&index_path)?
    } else {
        git_index::Index::new()
    };
    let tree_oid = index_for_tree.write_tree(repo.odb())?;

    // Get message
    let msg_path = git_dir.join("MERGE_MSG");
    let message = if msg_path.exists() {
        fs::read_to_string(&msg_path)?
    } else {
        "cherry-pick".to_string()
    };

    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    let new_oid = create_commit_default(repo, &tree_oid, &[head_oid], &message)?;

    update_head_to(repo, &new_oid)?;

    writeln!(out, "[{}] {}", &new_oid.to_hex()[..7], message.lines().next().unwrap_or(""))?;

    // Clean up
    let _ = fs::remove_file(git_dir.join("CHERRY_PICK_HEAD"));
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

    if !git_dir.join("CHERRY_PICK_HEAD").exists() {
        writeln!(err, "error: no cherry-pick in progress")?;
        return Ok(1);
    }

    // Reset to HEAD
    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        let obj = repo.odb().read(&head_oid)?;
        if let Some(Object::Commit(c)) = obj {
            super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
        }
    }

    // Clean up
    let _ = fs::remove_file(git_dir.join("CHERRY_PICK_HEAD"));
    let _ = fs::remove_file(git_dir.join("MERGE_MSG"));

    writeln!(out, "Cherry-pick skipped")?;
    Ok(0)
}

fn update_head_to(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    // If HEAD is symbolic (points to a branch), update the branch ref
    let head_ref = RefName::new(BString::from("HEAD"))?;
    let resolved = repo.refs().resolve(&head_ref)?;

    if let Some(git_ref::Reference::Symbolic { target, .. }) = resolved {
        repo.refs().write_ref(&target, oid)?;
    } else {
        repo.refs().write_ref(&head_ref, oid)?;
    }
    Ok(())
}

fn update_index_from_tree(
    _repo: &mut git_repository::Repository,
    _tree_oid: &ObjectId,
) -> Result<()> {
    // The merge engine already updates the index/worktree
    Ok(())
}

fn create_commit(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    parents: &[ObjectId],
    message: &str,
    original_author: &git_utils::date::Signature,
) -> Result<ObjectId> {
    let committer = super::commit::get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", repo)?;

    let commit = git_object::Commit {
        tree: *tree_oid,
        parents: parents.to_vec(),
        author: original_author.clone(),
        committer,
        message: BString::from(message),
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
    };

    let oid = repo.odb().write(&Object::Commit(commit))?;
    Ok(oid)
}

pub(crate) fn create_commit_default(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    parents: &[ObjectId],
    message: &str,
) -> Result<ObjectId> {
    let author = super::commit::get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", repo)?;
    let committer = author.clone();

    let commit = git_object::Commit {
        tree: *tree_oid,
        parents: parents.to_vec(),
        author,
        committer,
        message: BString::from(message),
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
    };

    let oid = repo.odb().write(&Object::Commit(commit))?;
    Ok(oid)
}

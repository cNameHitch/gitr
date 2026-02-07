use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_index::Index;
use git_merge::MergeOptions;
use git_object::{Commit, Object};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RebaseArgs {
    /// Rebase onto a different base
    #[arg(long)]
    onto: Option<String>,

    /// Abort the current rebase
    #[arg(long)]
    abort: bool,

    /// Continue the rebase after resolving conflicts
    #[arg(long)]
    r#continue: bool,

    /// Skip the current patch
    #[arg(long)]
    skip: bool,

    /// Interactive rebase
    #[arg(short, long)]
    interactive: bool,

    /// Upstream branch
    upstream: Option<String>,
}

pub fn run(args: &RebaseArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if args.abort {
        return rebase_abort(&mut repo, &mut err);
    }

    if args.r#continue {
        return rebase_continue(&mut repo, &mut err);
    }

    if args.interactive {
        bail!("interactive rebase is not yet implemented");
    }

    // Start a new rebase
    let upstream = args.upstream.as_deref()
        .ok_or_else(|| anyhow::anyhow!("fatal: no upstream configured for the current branch"))?;

    let onto_spec = args.onto.as_deref().unwrap_or(upstream);
    let onto_oid = git_revwalk::resolve_revision(&repo, onto_spec)?;
    let upstream_oid = git_revwalk::resolve_revision(&repo, upstream)?;

    let head_oid = repo.head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD is not valid"))?;

    // Save original HEAD
    let git_dir = repo.git_dir().to_path_buf();
    std::fs::write(git_dir.join("ORIG_HEAD"), head_oid.to_hex())?;

    // Find merge base
    let base = git_revwalk::merge_base_one(&repo, &head_oid, &upstream_oid)?;

    // Collect commits to replay (from base to HEAD)
    let commits_to_replay = collect_commits(&repo, &base, &head_oid)?;

    if commits_to_replay.is_empty() {
        writeln!(err, "Current branch is up to date.")?;
        return Ok(0);
    }

    writeln!(err, "Rebasing ({} commits)...", commits_to_replay.len())?;

    // Save rebase state
    let rebase_dir = git_dir.join("rebase-merge");
    std::fs::create_dir_all(&rebase_dir)?;
    std::fs::write(rebase_dir.join("onto"), onto_oid.to_hex())?;
    std::fs::write(rebase_dir.join("head-name"),
        repo.current_branch()?.unwrap_or_else(|| "HEAD".to_string()))?;
    std::fs::write(rebase_dir.join("orig-head"), head_oid.to_hex())?;

    // Move HEAD to onto
    super::reset::update_head(&repo, &onto_oid)?;

    // Cherry-pick each commit
    let mut current = onto_oid;
    for commit_oid in &commits_to_replay {
        match cherry_pick(&mut repo, &current, commit_oid)? {
            Some(new_oid) => {
                current = new_oid;
                super::reset::update_head(&repo, &current)?;
            }
            None => {
                // Conflict
                writeln!(err, "CONFLICT: resolve conflicts and run 'git rebase --continue'")?;
                // Save remaining commits
                let remaining: Vec<String> = commits_to_replay.iter()
                    .map(|o| o.to_hex())
                    .collect();
                std::fs::write(rebase_dir.join("todo"), remaining.join("\n"))?;
                std::fs::write(rebase_dir.join("current"), current.to_hex())?;
                return Ok(1);
            }
        }
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&rebase_dir);
    writeln!(err, "Successfully rebased.")?;

    Ok(0)
}

fn rebase_abort(repo: &mut git_repository::Repository, err: &mut impl Write) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();
    let orig_head_path = git_dir.join("ORIG_HEAD");
    if !orig_head_path.exists() {
        bail!("fatal: no rebase in progress");
    }

    let orig_head_hex = std::fs::read_to_string(&orig_head_path)?.trim().to_string();
    let orig_head = ObjectId::from_hex(&orig_head_hex)?;

    super::reset::update_head(repo, &orig_head)?;

    // Reset index and working tree
    let obj = repo.odb().read(&orig_head)?
        .ok_or_else(|| anyhow::anyhow!("object not found"))?;
    let tree_oid = match obj {
        Object::Commit(c) => c.tree,
        _ => bail!("expected commit"),
    };
    let index_path = repo.git_dir().join("index");
    let mut new_index = Index::new();
    super::reset::build_index_from_tree(repo.odb(), &tree_oid, &BString::from(""), &mut new_index)?;
    new_index.write_to(&index_path)?;

    if let Some(wt) = repo.work_tree() {
        super::reset::checkout_tree_to_worktree(repo.odb(), &tree_oid, wt)?;
    }

    let _ = std::fs::remove_dir_all(git_dir.join("rebase-merge"));
    let _ = std::fs::remove_file(&orig_head_path);
    writeln!(err, "Rebase aborted.")?;
    Ok(0)
}

fn rebase_continue(repo: &mut git_repository::Repository, err: &mut impl Write) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();
    let rebase_dir = git_dir.join("rebase-merge");
    if !rebase_dir.exists() {
        bail!("fatal: no rebase in progress");
    }

    writeln!(err, "Continuing rebase...")?;

    // Read remaining TODO
    let todo = std::fs::read_to_string(rebase_dir.join("todo"))?;
    let remaining: Vec<ObjectId> = todo.lines()
        .filter(|l| !l.is_empty())
        .map(|l| ObjectId::from_hex(l.trim()))
        .collect::<Result<Vec<_>, _>>()?;

    let current_hex = std::fs::read_to_string(rebase_dir.join("current"))?.trim().to_string();
    let mut current = ObjectId::from_hex(&current_hex)?;

    for commit_oid in &remaining {
        match cherry_pick(repo, &current, commit_oid)? {
            Some(new_oid) => {
                current = new_oid;
                super::reset::update_head(repo, &current)?;
            }
            None => {
                writeln!(err, "CONFLICT: resolve and continue")?;
                return Ok(1);
            }
        }
    }

    let _ = std::fs::remove_dir_all(&rebase_dir);
    let _ = std::fs::remove_file(git_dir.join("ORIG_HEAD"));
    writeln!(err, "Successfully rebased.")?;
    Ok(0)
}

fn collect_commits(
    repo: &git_repository::Repository,
    base: &Option<ObjectId>,
    head: &ObjectId,
) -> Result<Vec<ObjectId>> {
    let mut commits = Vec::new();
    let mut current = *head;

    loop {
        if let Some(ref b) = base {
            if current == *b {
                break;
            }
        }
        commits.push(current);
        let obj = repo.odb().read(&current)?
            .ok_or_else(|| anyhow::anyhow!("commit not found"))?;
        let commit = match obj {
            Object::Commit(c) => c,
            _ => bail!("expected commit"),
        };
        current = match commit.first_parent() {
            Some(p) => *p,
            None => break,
        };
    }

    commits.reverse();
    Ok(commits)
}

fn cherry_pick(
    repo: &mut git_repository::Repository,
    onto: &ObjectId,
    commit_oid: &ObjectId,
) -> Result<Option<ObjectId>> {
    let obj = repo.odb().read(commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit not found"))?;
    let commit = match obj {
        Object::Commit(c) => c,
        _ => bail!("expected commit"),
    };

    let parent_oid = commit.first_parent()
        .ok_or_else(|| anyhow::anyhow!("cannot cherry-pick a root commit"))?;

    // 3-way merge: base=parent, ours=onto, theirs=commit
    let options = MergeOptions::default();
    let result = git_merge::strategy::dispatch_merge(
        repo, onto, commit_oid, parent_oid, &options,
    )?;

    if result.is_clean {
        if let Some(tree_oid) = result.tree {
            // Create new commit
            let new_commit = Commit {
                tree: tree_oid,
                parents: vec![*onto],
                author: commit.author.clone(),
                committer: build_committer(repo)?,
                message: commit.message.clone(),
                encoding: commit.encoding.clone(),
                gpgsig: None,
                extra_headers: Vec::new(),
            };
            let new_oid = repo.odb().write(&Object::Commit(new_commit))?;
            Ok(Some(new_oid))
        } else {
            bail!("merge produced no tree");
        }
    } else {
        Ok(None) // Conflict
    }
}

fn build_committer(repo: &git_repository::Repository) -> Result<Signature> {
    let name = std::env::var("GIT_COMMITTER_NAME")
        .or_else(|_| repo.config().get_string("user.name").ok().flatten().ok_or(std::env::VarError::NotPresent))
        .unwrap_or_else(|_| "Unknown".to_string());
    let email = std::env::var("GIT_COMMITTER_EMAIL")
        .or_else(|_| repo.config().get_string("user.email").ok().flatten().ok_or(std::env::VarError::NotPresent))
        .unwrap_or_else(|_| "unknown@unknown".to_string());
    let date = if let Ok(date_str) = std::env::var("GIT_COMMITTER_DATE") {
        GitDate::parse_raw(&date_str)?
    } else {
        GitDate::now()
    };
    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date,
    })
}

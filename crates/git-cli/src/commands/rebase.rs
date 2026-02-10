use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_index::Index;
use git_merge::MergeOptions;
use git_object::{Commit, Object};
use git_ref::RefName;
use git_ref::reflog::{ReflogEntry, append_reflog_entry};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RebaseArgs {
    /// Rebase onto a different base
    #[arg(long)]
    pub(crate) onto: Option<String>,

    /// Abort the current rebase
    #[arg(long)]
    pub(crate) abort: bool,

    /// Continue the rebase after resolving conflicts
    #[arg(long)]
    pub(crate) r#continue: bool,

    /// Skip the current patch
    #[arg(long)]
    pub(crate) skip: bool,

    /// Interactive rebase (stub for Phase 8)
    #[arg(short, long)]
    pub(crate) interactive: bool,

    /// Be quiet
    #[arg(short, long)]
    pub(crate) quiet: bool,

    /// Be verbose
    #[arg(short, long)]
    pub(crate) verbose: bool,

    /// Add Signed-off-by trailer to commits
    #[arg(long)]
    pub(crate) signoff: bool,

    /// Force rebase even if current branch is up to date
    #[arg(short = 'f', long)]
    pub(crate) force_rebase: bool,

    /// Automatically squash fixup commits
    #[arg(long)]
    pub(crate) autosquash: bool,

    /// Do not automatically squash fixup commits
    #[arg(long)]
    pub(crate) no_autosquash: bool,

    /// Automatically stash/unstash before and after
    #[arg(long)]
    pub(crate) autostash: bool,

    /// Do not automatically stash/unstash
    #[arg(long)]
    pub(crate) no_autostash: bool,

    /// Automatically update refs that point to rebased commits
    #[arg(long)]
    pub(crate) update_refs: bool,

    /// Execute a shell command after each commit
    #[arg(short = 'x', long)]
    pub(crate) exec: Option<String>,

    /// Rebase all reachable commits from root
    #[arg(long)]
    pub(crate) root: bool,

    /// Merge strategy to use
    #[arg(short = 's', long = "strategy")]
    pub(crate) strategy: Option<String>,

    /// Pass option to the merge strategy
    #[arg(short = 'X', long = "strategy-option")]
    pub(crate) strategy_option: Vec<String>,

    /// Upstream branch
    pub(crate) upstream: Option<String>,
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
    {
        let sig = build_committer(&repo)?;
        let entry = ReflogEntry {
            old_oid: head_oid,
            new_oid: onto_oid,
            identity: sig,
            message: BString::from(format!("rebase (start): checkout {}", onto_spec)),
        };
        let head_ref = RefName::new(BString::from("HEAD"))?;
        append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
    }

    // Cherry-pick each commit
    let total = commits_to_replay.len();
    let mut current = onto_oid;
    let mut prev_oid = onto_oid;
    for (i, commit_oid) in commits_to_replay.iter().enumerate() {
        writeln!(err, "Rebasing ({}/{})", i + 1, total)?;
        match cherry_pick(&mut repo, &current, commit_oid)? {
            Some(new_oid) => {
                current = new_oid;
                super::reset::update_head(&repo, &current)?;
                {
                    if let Some(Object::Commit(c)) = repo.odb().read(commit_oid)? {
                        let sig = build_committer(&repo)?;
                        let subject = String::from_utf8_lossy(c.summary()).to_string();
                        let entry = ReflogEntry {
                            old_oid: prev_oid,
                            new_oid,
                            identity: sig,
                            message: BString::from(format!("rebase: {}", subject)),
                        };
                        let head_ref = RefName::new(BString::from("HEAD"))?;
                        append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
                    }
                    prev_oid = new_oid;
                }
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

    // Record rebase finish in reflog
    {
        let sig = build_committer(&repo)?;
        let head_name = repo.current_branch()?.unwrap_or_else(|| "HEAD".to_string());
        let entry = ReflogEntry {
            old_oid: onto_oid,
            new_oid: current,
            identity: sig,
            message: BString::from(format!("rebase (finish): returning to refs/heads/{}", head_name)),
        };
        let head_ref = RefName::new(BString::from("HEAD"))?;
        append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
    }

    // Clean up
    let head_name = std::fs::read_to_string(rebase_dir.join("head-name")).unwrap_or_default().trim().to_string();
    let _ = std::fs::remove_dir_all(&rebase_dir);
    if head_name.is_empty() {
        writeln!(err, "Successfully rebased.")?;
    } else {
        writeln!(err, "Successfully rebased and updated refs/heads/{}.", head_name)?;
    }

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

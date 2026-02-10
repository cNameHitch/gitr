use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct BranchArgs {
    /// Delete a branch
    #[arg(short, long)]
    delete: bool,

    /// Force delete a branch
    #[arg(short = 'D')]
    force_delete: bool,

    /// Move/rename a branch
    #[arg(short = 'm', long)]
    r#move: bool,

    /// Force move/rename
    #[arg(short = 'M')]
    force_move: bool,

    /// List both remote-tracking and local branches
    #[arg(short, long)]
    all: bool,

    /// List remote-tracking branches
    #[arg(short, long)]
    remotes: bool,

    /// Show branch details (verbose)
    #[arg(short = 'v', long)]
    verbose: bool,

    /// List branches matching pattern
    #[arg(long)]
    list: bool,

    /// Format string for branch listing
    #[arg(long)]
    format: Option<String>,

    /// Only list branches which contain the specified commit
    #[arg(long)]
    contains: Option<String>,

    /// Show current branch
    #[arg(long)]
    show_current: bool,

    /// Branch name (for create/delete/rename)
    name: Option<String>,

    /// Start point or new name (for create/rename)
    start_point: Option<String>,
}

pub fn run(args: &BranchArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.show_current {
        if let Ok(Some(branch)) = repo.current_branch() {
            writeln!(out, "{}", branch)?;
        }
        return Ok(0);
    }

    if args.delete || args.force_delete {
        let name = args.name.as_deref()
            .ok_or_else(|| anyhow::anyhow!("branch name required"))?;
        return delete_branch(&repo, name, args.force_delete, &mut out);
    }

    if args.r#move || args.force_move {
        return rename_branch(&repo, args, &mut out);
    }

    // If a name is given without flags, create a new branch
    if let Some(ref name) = args.name {
        return create_branch(&repo, name, args.start_point.as_deref());
    }

    // Default: list branches
    list_branches(&repo, args, &mut out)
}

fn create_branch(repo: &git_repository::Repository, name: &str, start: Option<&str>) -> Result<i32> {
    let refname = RefName::new(BString::from(format!("refs/heads/{}", name)))?;

    // Check if branch already exists
    if repo.refs().resolve(&refname)?.is_some() {
        bail!("fatal: a branch named '{}' already exists", name);
    }

    // Resolve start point
    let oid = if let Some(spec) = start {
        git_revwalk::resolve_revision(repo, spec)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("fatal: not a valid object name: 'HEAD'"))?
    };

    repo.refs().write_ref(&refname, &oid)?;
    Ok(0)
}

fn delete_branch(
    repo: &git_repository::Repository,
    name: &str,
    force: bool,
    out: &mut impl Write,
) -> Result<i32> {
    let refname = RefName::new(BString::from(format!("refs/heads/{}", name)))?;

    let reference = match repo.refs().resolve(&refname)? {
        Some(r) => r,
        None => {
            eprintln!("error: branch '{}' not found.", name);
            return Ok(1);
        }
    };

    // Check if it's the current branch
    if let Ok(Some(current)) = repo.current_branch() {
        if current == name {
            bail!("error: Cannot delete branch '{}' checked out at '{}'", name,
                repo.work_tree().map(|p| p.display().to_string()).unwrap_or_default());
        }
    }

    if !force {
        // Check if branch is fully merged into HEAD
        if let (Some(branch_oid), Some(head_oid)) = (reference.target_oid(), repo.head_oid()?) {
            if !git_revwalk::is_ancestor(repo, &branch_oid, &head_oid)? {
                bail!("error: The branch '{}' is not fully merged.\nIf you are sure you want to delete it, run 'git branch -D {}'", name, name);
            }
        }
    }

    let oid = reference.target_oid()
        .map(|o| o.to_hex())
        .unwrap_or_else(|| "?".to_string());

    repo.refs().delete_ref(&refname)?;
    writeln!(out, "Deleted branch {} (was {}).", name, &oid[..7])?;
    Ok(0)
}

fn rename_branch(
    repo: &git_repository::Repository,
    args: &BranchArgs,
    _out: &mut impl Write,
) -> Result<i32> {
    let old_name = args.name.as_deref()
        .ok_or_else(|| anyhow::anyhow!("branch name required"))?;
    let new_name = args.start_point.as_deref()
        .ok_or_else(|| anyhow::anyhow!("new branch name required"))?;

    let old_ref = RefName::new(BString::from(format!("refs/heads/{}", old_name)))?;
    let new_ref = RefName::new(BString::from(format!("refs/heads/{}", new_name)))?;

    // Check old branch exists
    let reference = repo.refs().resolve(&old_ref)?
        .ok_or_else(|| anyhow::anyhow!("error: branch '{}' not found", old_name))?;

    // Check new name doesn't exist (unless force)
    if !args.force_move && repo.refs().resolve(&new_ref)?.is_some() {
        bail!("fatal: a branch named '{}' already exists", new_name);
    }

    let oid = reference.peel_to_oid(repo.refs())?;

    // Create new ref, delete old
    repo.refs().write_ref(&new_ref, &oid)?;
    repo.refs().delete_ref(&old_ref)?;

    // Update HEAD if needed
    if let Ok(Some(current)) = repo.current_branch() {
        if current == old_name {
            let head = RefName::new(BString::from("HEAD"))?;
            repo.refs().write_symbolic_ref(&head, &new_ref)?;
        }
    }

    Ok(0)
}

fn list_branches(
    repo: &git_repository::Repository,
    args: &BranchArgs,
    out: &mut impl Write,
) -> Result<i32> {
    let current_branch = repo.current_branch().unwrap_or(None);

    // Resolve --contains commit if provided
    let contains_oid = if let Some(ref contains_spec) = args.contains {
        Some(git_revwalk::resolve_revision(repo, contains_spec)?)
    } else {
        None
    };

    if !args.remotes || args.all {
        // List local branches - collect first for alignment
        let refs = repo.refs().iter(Some("refs/heads/"))?;
        let mut branches: Vec<(String, bool)> = Vec::new();
        let mut ref_map: Vec<git_ref::Reference> = Vec::new();
        for r in refs {
            let r = r?;
            let full_name = r.name().as_str().to_string();
            let short = full_name.strip_prefix("refs/heads/").unwrap_or(&full_name).to_string();
            let is_current = current_branch.as_deref() == Some(short.as_str());

            // Filter by --contains: skip branches that don't contain the specified commit
            if let Some(ref target_oid) = contains_oid {
                if let Ok(branch_oid) = r.peel_to_oid(repo.refs()) {
                    if !git_revwalk::is_ancestor(repo, target_oid, &branch_oid).unwrap_or(false) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            branches.push((short, is_current));
            ref_map.push(r);
        }

        // Find max branch name length for alignment in verbose mode
        let max_name_len = if args.verbose {
            branches.iter().map(|(name, _)| name.len()).max().unwrap_or(0)
        } else {
            0
        };

        for (i, (short, is_current)) in branches.iter().enumerate() {
            let prefix = if *is_current { "* " } else { "  " };

            if args.verbose {
                if let Ok(oid) = ref_map[i].peel_to_oid(repo.refs()) {
                    let hex = oid.to_hex();
                    let short_hash = &hex[..7.min(hex.len())];
                    let subject = match repo.odb().read(&oid) {
                        Ok(Some(git_object::Object::Commit(c))) => {
                            String::from_utf8_lossy(c.summary()).to_string()
                        }
                        _ => String::new(),
                    };
                    writeln!(out, "{}{:<width$} {} {}", prefix, short, short_hash, subject, width = max_name_len)?;
                } else {
                    writeln!(out, "{}{}", prefix, short)?;
                }
            } else {
                writeln!(out, "{}{}", prefix, short)?;
            }
        }
    }

    if args.remotes || args.all {
        let refs = repo.refs().iter(Some("refs/remotes/"))?;
        for r in refs {
            let r = r?;
            let full_name = r.name().as_str().to_string();
            let short = full_name.strip_prefix("refs/remotes/").unwrap_or(&full_name);
            writeln!(out, "  {}", short)?;
        }
    }

    Ok(0)
}

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};
use bstr::BString;
use clap::{Args, Subcommand};
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct WorktreeArgs {
    #[command(subcommand)]
    command: WorktreeSubcommand,
}

#[derive(Subcommand)]
pub enum WorktreeSubcommand {
    /// Create a new working tree
    Add {
        /// Path to the new worktree
        path: String,

        /// Branch to check out (or create)
        branch: Option<String>,

        /// Create a new branch
        #[arg(short = 'b')]
        new_branch: Option<String>,

        /// Force creation even if branch is checked out elsewhere
        #[arg(short, long)]
        force: bool,

        /// Detach HEAD at the given commit
        #[arg(long)]
        detach: bool,
    },

    /// List working trees
    List {
        /// Machine-readable porcelain format
        #[arg(long)]
        porcelain: bool,
    },

    /// Remove a working tree
    Remove {
        /// Path to the worktree
        worktree: String,

        /// Force removal even if dirty
        #[arg(short, long)]
        force: bool,
    },

    /// Lock a working tree to prevent pruning
    Lock {
        /// Path to the worktree
        worktree: String,

        /// Reason for locking
        #[arg(long)]
        reason: Option<String>,
    },

    /// Unlock a working tree
    Unlock {
        /// Path to the worktree
        worktree: String,
    },

    /// Move a working tree to a new location
    Move {
        /// Current worktree path
        worktree: String,

        /// New path
        new_path: String,

        /// Force move even if worktree is locked
        #[arg(short, long)]
        force: bool,
    },

    /// Remove stale working tree admin data
    Prune {
        /// Dry run
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Verbose
        #[arg(short, long)]
        verbose: bool,

        /// Expire threshold
        #[arg(long)]
        expire: Option<String>,
    },
}

pub fn run(args: &WorktreeArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        WorktreeSubcommand::Add {
            path,
            branch,
            new_branch,
            force,
            detach,
        } => worktree_add(cli, path, branch.as_deref(), new_branch.as_deref(), *force, *detach),
        WorktreeSubcommand::List { porcelain } => worktree_list(cli, *porcelain),
        WorktreeSubcommand::Remove { worktree, force } => worktree_remove(cli, worktree, *force),
        WorktreeSubcommand::Lock { worktree, reason } => {
            worktree_lock(cli, worktree, reason.as_deref())
        }
        WorktreeSubcommand::Unlock { worktree } => worktree_unlock(cli, worktree),
        WorktreeSubcommand::Move {
            worktree,
            new_path,
            force,
        } => worktree_move(cli, worktree, new_path, *force),
        WorktreeSubcommand::Prune {
            dry_run,
            verbose,
            expire,
        } => worktree_prune(cli, *dry_run, *verbose, expire.as_deref()),
    }
}

fn worktree_add(
    cli: &Cli,
    path: &str,
    branch: Option<&str>,
    new_branch: Option<&str>,
    force: bool,
    detach: bool,
) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let wt_path = PathBuf::from(path);
    if wt_path.exists() && !force {
        bail!("'{}' already exists", path);
    }

    let git_dir = repo.git_dir().to_path_buf();
    let common_dir = repo.common_dir().to_path_buf();
    let worktrees_dir = git_dir.join("worktrees");

    // Determine the worktree name from the path
    let wt_name = wt_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("invalid worktree path"))?
        .to_string_lossy()
        .to_string();

    // Create worktree admin dir
    let wt_admin_dir = worktrees_dir.join(&wt_name);
    std::fs::create_dir_all(&wt_admin_dir)?;

    // Resolve commit/branch
    let branch_name = new_branch.or(branch).unwrap_or(&wt_name);
    let checkout_oid = if let Some(b) = branch {
        git_revwalk::resolve_revision(&repo, b)?
    } else if let Some(head) = repo.head_oid()? {
        head
    } else {
        bail!("HEAD does not point to a valid commit");
    };

    // If creating a new branch, write the ref
    if new_branch.is_some() {
        let refname = RefName::new(BString::from(format!("refs/heads/{}", branch_name)))?;
        repo.refs().write_ref(&refname, &checkout_oid)?;
    }

    // Create the worktree directory
    std::fs::create_dir_all(&wt_path)?;

    // Write the .git file in the worktree pointing to the admin dir
    let git_file_content = format!("gitdir: {}\n", wt_admin_dir.display());
    std::fs::write(wt_path.join(".git"), &git_file_content)?;

    // Write gitdir in admin dir pointing back to the worktree
    let abs_wt_path = std::fs::canonicalize(&wt_path)?;
    std::fs::write(
        wt_admin_dir.join("gitdir"),
        format!("{}\n", abs_wt_path.join(".git").display()),
    )?;

    // Write commondir
    std::fs::write(wt_admin_dir.join("commondir"), format!("{}\n", common_dir.display()))?;

    // Write HEAD
    if detach {
        std::fs::write(wt_admin_dir.join("HEAD"), format!("{}\n", checkout_oid.to_hex()))?;
    } else {
        std::fs::write(
            wt_admin_dir.join("HEAD"),
            format!("ref: refs/heads/{}\n", branch_name),
        )?;
    }

    writeln!(
        err,
        "Preparing worktree ({})",
        if new_branch.is_some() {
            format!("new branch '{}'", branch_name)
        } else if detach {
            format!("detached HEAD {}", &checkout_oid.to_hex()[..7])
        } else {
            format!("branch '{}'", branch_name)
        }
    )?;

    // Checkout the tree to the new worktree
    if let Some(Object::Commit(commit)) = repo.odb().read(&checkout_oid)?.as_ref() {
        super::reset::checkout_tree_to_worktree(repo.odb(), &commit.tree, &wt_path)?;
    }

    writeln!(err, "HEAD is now at {}", &checkout_oid.to_hex()[..7])?;

    Ok(0)
}

fn worktree_list(cli: &Cli, porcelain: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let git_dir = repo.git_dir();

    // The main worktree
    if let Some(wt) = repo.work_tree() {
        let head = repo.head_oid()?.map(|h| h.to_hex()).unwrap_or_default();
        let branch = repo
            .current_branch()?
            .unwrap_or_else(|| "(detached HEAD)".to_string());

        if porcelain {
            writeln!(out, "worktree {}", wt.display())?;
            writeln!(out, "HEAD {}", head)?;
            writeln!(out, "branch refs/heads/{}", branch)?;
            writeln!(out)?;
        } else {
            writeln!(
                out,
                "{:<40} {} [{}]",
                wt.display(),
                &head[..7.min(head.len())],
                branch
            )?;
        }
    }

    // Linked worktrees
    let worktrees_dir = git_dir.join("worktrees");
    if worktrees_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&worktrees_dir)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let gitdir_file = path.join("gitdir");
            if !gitdir_file.exists() {
                continue;
            }

            let wt_path = std::fs::read_to_string(&gitdir_file)
                .unwrap_or_default()
                .trim()
                .to_string();
            // gitdir points to <worktree>/.git, so get the parent
            let wt_path = PathBuf::from(&wt_path);
            let wt_dir = wt_path.parent().unwrap_or(&wt_path);

            let head_file = path.join("HEAD");
            let head_content = std::fs::read_to_string(&head_file).unwrap_or_default();
            let head_content = head_content.trim();

            let (head_display, branch_display) = if let Some(ref_target) = head_content.strip_prefix("ref: ") {
                let branch = ref_target.strip_prefix("refs/heads/").unwrap_or(ref_target);
                // Resolve the ref to get HEAD oid
                let head_hex = if let Ok(refname) = RefName::new(BString::from(ref_target)) {
                    match repo.refs().resolve_to_oid(&refname) {
                        Ok(Some(oid)) => oid.to_hex(),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                (head_hex, branch.to_string())
            } else {
                (head_content.to_string(), "(detached HEAD)".to_string())
            };

            // Check if locked
            let locked = path.join("locked").exists();

            if porcelain {
                writeln!(out, "worktree {}", wt_dir.display())?;
                writeln!(out, "HEAD {}", head_display)?;
                if branch_display == "(detached HEAD)" {
                    writeln!(out, "detached")?;
                } else {
                    writeln!(out, "branch refs/heads/{}", branch_display)?;
                }
                if locked {
                    writeln!(out, "locked")?;
                }
                writeln!(out)?;
            } else {
                writeln!(
                    out,
                    "{:<40} {} [{}]{}",
                    wt_dir.display(),
                    &head_display[..7usize.min(head_display.len())],
                    branch_display,
                    if locked { " locked" } else { "" }
                )?;
            }
        }
    }

    Ok(0)
}

fn worktree_remove(cli: &Cli, worktree: &str, force: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir();
    let worktrees_dir = git_dir.join("worktrees");

    // Find the worktree admin dir
    let wt_name = PathBuf::from(worktree)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| worktree.to_string());

    let wt_admin_dir = worktrees_dir.join(&wt_name);
    if !wt_admin_dir.is_dir() {
        bail!("'{}' is not a valid worktree", worktree);
    }

    // Check if locked
    if wt_admin_dir.join("locked").exists() && !force {
        bail!("'{}' is locked, use --force to remove", worktree);
    }

    // Remove the worktree directory
    let wt_path = PathBuf::from(worktree);
    if wt_path.exists() {
        std::fs::remove_dir_all(&wt_path)?;
    }

    // Remove the admin dir
    std::fs::remove_dir_all(&wt_admin_dir)?;

    Ok(0)
}

fn worktree_lock(cli: &Cli, worktree: &str, reason: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir();
    let worktrees_dir = git_dir.join("worktrees");

    let wt_name = PathBuf::from(worktree)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| worktree.to_string());

    let wt_admin_dir = worktrees_dir.join(&wt_name);
    if !wt_admin_dir.is_dir() {
        bail!("'{}' is not a valid worktree", worktree);
    }

    let lock_file = wt_admin_dir.join("locked");
    if lock_file.exists() {
        bail!("'{}' is already locked", worktree);
    }

    let reason_text = reason.unwrap_or("");
    std::fs::write(&lock_file, reason_text)?;

    Ok(0)
}

fn worktree_unlock(cli: &Cli, worktree: &str) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir();
    let worktrees_dir = git_dir.join("worktrees");

    let wt_name = PathBuf::from(worktree)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| worktree.to_string());

    let wt_admin_dir = worktrees_dir.join(&wt_name);
    let lock_file = wt_admin_dir.join("locked");

    if !lock_file.exists() {
        bail!("'{}' is not locked", worktree);
    }

    std::fs::remove_file(&lock_file)?;

    Ok(0)
}

fn worktree_move(cli: &Cli, worktree: &str, new_path: &str, force: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir();
    let worktrees_dir = git_dir.join("worktrees");

    let wt_name = PathBuf::from(worktree)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| worktree.to_string());

    let wt_admin_dir = worktrees_dir.join(&wt_name);
    if !wt_admin_dir.is_dir() {
        bail!("'{}' is not a valid worktree", worktree);
    }

    // Check if locked
    if wt_admin_dir.join("locked").exists() && !force {
        bail!("'{}' is locked, use --force to move", worktree);
    }

    let src = PathBuf::from(worktree);
    let dst = PathBuf::from(new_path);

    if dst.exists() {
        bail!("'{}' already exists", new_path);
    }

    // Move the worktree directory
    std::fs::rename(&src, &dst)?;

    // Update the gitdir pointer in the admin dir
    let abs_dst = std::fs::canonicalize(&dst)?;
    std::fs::write(
        wt_admin_dir.join("gitdir"),
        format!("{}\n", abs_dst.join(".git").display()),
    )?;

    // Update the .git file in the worktree
    std::fs::write(
        abs_dst.join(".git"),
        format!("gitdir: {}\n", wt_admin_dir.display()),
    )?;

    Ok(0)
}

fn worktree_prune(cli: &Cli, dry_run: bool, verbose: bool, _expire: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let git_dir = repo.git_dir();
    let worktrees_dir = git_dir.join("worktrees");

    if !worktrees_dir.is_dir() {
        return Ok(0);
    }

    let entries: Vec<_> = std::fs::read_dir(&worktrees_dir)?
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let gitdir_file = path.join("gitdir");

        let should_remove = if !gitdir_file.exists() {
            Some("gitdir file is missing")
        } else {
            let wt_path = std::fs::read_to_string(&gitdir_file)
                .unwrap_or_default()
                .trim()
                .to_string();
            if !PathBuf::from(&wt_path).exists() {
                Some("gitdir points to non-existing location")
            } else {
                None
            }
        };

        if let Some(reason) = should_remove {
            // Check if locked
            if path.join("locked").exists() {
                if verbose {
                    writeln!(err, "Skipping locked worktree '{}'", name)?;
                }
                continue;
            }

            if verbose || dry_run {
                writeln!(err, "Removing worktrees/{}: {}", name, reason)?;
            }

            if !dry_run {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }

    Ok(0)
}

use git_object::Object;

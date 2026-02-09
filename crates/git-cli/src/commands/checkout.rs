use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;
use super::switch;
use super::restore;

#[derive(Args)]
pub struct CheckoutArgs {
    /// Create a new branch and switch to it
    #[arg(short, long, value_name = "new-branch")]
    b: Option<String>,

    /// Create or reset a branch and switch to it
    #[arg(short = 'B', value_name = "new-branch")]
    force_b: Option<String>,

    /// Detach HEAD at the named commit
    #[arg(long)]
    detach: bool,

    /// Force checkout (discard local changes)
    #[arg(short, long)]
    force: bool,

    /// Target branch, commit, or file
    target: Option<String>,

    /// Additional paths (when checking out files)
    #[arg(last = true)]
    paths: Vec<String>,
}

pub fn run(args: &CheckoutArgs, cli: &Cli) -> Result<i32> {
    // If paths are given after --, this is a file checkout (restore)
    if !args.paths.is_empty() {
        let mut files = args.paths.clone();
        if let Some(ref t) = args.target {
            files.insert(0, t.clone());
        }
        let restore_args = restore::RestoreArgs::from_paths(files);
        return restore::run(&restore_args, cli);
    }

    // If -b is given, create and switch
    if let Some(ref new_branch) = args.b {
        let switch_args = switch::SwitchArgs {
            create: Some(new_branch.clone()),
            force_create: None,
            detach: false,
            force: args.force,
            target: args.target.clone(),
        };
        return switch::run(&switch_args, cli);
    }

    if let Some(ref new_branch) = args.force_b {
        let switch_args = switch::SwitchArgs {
            create: None,
            force_create: Some(new_branch.clone()),
            detach: false,
            force: args.force,
            target: args.target.clone(),
        };
        return switch::run(&switch_args, cli);
    }

    // Check if target is a file or a branch
    if let Some(ref target) = args.target {
        let repo = open_repo(cli)?;

        // Check if target is a known branch
        let refname = RefName::new(BString::from(format!("refs/heads/{}", target)))?;
        let is_branch = repo.refs().resolve(&refname)?.is_some();

        // Check if target is a file
        let work_tree = repo.work_tree().map(|p| p.to_path_buf());
        let is_file = work_tree.as_ref()
            .map(|wt| wt.join(target).exists())
            .unwrap_or(false);

        drop(repo);

        if is_branch && !is_file {
            let switch_args = switch::SwitchArgs {
                create: None,
                force_create: None,
                detach: args.detach,
                force: args.force,
                target: Some(target.clone()),
            };
            return switch::run(&switch_args, cli);
        } else if is_file && !is_branch {
            // Restore the file from index
            let restore_args = restore::RestoreArgs::from_paths(vec![target.clone()]);
            return restore::run(&restore_args, cli);
        } else if is_branch {
            // Prefer branch interpretation
            let switch_args = switch::SwitchArgs {
                create: None,
                force_create: None,
                detach: args.detach,
                force: args.force,
                target: Some(target.clone()),
            };
            return switch::run(&switch_args, cli);
        } else {
            // Try as a commit
            let switch_args = switch::SwitchArgs {
                create: None,
                force_create: None,
                detach: true,
                force: args.force,
                target: Some(target.clone()),
            };
            match switch::run(&switch_args, cli) {
                Ok(code) => return Ok(code),
                Err(_) => {
                    eprintln!("error: pathspec '{}' did not match any file(s) known to git", target);
                    return Ok(1);
                }
            }
        }
    }

    anyhow::bail!("you must specify a branch or commit to checkout");
}

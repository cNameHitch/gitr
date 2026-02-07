use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_index::Stage;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RmArgs {
    /// Only remove from the index (keep in working tree)
    #[arg(long)]
    cached: bool,

    /// Override the up-to-date check
    #[arg(short, long)]
    force: bool,

    /// Allow recursive removal
    #[arg(short, long)]
    r: bool,

    /// Don't actually remove anything, just show what would be done
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Be quiet
    #[arg(short, long)]
    quiet: bool,

    /// Files to remove
    #[arg(required = true)]
    files: Vec<String>,
}

pub fn run(args: &RmArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    let stderr = io::stderr();
    let mut err_out = stderr.lock();

    for file in &args.files {
        let rel_path = BString::from(file.as_bytes());

        // Check if it's in the index
        {
            let index = repo.index()?;
            if index.get(rel_path.as_ref(), Stage::Normal).is_none() {
                bail!("pathspec '{}' did not match any files", file);
            }
        }

        if !args.quiet && !args.dry_run {
            writeln!(err_out, "rm '{}'", file)?;
        }

        if !args.dry_run {
            // Remove from index
            let index = repo.index_mut()?;
            index.remove(rel_path.as_ref(), Stage::Normal);

            // Remove from working tree unless --cached
            if !args.cached {
                let fs_path = work_tree.join(file);
                if fs_path.exists() {
                    if fs_path.is_dir() {
                        if args.r {
                            std::fs::remove_dir_all(&fs_path)?;
                        } else {
                            bail!("not removing '{}' recursively without -r", file);
                        }
                    } else {
                        std::fs::remove_file(&fs_path)?;
                    }
                }
            }
        }
    }

    if !args.dry_run {
        repo.write_index()?;
    }

    Ok(0)
}

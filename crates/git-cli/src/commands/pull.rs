use std::io::{self, Write};

use anyhow::Result;
use clap::Args;

use crate::Cli;
use super::{fetch, merge};

#[derive(Args)]
pub struct PullArgs {
    /// Rebase instead of merge
    #[arg(long)]
    rebase: bool,

    /// Don't rebase
    #[arg(long)]
    no_rebase: bool,

    /// Only allow fast-forward
    #[arg(long)]
    ff_only: bool,

    /// Be quiet
    #[arg(short, long)]
    quiet: bool,

    /// Remote to pull from
    remote: Option<String>,

    /// Branch to pull
    branch: Option<String>,
}

pub fn run(args: &PullArgs, cli: &Cli) -> Result<i32> {
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Step 1: Fetch
    let remote = args.remote.as_deref().unwrap_or("origin");
    let fetch_args = fetch::FetchArgs {
        all: false,
        prune: false,
        depth: None,
        tags: false,
        quiet: args.quiet,
        remote: Some(remote.to_string()),
        refspec: if let Some(ref branch) = args.branch {
            vec![format!("refs/heads/{}:refs/remotes/{}/{}", branch, remote, branch)]
        } else {
            vec![]
        },
    };

    let fetch_result = fetch::run(&fetch_args, cli)?;
    if fetch_result != 0 {
        return Ok(fetch_result);
    }

    // Step 2: Determine upstream branch to merge
    let repo = super::open_repo(cli)?;
    let current = repo.current_branch()?
        .ok_or_else(|| anyhow::anyhow!("You are not currently on a branch."))?;
    drop(repo);

    let merge_branch = if let Some(ref branch) = args.branch {
        format!("{}/{}", remote, branch)
    } else {
        format!("{}/{}", remote, current)
    };

    // Step 3: Merge (or rebase)
    if args.rebase {
        // TODO: rebase support
        if !args.quiet {
            writeln!(err, "pull --rebase is not yet fully implemented, falling back to merge")?;
        }
    }

    let merge_args = merge::MergeArgs {
        no_ff: false,
        ff_only: args.ff_only,
        squash: false,
        abort: false,
        cont: false,
        no_commit: false,
        no_edit: false,
        message: None,
        commit: Some(merge_branch),
    };

    merge::run(&merge_args, cli)
}

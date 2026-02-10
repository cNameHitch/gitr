use std::io::{self, Write};

use anyhow::Result;
use clap::Args;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct MergeBaseArgs {
    /// Output all common ancestors
    #[arg(long)]
    all: bool,

    /// Find best common ancestor for octopus merge
    #[arg(long)]
    octopus: bool,

    /// Check if first commit is ancestor of second (exit 0=yes, 1=no)
    #[arg(long)]
    is_ancestor: bool,

    /// Find fork point using reflog
    #[arg(long)]
    fork_point: bool,

    /// Commits to find common ancestor of
    commits: Vec<String>,
}

pub fn run(args: &MergeBaseArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.commits.len() < 2 && !args.fork_point {
        anyhow::bail!("merge-base requires at least two commits");
    }

    // Resolve all commit arguments
    let mut oids = Vec::new();
    for rev in &args.commits {
        let oid = git_revwalk::resolve_revision(&repo, rev)?;
        oids.push(oid);
    }

    if args.is_ancestor {
        if oids.len() != 2 {
            anyhow::bail!("--is-ancestor requires exactly two commits");
        }
        let result = git_revwalk::is_ancestor(&repo, &oids[0], &oids[1])?;
        return Ok(if result { 0 } else { 1 });
    }

    if args.fork_point {
        if oids.len() != 2 {
            anyhow::bail!("--fork-point requires exactly two commits");
        }
        match git_revwalk::fork_point(&repo, &oids[0], &oids[1])? {
            Some(base) => {
                writeln!(out, "{}", base.to_hex())?;
                Ok(0)
            }
            None => {
                // No fork point found
                Ok(1)
            }
        }
    } else if args.octopus {
        match git_revwalk::merge_base_octopus(&repo, &oids)? {
            Some(base) => {
                writeln!(out, "{}", base.to_hex())?;
                Ok(0)
            }
            None => {
                // No common ancestor found
                Ok(1)
            }
        }
    } else if args.all {
        let bases = git_revwalk::merge_base(&repo, &oids[0], &oids[1])?;
        if bases.is_empty() {
            return Ok(1);
        }
        for base in &bases {
            writeln!(out, "{}", base.to_hex())?;
        }
        Ok(0)
    } else {
        // Default: output the single best merge base
        match git_revwalk::merge_base_one(&repo, &oids[0], &oids[1])? {
            Some(base) => {
                writeln!(out, "{}", base.to_hex())?;
                Ok(0)
            }
            None => {
                // No common ancestor found
                Ok(1)
            }
        }
    }
}

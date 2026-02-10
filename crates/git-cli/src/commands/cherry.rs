use std::io::{self, Write};

use anyhow::Result;
use clap::Args;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct CherryArgs {
    /// Show commit subjects alongside SHA1s
    #[arg(short = 'v')]
    verbose: bool,

    /// Upstream branch to compare against
    upstream: Option<String>,

    /// Head branch (defaults to HEAD)
    head: Option<String>,

    /// Limit commits (restrict cherry to descendants of this commit)
    limit: Option<String>,
}

pub fn run(args: &CherryArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Resolve upstream: default to the upstream tracking branch or error
    let upstream_oid = if let Some(ref rev) = args.upstream {
        git_revwalk::resolve_revision(&repo, rev)?
    } else {
        // Try to find upstream of current branch
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("no upstream configured and HEAD is unborn"))?
    };

    // Resolve head: default to HEAD
    let head_oid = if let Some(ref rev) = args.head {
        git_revwalk::resolve_revision(&repo, rev)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD does not point to a valid object"))?
    };

    // Resolve limit if provided
    let limit_oid = if let Some(ref rev) = args.limit {
        Some(git_revwalk::resolve_revision(&repo, rev)?)
    } else {
        None
    };

    let entries = git_revwalk::cherry(
        &repo,
        &upstream_oid,
        &head_oid,
        limit_oid.as_ref(),
    )?;

    for entry in &entries {
        let hex = entry.oid.to_hex();
        let short_oid = &hex[..7.min(hex.len())];
        if args.verbose {
            writeln!(out, "{} {} {}", entry.marker, short_oid, entry.subject)?;
        } else {
            writeln!(out, "{} {}", entry.marker, short_oid)?;
        }
    }

    Ok(0)
}

use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_revwalk::{RevWalk, WalkOptions};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct RevListArgs {
    /// Limit the number of commits to output
    #[arg(short = 'n', long = "max-count")]
    max_count: Option<usize>,

    /// Show all refs
    #[arg(long)]
    all: bool,

    /// Reverse the output
    #[arg(long)]
    reverse: bool,

    /// Count commits and print a number
    #[arg(long)]
    count: bool,

    /// Show commit objects only (no trees/blobs)
    #[arg(long)]
    objects: bool,

    /// Print only the first parent
    #[arg(long)]
    first_parent: bool,

    /// Show commits more recent than a specific date
    #[arg(long)]
    since: Option<String>,

    /// Show commits older than a specific date
    #[arg(long)]
    until: Option<String>,

    /// Limit commits to author matching pattern
    #[arg(long)]
    author: Option<String>,

    /// Limit commits to message matching pattern
    #[arg(long)]
    grep: Option<String>,

    /// Revisions and ranges
    revisions: Vec<String>,
}

pub fn run(args: &RevListArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut walk_opts = WalkOptions {
        max_count: args.max_count,
        first_parent_only: args.first_parent,
        author_pattern: args.author.clone(),
        grep_pattern: args.grep.clone(),
        ..WalkOptions::default()
    };

    if args.reverse {
        walk_opts.sort = git_revwalk::SortOrder::Reverse;
    }

    if let Some(ref since_str) = args.since {
        walk_opts.since = parse_date(since_str);
    }
    if let Some(ref until_str) = args.until {
        walk_opts.until = parse_date(until_str);
    }

    let mut walker = RevWalk::new(&repo)?;
    walker.set_options(walk_opts);

    if args.all {
        walker.push_all()?;
    }

    if args.revisions.is_empty() && !args.all {
        walker.push_head()?;
    } else {
        for rev in &args.revisions {
            if rev.contains("..") {
                walker.push_range(rev)?;
            } else if let Some(stripped) = rev.strip_prefix('^') {
                let oid = git_revwalk::resolve_revision(&repo, stripped)?;
                walker.hide(oid)?;
            } else {
                let oid = git_revwalk::resolve_revision(&repo, rev)?;
                walker.push(oid)?;
            }
        }
    }

    if args.count {
        let count = walker.count();
        writeln!(out, "{}", count)?;
        return Ok(0);
    }

    if args.objects {
        // List all reachable objects
        let mut include = Vec::new();
        let mut exclude = Vec::new();
        for rev in &args.revisions {
            if let Some(stripped) = rev.strip_prefix('^') {
                let oid = git_revwalk::resolve_revision(&repo, stripped)?;
                exclude.push(oid);
            } else if rev.contains("..") {
                // Already handled via range
            } else {
                let oid = git_revwalk::resolve_revision(&repo, rev)?;
                include.push(oid);
            }
        }
        if include.is_empty() {
            if let Some(oid) = repo.head_oid()? {
                include.push(oid);
            }
        }
        let objects = git_revwalk::list_objects(&repo, &include, &exclude, None)?;
        for oid in objects {
            writeln!(out, "{}", oid.to_hex())?;
        }
    } else {
        for oid_result in walker {
            let oid = oid_result?;
            writeln!(out, "{}", oid.to_hex())?;
        }
    }

    Ok(0)
}

fn parse_date(s: &str) -> Option<i64> {
    if let Ok(ts) = s.parse::<i64>() {
        return Some(ts);
    }
    None
}

use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_object::Object;
use git_revwalk::RevWalk;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct ShortlogArgs {
    /// Suppress commit description, only provide commit count
    #[arg(short = 's', long)]
    summary: bool,

    /// Sort output by number of commits per author
    #[arg(short = 'n', long)]
    numbered: bool,

    /// Show author email address
    #[arg(short = 'e', long)]
    email: bool,

    /// Show all refs
    #[arg(long)]
    all: bool,

    /// Revisions
    revisions: Vec<String>,
}

pub fn run(args: &ShortlogArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut walker = RevWalk::new(&repo)?;

    if args.all {
        walker.push_all()?;
    } else if args.revisions.is_empty() {
        walker.push_head()?;
    } else {
        for rev in &args.revisions {
            if rev.contains("..") {
                walker.push_range(rev)?;
            } else {
                let oid = git_revwalk::resolve_revision(&repo, rev)?;
                walker.push(oid)?;
            }
        }
    }

    // Group commits by author
    let mut authors: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for oid_result in walker {
        let oid = oid_result?;
        let obj = repo.odb().read(&oid)?;
        if let Some(Object::Commit(commit)) = obj {
            let author_name = String::from_utf8_lossy(&commit.author.name).to_string();
            let author_email = String::from_utf8_lossy(&commit.author.email).to_string();

            let key = if args.email {
                format!("{} <{}>", author_name, author_email)
            } else {
                author_name
            };

            let summary = String::from_utf8_lossy(commit.summary()).to_string();
            authors.entry(key).or_default().push(summary);
        }
    }

    // Sort by count if requested
    let mut entries: Vec<(String, Vec<String>)> = authors.into_iter().collect();
    if args.numbered {
        entries.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    }

    for (author, subjects) in &entries {
        if args.summary {
            writeln!(out, "{:>6}\t{}", subjects.len(), author)?;
        } else {
            writeln!(out, "{} ({}):", author, subjects.len())?;
            for subject in subjects {
                writeln!(out, "      {}", subject)?;
            }
            writeln!(out)?;
        }
    }

    Ok(0)
}

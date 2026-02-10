use std::collections::BTreeMap;
use std::io::{self, BufRead, IsTerminal, Write};

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
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Group commits by author
    let mut authors: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let read_from_stdin = !args.all && args.revisions.is_empty() && !io::stdin().is_terminal();

    if read_from_stdin {
        // Read git log output from stdin
        // Parses the default `git log` format:
        //   commit <hash>
        //   Author: Name <email>
        //   Date:   ...
        //
        //       subject line
        //       ...
        let stdin = io::stdin();
        let reader = stdin.lock();

        let mut current_author: Option<String> = None;
        let mut in_body = false;
        let mut found_subject = false;

        for line in reader.lines() {
            let line = line?;
            if line.starts_with("commit ") {
                // New commit entry — reset state
                current_author = None;
                in_body = false;
                found_subject = false;
            } else if let Some(rest) = line.strip_prefix("Author: ") {
                // Parse "Name <email>" or just "Name"
                let rest = rest.trim();
                if args.email {
                    current_author = Some(rest.to_string());
                } else {
                    // Strip email portion: "Name <email>" -> "Name"
                    current_author = Some(
                        rest.find(" <")
                            .map(|i| rest[..i].to_string())
                            .unwrap_or_else(|| rest.to_string()),
                    );
                }
            } else if current_author.is_some() && !in_body && line.is_empty() {
                // Blank line after headers signals start of commit message body
                in_body = true;
            } else if in_body && !found_subject {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    // First non-empty line in the body is the subject
                    if let Some(ref author) = current_author {
                        authors
                            .entry(author.clone())
                            .or_default()
                            .push(trimmed.to_string());
                    }
                    found_subject = true;
                }
            }
        }
    } else {
        let repo = open_repo(cli)?;
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
    }

    // Sort by count if requested
    let mut entries: Vec<(String, Vec<String>)> = authors.into_iter().collect();
    if args.numbered {
        entries.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    }

    for (author, subjects) in &mut entries {
        // Reverse to show oldest-first (commits come from walker in newest-first order)
        subjects.reverse();
        if args.summary {
            writeln!(out, "{:>6}\t{}", subjects.len(), author)?;
        } else {
            writeln!(out, "{} ({}):", author, subjects.len())?;
            for subject in subjects.iter() {
                writeln!(out, "      {}", subject)?;
            }
            writeln!(out)?;
        }
    }

    Ok(0)
}

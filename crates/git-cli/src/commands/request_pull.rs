use std::io::{self, Write};

use anyhow::{bail, Result};
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_hash::ObjectId;
use git_object::Object;
use git_revwalk::RevWalk;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct RequestPullArgs {
    /// Include patch text in the output
    #[arg(short = 'p')]
    patch: bool,

    /// Start commit (the point where your changes begin)
    start: String,

    /// Public URL of the repository
    url: String,

    /// End commit (default: HEAD)
    end: Option<String>,
}

pub fn run(args: &RequestPullArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Resolve start and end commits
    let start_oid = git_revwalk::resolve_revision(&repo, &args.start)?;
    let end_oid = if let Some(ref end) = args.end {
        git_revwalk::resolve_revision(&repo, end)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD not found"))?
    };

    // Get end ref name for the request
    let end_ref = args.end.as_deref().unwrap_or("HEAD");

    // Get the end commit to extract summary info
    let end_commit = match repo.odb().read(&end_oid)? {
        Some(Object::Commit(c)) => c,
        _ => bail!("not a commit: {}", end_oid.to_hex()),
    };

    // Determine the branch name
    let branch_name = if end_ref == "HEAD" {
        repo.current_branch()?
            .unwrap_or_else(|| end_oid.to_hex().to_string())
    } else {
        end_ref.to_string()
    };

    // Collect commits between start and end
    let commits = collect_commits_between(&repo, &start_oid, &end_oid)?;

    if commits.is_empty() {
        bail!(
            "warn: no commits between {} and {}",
            args.start,
            end_ref
        );
    }

    // Generate the pull request message

    // Header
    writeln!(
        out,
        "The following changes since commit {}:",
        start_oid.to_hex()
    )?;
    writeln!(out)?;

    // Show start commit summary
    if let Some(Object::Commit(start_commit)) = repo.odb().read(&start_oid)? {
        let summary = first_line(start_commit.message.as_ref());
        writeln!(out, "  {} ({})", summary, start_oid.to_hex())?;
    }

    writeln!(out)?;
    writeln!(out, "are available in the Git repository at:")?;
    writeln!(out)?;
    writeln!(out, "  {} {}", args.url, branch_name)?;
    writeln!(out)?;
    writeln!(
        out,
        "for you to fetch changes up to {}:",
        end_oid.to_hex()
    )?;
    writeln!(out)?;

    // Show end commit summary
    let end_summary = first_line(end_commit.message.as_ref());
    writeln!(out, "  {} ({})", end_summary, end_oid.to_hex())?;
    writeln!(out)?;

    // Separator
    writeln!(
        out,
        "----------------------------------------------------------------"
    )?;

    // Show shortlog of commits
    let mut author_commits: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    for commit_oid in &commits {
        if let Some(Object::Commit(commit)) = repo.odb().read(commit_oid)? {
            let author = String::from_utf8_lossy(&commit.author.name).to_string();
            let summary = first_line(commit.message.as_ref());
            author_commits.entry(author).or_default().push(summary);
        }
    }

    for (author, summaries) in &author_commits {
        writeln!(out, "{} ({}):", author, summaries.len())?;
        for summary in summaries {
            writeln!(out, "      {}", summary)?;
        }
        writeln!(out)?;
    }

    // Get tree OIDs for diffstat
    let start_tree = get_commit_tree(&repo, &start_oid)?;
    let end_tree = get_commit_tree(&repo, &end_oid)?;

    // Show diffstat
    let stat_opts = DiffOptions {
        context_lines: 3,
        output_format: DiffOutputFormat::Stat,
        ..Default::default()
    };
    let diff_result =
        git_diff::tree::diff_trees(repo.odb(), Some(&start_tree), Some(&end_tree), &stat_opts)?;
    let stat_output = format_diff(&diff_result, &stat_opts);
    if !stat_output.is_empty() {
        out.write_all(stat_output.as_bytes())?;
        if !stat_output.ends_with('\n') {
            writeln!(out)?;
        }
    }

    // Optionally show the full patch
    if args.patch {
        writeln!(out)?;

        let patch_opts = DiffOptions {
            context_lines: 3,
            output_format: DiffOutputFormat::Unified,
            ..Default::default()
        };
        let patch_result = git_diff::tree::diff_trees(
            repo.odb(),
            Some(&start_tree),
            Some(&end_tree),
            &patch_opts,
        )?;
        let patch_output = format_diff(&patch_result, &patch_opts);
        if !patch_output.is_empty() {
            out.write_all(patch_output.as_bytes())?;
            if !patch_output.ends_with('\n') {
                writeln!(out)?;
            }
        }
    }

    Ok(0)
}

/// Extract the first line of a commit message.
fn first_line(message: &[u8]) -> String {
    let s = String::from_utf8_lossy(message);
    s.lines().next().unwrap_or("").trim().to_string()
}

/// Collect commit OIDs between start (exclusive) and end (inclusive).
fn collect_commits_between(
    repo: &git_repository::Repository,
    start: &ObjectId,
    end: &ObjectId,
) -> Result<Vec<ObjectId>> {
    let mut walk = RevWalk::new(repo)?;
    walk.push(*end)?;
    walk.hide(*start)?;

    let mut commits = Vec::new();
    for result in &mut walk {
        let oid = result?;
        commits.push(oid);
    }

    // RevWalk returns in reverse chronological order, reverse for chronological
    commits.reverse();
    Ok(commits)
}

/// Get the tree OID from a commit.
fn get_commit_tree(
    repo: &git_repository::Repository,
    commit_oid: &ObjectId,
) -> Result<ObjectId> {
    match repo.odb().read(commit_oid)? {
        Some(Object::Commit(commit)) => Ok(commit.tree),
        _ => bail!("not a commit: {}", commit_oid.to_hex()),
    }
}

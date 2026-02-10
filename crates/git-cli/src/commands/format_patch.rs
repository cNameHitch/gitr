use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::format::nameonly::format_summary;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_hash::ObjectId;
use git_object::{Commit, Object};
use git_revwalk::RevWalk;
use git_utils::date::DateFormat;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct FormatPatchArgs {
    /// Output directory
    #[arg(short = 'o', long)]
    output_directory: Option<PathBuf>,

    /// Generate a cover letter
    #[arg(long)]
    cover_letter: bool,

    /// Number patches
    #[arg(short = 'n', long)]
    numbered: bool,

    /// Add threading headers
    #[arg(long)]
    thread: bool,

    /// Subject prefix (default: PATCH)
    #[arg(long, default_value = "PATCH")]
    subject_prefix: String,

    /// Start numbering from <n>
    #[arg(long, default_value = "1")]
    start_number: usize,

    /// Maximum number of commits to format (supports -<n> syntax)
    #[arg(long)]
    max_count: Option<usize>,

    /// Output patches to stdout instead of files
    #[arg(long)]
    stdout: bool,

    /// Add Signed-off-by trailer
    #[arg(short = 's', long)]
    signoff: bool,

    /// Suppress patch numbering
    #[arg(short = 'N', long)]
    no_numbered: bool,

    /// Keep subject (don't strip Re: or [PATCH])
    #[arg(short = 'k')]
    keep_subject: bool,

    /// Add To: header
    #[arg(long)]
    to: Vec<String>,

    /// Add Cc: header
    #[arg(long)]
    cc: Vec<String>,

    /// Use <ident> in From: header
    #[arg(long, value_name = "ident")]
    from: Option<String>,

    /// Set In-Reply-To header
    #[arg(long, value_name = "message-id")]
    in_reply_to: Option<String>,

    /// Set the base commit for the patch series
    #[arg(long, value_name = "commit")]
    base: Option<String>,

    /// Mark the series as the <n>th iteration
    #[arg(short = 'v', long, value_name = "n")]
    reroll_count: Option<u32>,

    /// Include a range-diff in the cover letter
    #[arg(long, value_name = "refspec")]
    range_diff: Option<String>,

    /// Revision range
    revision: String,
}

pub fn run(args: &FormatPatchArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Collect commits
    let commits = collect_commits(&repo, &args.revision, args.max_count)?;
    let total = commits.len();

    if total == 0 {
        return Ok(0);
    }

    if args.stdout {
        // Output all patches to stdout
        for (i, (oid, commit)) in commits.iter().rev().enumerate() {
            let patch_num = args.start_number + i;
            let subject = String::from_utf8_lossy(commit.summary());

            writeln!(out, "From {} Mon Sep 17 00:00:00 2001", oid.to_hex())?;
            writeln!(out, "From: {} <{}>",
                String::from_utf8_lossy(&commit.author.name),
                String::from_utf8_lossy(&commit.author.email))?;
            writeln!(out, "Date: {}", commit.author.date.format(&DateFormat::Rfc2822))?;

            if args.numbered || total > 1 {
                writeln!(out, "Subject: [{} {}/{}] {}", args.subject_prefix, patch_num, total, subject)?;
            } else {
                writeln!(out, "Subject: [{}] {}", args.subject_prefix, subject)?;
            }

            writeln!(out)?;

            if let Some(body) = commit.body() {
                let body_str = String::from_utf8_lossy(body);
                write!(out, "{}", body_str)?;
                writeln!(out)?;
            }

            writeln!(out, "---")?;

            let parent_tree = if let Some(parent_oid) = commit.first_parent() {
                match repo.odb().read(parent_oid)? {
                    Some(Object::Commit(pc)) => Some(pc.tree),
                    _ => None,
                }
            } else {
                None
            };

            let mut diff_opts = DiffOptions {
                output_format: DiffOutputFormat::Stat,
                ..DiffOptions::default()
            };

            let stat_result = git_diff::tree::diff_trees(
                repo.odb(), parent_tree.as_ref(), Some(&commit.tree), &diff_opts)?;
            if !stat_result.is_empty() {
                let stat_output = format_diff(&stat_result, &diff_opts);
                write!(out, "{}", stat_output)?;
                let summary = format_summary(&stat_result);
                if !summary.is_empty() {
                    write!(out, "{}", summary)?;
                }
            }

            writeln!(out)?;

            diff_opts.output_format = DiffOutputFormat::Unified;
            let diff_result = git_diff::tree::diff_trees(
                repo.odb(), parent_tree.as_ref(), Some(&commit.tree), &diff_opts)?;
            if !diff_result.is_empty() {
                let diff_output = format_diff(&diff_result, &diff_opts);
                write!(out, "{}", diff_output)?;
            }

            writeln!(out, "-- ")?;
            writeln!(out, "{}", git_version_string())?;
            writeln!(out)?;
        }

        return Ok(0);
    }

    // Determine output directory
    let output_dir = if let Some(ref dir) = args.output_directory {
        fs::create_dir_all(dir)?;
        dir.clone()
    } else {
        PathBuf::from(".")
    };

    // Generate cover letter if requested
    if args.cover_letter {
        let filename = output_dir.join("0000-cover-letter.patch");
        let mut file = fs::File::create(&filename)?;

        writeln!(file, "Subject: [{}] *** SUBJECT HERE ***", args.subject_prefix)?;
        writeln!(file)?;
        writeln!(file, "*** BLURB HERE ***")?;
        writeln!(file)?;
        writeln!(file, "---")?;

        writeln!(out, "{}", filename.display())?;
    }

    // Generate patches (in chronological order)
    for (i, (oid, commit)) in commits.iter().rev().enumerate() {
        let patch_num = args.start_number + i;
        let subject = String::from_utf8_lossy(commit.summary());

        // Build filename
        let sanitized_subject: String = subject
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect();
        let filename = output_dir.join(format!(
            "{:04}-{}.patch",
            patch_num,
            truncate_str(&sanitized_subject, 52)
        ));

        let mut file = fs::File::create(&filename)?;

        // Write email headers
        writeln!(file, "From {} Mon Sep 17 00:00:00 2001", oid.to_hex())?;
        writeln!(file, "From: {} <{}>",
            String::from_utf8_lossy(&commit.author.name),
            String::from_utf8_lossy(&commit.author.email))?;
        writeln!(file, "Date: {}", commit.author.date.format(&DateFormat::Rfc2822))?;

        // Subject line
        if args.numbered || total > 1 {
            writeln!(file, "Subject: [{} {}/{}] {}", args.subject_prefix, patch_num, total, subject)?;
        } else {
            writeln!(file, "Subject: [{}] {}", args.subject_prefix, subject)?;
        }

        if args.thread {
            writeln!(file, "Message-Id: <{}.{}.git-gitr@localhost>", oid.to_hex(), patch_num)?;
        }

        writeln!(file)?;

        // Body
        if let Some(body) = commit.body() {
            let body_str = String::from_utf8_lossy(body);
            write!(file, "{}", body_str)?;
            writeln!(file)?;
        }

        writeln!(file, "---")?;

        // Diff
        let parent_tree = if let Some(parent_oid) = commit.first_parent() {
            match repo.odb().read(parent_oid)? {
                Some(Object::Commit(pc)) => Some(pc.tree),
                _ => None,
            }
        } else {
            None
        };

        let mut diff_opts = DiffOptions {
            output_format: DiffOutputFormat::Stat,
            ..DiffOptions::default()
        };

        let stat_result = git_diff::tree::diff_trees(
            repo.odb(), parent_tree.as_ref(), Some(&commit.tree), &diff_opts)?;
        if !stat_result.is_empty() {
            let stat_output = format_diff(&stat_result, &diff_opts);
            write!(file, "{}", stat_output)?;
            let summary = format_summary(&stat_result);
            if !summary.is_empty() {
                write!(file, "{}", summary)?;
            }
        }

        writeln!(file)?;

        diff_opts.output_format = DiffOutputFormat::Unified;
        let diff_result = git_diff::tree::diff_trees(
            repo.odb(), parent_tree.as_ref(), Some(&commit.tree), &diff_opts)?;
        if !diff_result.is_empty() {
            let diff_output = format_diff(&diff_result, &diff_opts);
            write!(file, "{}", diff_output)?;
        }

        writeln!(file, "-- ")?;
        writeln!(file, "{}", git_version_string())?;

        writeln!(out, "{}", filename.display())?;
    }

    Ok(0)
}

fn collect_commits(
    repo: &git_repository::Repository,
    range: &str,
    max_count: Option<usize>,
) -> Result<Vec<(ObjectId, Commit)>> {
    let mut walker = RevWalk::new(repo)?;

    if range.contains("..") {
        walker.push_range(range)?;
        if let Some(n) = max_count {
            let walk_opts = git_revwalk::WalkOptions {
                max_count: Some(n),
                ..Default::default()
            };
            walker.set_options(walk_opts);
        }
    } else if max_count.is_some() {
        // -<n> <revision>: walk backwards from revision with max_count
        let oid = git_revwalk::resolve_revision(repo, range)?;
        walker.push(oid)?;
        let walk_opts = git_revwalk::WalkOptions {
            max_count,
            ..Default::default()
        };
        walker.set_options(walk_opts);
    } else {
        // Single revision without max_count means "since that revision" (revision..HEAD)
        let implicit_range = format!("{}..HEAD", range);
        walker.push_range(&implicit_range)?;
    }

    let mut commits = Vec::new();
    for oid_result in walker {
        let oid = oid_result?;
        let obj = repo.odb().read(&oid)?;
        if let Some(Object::Commit(c)) = obj {
            commits.push((oid, c));
        }
    }

    Ok(commits)
}

/// Return git version string for format-patch trailer.
/// Uses the gitr version; tests should normalize this field.
fn git_version_string() -> String {
    format!("gitr {}", env!("CARGO_PKG_VERSION"))
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_object::{Commit, Object};
use git_revwalk::{
    format_builtin_with_decorations, format_commit_with_decorations, BuiltinFormat, FormatOptions,
    RevWalk, SortOrder, WalkOptions,
};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct WhatchangedArgs {
    /// Show only the first <n> commits
    #[arg(short = 'n', long = "max-count")]
    max_count: Option<usize>,

    /// Show commits more recent than a specific date
    #[arg(long)]
    since: Option<String>,

    /// Show commits older than a specific date
    #[arg(long)]
    until: Option<String>,

    /// Limit commits to author matching pattern
    #[arg(long)]
    author: Option<String>,

    /// Show format (oneline, short, medium, full, fuller, email, raw)
    #[arg(long)]
    format: Option<String>,

    /// Pretty-print format (alias for --format)
    #[arg(long)]
    pretty: Option<String>,

    /// Generate patch output instead of raw diff
    #[arg(short = 'p', long)]
    patch: bool,

    /// Show only names of changed files
    #[arg(long)]
    name_only: bool,

    /// Show names and status of changed files
    #[arg(long)]
    name_status: bool,

    /// Follow only the first parent
    #[arg(long)]
    first_parent: bool,

    /// Reverse the output order
    #[arg(long)]
    reverse: bool,

    /// Revision range or starting point
    #[arg(value_name = "revision")]
    revisions: Vec<String>,

    /// Limit to commits touching these paths (after --)
    #[arg(last = true, value_name = "path")]
    path_args: Vec<String>,
}

pub fn run(args: &WhatchangedArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Parse format
    let (builtin, custom_format) = parse_format(args);
    let format_options = FormatOptions::default();

    // Build walk options
    let mut walk_opts = WalkOptions {
        max_count: args.max_count,
        author_pattern: args.author.clone(),
        first_parent_only: args.first_parent,
        ..WalkOptions::default()
    };
    if args.reverse {
        walk_opts.sort = SortOrder::Reverse;
    }

    if let Some(ref since_str) = args.since {
        walk_opts.since = parse_date(since_str);
    }
    if let Some(ref until_str) = args.until {
        walk_opts.until = parse_date(until_str);
    }

    let mut walker = RevWalk::new(&repo)?;
    walker.set_options(walk_opts);

    // Parse pathspec: use path_args (after --) plus any remaining from revisions after --
    let mut revs = Vec::new();
    let mut pathspecs: Vec<String> = args.path_args.clone();
    let mut saw_separator = false;
    for arg in &args.revisions {
        if arg == "--" {
            saw_separator = true;
            continue;
        }
        if saw_separator {
            pathspecs.push(arg.clone());
        } else {
            revs.push(arg.clone());
        }
    }

    if revs.is_empty() {
        walker.push_head()?;
    } else {
        for rev in &revs {
            if rev.contains("..") {
                walker.push_range(rev)?;
            } else {
                let oid = git_revwalk::resolve_revision(&repo, rev)?;
                walker.push(oid)?;
            }
        }
    }

    // Determine diff output format: default is Raw for whatchanged
    let diff_format = if args.patch {
        DiffOutputFormat::Unified
    } else if args.name_only {
        DiffOutputFormat::NameOnly
    } else if args.name_status {
        DiffOutputFormat::NameStatus
    } else {
        DiffOutputFormat::Raw
    };

    let mut first_commit = true;
    for oid_result in walker {
        let oid = oid_result?;

        let obj = repo
            .odb()
            .read(&oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;

        let commit = match obj {
            Object::Commit(c) => c,
            _ => continue,
        };

        // Filter by pathspec
        if !pathspecs.is_empty() && !commit_touches_paths(&repo, &commit, &pathspecs)? {
            continue;
        }

        // Separator between commits
        let needs_separator = custom_format.is_none() && builtin != BuiltinFormat::Oneline;
        if needs_separator && !first_commit {
            writeln!(out)?;
        }
        first_commit = false;

        // Format commit header
        let formatted = if let Some(ref fmt) = custom_format {
            format_commit_with_decorations(&commit, &oid, fmt, &format_options, None)
        } else {
            format_builtin_with_decorations(&commit, &oid, builtin, &format_options, None)
        };

        write!(out, "{}", formatted)?;
        if custom_format.is_some() || builtin == BuiltinFormat::Oneline {
            writeln!(out)?;
        }

        // Show diff for this commit
        show_commit_diff(&repo, &commit, diff_format, &mut out)?;
    }

    Ok(0)
}

fn show_commit_diff(
    repo: &git_repository::Repository,
    commit: &Commit,
    format: DiffOutputFormat,
    out: &mut impl Write,
) -> Result<()> {
    let parent_tree = if let Some(parent_oid) = commit.first_parent() {
        let parent_obj = repo.odb().read(parent_oid)?;
        match parent_obj {
            Some(Object::Commit(pc)) => Some(pc.tree),
            _ => None,
        }
    } else {
        None
    };

    let diff_opts = DiffOptions {
        output_format: format,
        ..DiffOptions::default()
    };

    let result = git_diff::tree::diff_trees(
        repo.odb(),
        parent_tree.as_ref(),
        Some(&commit.tree),
        &diff_opts,
    )?;

    if !result.is_empty() {
        let output = format_diff(&result, &diff_opts);
        write!(out, "\n{}", output)?;
    }

    Ok(())
}

fn commit_touches_paths(
    repo: &git_repository::Repository,
    commit: &Commit,
    pathspecs: &[String],
) -> Result<bool> {
    let parent_tree = commit.first_parent().and_then(|p| {
        repo.odb().read(p).ok().flatten().and_then(|o| match o {
            Object::Commit(c) => Some(c.tree),
            _ => None,
        })
    });
    let diff_opts = DiffOptions::default();
    if let Ok(result) = git_diff::tree::diff_trees(
        repo.odb(),
        parent_tree.as_ref(),
        Some(&commit.tree),
        &diff_opts,
    ) {
        let touches = result.files.iter().any(|f| {
            let path = f
                .new_path
                .as_ref()
                .or(f.old_path.as_ref())
                .map(|p| p.to_str_lossy())
                .unwrap_or_default();
            pathspecs
                .iter()
                .any(|ps| path.starts_with(ps.as_str()))
        });
        Ok(touches)
    } else {
        Ok(true)
    }
}

fn parse_format(args: &WhatchangedArgs) -> (BuiltinFormat, Option<String>) {
    let fmt_str = args.format.as_deref().or(args.pretty.as_deref());

    match fmt_str {
        Some("oneline") => (BuiltinFormat::Oneline, None),
        Some("short") => (BuiltinFormat::Short, None),
        Some("medium") => (BuiltinFormat::Medium, None),
        Some("full") => (BuiltinFormat::Full, None),
        Some("fuller") => (BuiltinFormat::Fuller, None),
        Some("email") => (BuiltinFormat::Email, None),
        Some("raw") => (BuiltinFormat::Raw, None),
        Some(custom) => {
            let fmt = if let Some(stripped) = custom.strip_prefix("format:") {
                stripped
            } else if let Some(stripped) = custom.strip_prefix("tformat:") {
                stripped
            } else {
                custom
            };
            (BuiltinFormat::Medium, Some(fmt.to_string()))
        }
        None => (BuiltinFormat::Medium, None),
    }
}

fn parse_date(s: &str) -> Option<i64> {
    if let Ok(ts) = s.parse::<i64>() {
        return Some(ts);
    }
    None
}

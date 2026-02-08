use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_object::{Commit, Object};
use git_revwalk::{
    format_builtin, format_commit, BuiltinFormat, FormatOptions, GraphDrawer, RevWalk,
    SortOrder, WalkOptions,
};
use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct LogArgs {
    /// Show only the first <n> commits
    #[arg(short = 'n', long = "max-count")]
    max_count: Option<usize>,

    /// Skip <n> commits before starting to show
    #[arg(long)]
    skip: Option<usize>,

    /// Show commits more recent than a specific date
    #[arg(long)]
    since: Option<String>,

    /// Show commits older than a specific date
    #[arg(long)]
    until: Option<String>,

    /// Limit commits to author matching pattern
    #[arg(long)]
    author: Option<String>,

    /// Limit commits to those with log message matching pattern
    #[arg(long)]
    grep: Option<String>,

    /// Show one-line summary per commit
    #[arg(long)]
    oneline: bool,

    /// Show format (oneline, short, medium, full, fuller, email, raw)
    #[arg(long)]
    format: Option<String>,

    /// Pretty-print format (alias for --format)
    #[arg(long)]
    pretty: Option<String>,

    /// Draw ASCII graph of branch structure
    #[arg(long)]
    graph: bool,

    /// Show diffstat for each commit
    #[arg(long)]
    stat: bool,

    /// Show patch (diff) for each commit
    #[arg(short = 'p', long)]
    patch: bool,

    /// Show all refs
    #[arg(long)]
    all: bool,

    /// Reverse the output order
    #[arg(long)]
    reverse: bool,

    /// Follow only the first parent
    #[arg(long)]
    first_parent: bool,

    /// Show name-only diff
    #[arg(long)]
    name_only: bool,

    /// Show name-status diff
    #[arg(long)]
    name_status: bool,

    /// Revision range or starting point
    #[arg(value_name = "revision")]
    revisions: Vec<String>,
}

pub fn run(args: &LogArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Parse format
    let (builtin, custom_format) = parse_format(args);
    let format_options = FormatOptions::default();

    // Build walk options
    let mut walk_opts = WalkOptions {
        max_count: args.max_count,
        skip: args.skip,
        author_pattern: args.author.clone(),
        grep_pattern: args.grep.clone(),
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

    // Set up revision walker
    let mut walker = RevWalk::new(&repo)?;
    walker.set_options(walk_opts);

    // Parse pathspec from revisions (anything after --)
    let mut revs = Vec::new();
    let mut _pathspecs: Vec<String> = Vec::new();
    let mut saw_separator = false;
    for arg in &args.revisions {
        if arg == "--" {
            saw_separator = true;
            continue;
        }
        if saw_separator {
            _pathspecs.push(arg.clone());
        } else {
            revs.push(arg.clone());
        }
    }

    if args.all {
        walker.push_all()?;
    }

    if revs.is_empty() && !args.all {
        // Default: start from HEAD
        // Check if the repo is unborn (no commits)
        if repo.is_unborn()? {
            let branch_name = match repo.current_branch() {
                Ok(Some(name)) => name,
                _ => "main".to_string(),
            };
            let stderr = io::stderr();
            let mut err = stderr.lock();
            writeln!(
                err,
                "fatal: your current branch '{}' does not have any commits yet",
                branch_name
            )?;
            return Ok(128);
        }
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

    // Graph drawer
    let mut graph_drawer = if args.graph {
        Some(GraphDrawer::new())
    } else {
        None
    };

    // Walk commits
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

        // Format the commit
        let formatted = if let Some(ref fmt) = custom_format {
            format_commit(&commit, &oid, fmt, &format_options)
        } else {
            format_builtin(&commit, &oid, builtin, &format_options)
        };

        // Add separator between commits for multi-line formats
        let needs_separator = custom_format.is_none() && builtin != BuiltinFormat::Oneline;
        if needs_separator && !first_commit {
            writeln!(out)?;
        }
        first_commit = false;

        // Output with optional graph
        if let Some(ref mut drawer) = graph_drawer {
            let graph_lines = drawer.draw_commit(&oid, &commit.parents);
            let commit_lines: Vec<&str> = formatted.lines().collect();

            for (i, graph_line) in graph_lines.iter().enumerate() {
                if i < commit_lines.len() {
                    writeln!(out, "{} {}", graph_line, commit_lines[i])?;
                } else {
                    writeln!(out, "{}", graph_line)?;
                }
            }
            // Any remaining commit lines
            for line in commit_lines.iter().skip(graph_lines.len()) {
                let pad = " ".repeat(graph_lines.first().map_or(0, |l| l.len()));
                writeln!(out, "{} {}", pad, line)?;
            }
        } else {
            write!(out, "{}", formatted)?;
            if custom_format.is_some() || builtin == BuiltinFormat::Oneline {
                writeln!(out)?;
            }
        }

        // Append stat/patch if requested
        if args.stat || args.patch || args.name_only || args.name_status {
            show_commit_diff(&repo, &commit, &oid, args, &mut out)?;
        }
    }

    Ok(0)
}

fn parse_format(args: &LogArgs) -> (BuiltinFormat, Option<String>) {
    if args.oneline {
        return (BuiltinFormat::Oneline, None);
    }

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
            // Custom format string (e.g., "format:%H %s")
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
    // Try parsing as unix timestamp
    if let Ok(ts) = s.parse::<i64>() {
        return Some(ts);
    }
    None
}

fn show_commit_diff(
    repo: &git_repository::Repository,
    commit: &Commit,
    _oid: &git_hash::ObjectId,
    args: &LogArgs,
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

    let mut diff_opts = DiffOptions::default();
    if args.stat {
        diff_opts.output_format = DiffOutputFormat::Stat;
    } else if args.name_only {
        diff_opts.output_format = DiffOutputFormat::NameOnly;
    } else if args.name_status {
        diff_opts.output_format = DiffOutputFormat::NameStatus;
    } else {
        diff_opts.output_format = DiffOutputFormat::Unified;
    }

    let result = git_diff::tree::diff_trees(
        repo.odb(),
        parent_tree.as_ref(),
        Some(&commit.tree),
        &diff_opts,
    )?;

    if !result.is_empty() {
        let output = format_diff(&result, &diff_opts);
        write!(out, "{}", output)?;
    }

    // If both stat and patch were requested, show stat then patch
    if args.stat && args.patch {
        diff_opts.output_format = DiffOutputFormat::Unified;
        let result = git_diff::tree::diff_trees(
            repo.odb(),
            parent_tree.as_ref(),
            Some(&commit.tree),
            &diff_opts,
        )?;
        if !result.is_empty() {
            let output = format_diff(&result, &diff_opts);
            write!(out, "{}", output)?;
        }
    }

    Ok(())
}

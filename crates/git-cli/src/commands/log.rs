use std::collections::HashMap;
use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_hash::ObjectId;
use git_object::{Commit, Object};
use git_ref::RefStore;
use git_revwalk::{
    format_builtin_with_decorations, format_commit_with_decorations, BuiltinFormat, FormatOptions,
    GraphDrawer, RevWalk, SortOrder, WalkOptions,
};
use git_utils::date::DateFormat;
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

    /// Format date output (iso, relative, short, default, format:<strftime>)
    #[arg(long)]
    date: Option<String>,

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

    /// Decorate commits with ref names
    #[arg(long)]
    decorate: bool,

    /// Show all refs
    #[arg(long)]
    all: bool,

    /// Reverse the output order
    #[arg(long)]
    reverse: bool,

    /// Follow only the first parent
    #[arg(long)]
    first_parent: bool,

    /// Show only merge commits (commits with more than one parent)
    #[arg(long)]
    merges: bool,

    /// Skip merge commits (commits with more than one parent)
    #[arg(long)]
    no_merges: bool,

    /// Show name-only diff
    #[arg(long)]
    name_only: bool,

    /// Show name-status diff
    #[arg(long)]
    name_status: bool,

    /// Revision range or starting point
    #[arg(value_name = "revision")]
    revisions: Vec<String>,

    /// Limit to commits touching these paths (after --)
    #[arg(last = true, value_name = "path")]
    path_args: Vec<String>,
}

pub fn run(args: &LogArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Parse format
    let (builtin, custom_format) = parse_format(args);
    let mut format_options = FormatOptions::default();
    // --oneline is shorthand for --format=oneline --abbrev-commit
    if args.oneline {
        format_options.abbrev_len = 7;
    }
    // Apply --date format
    if let Some(ref date_str) = args.date {
        format_options.date_format = parse_date_format(date_str);
    }

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

    // Build decoration map if needed
    let decorations = if args.decorate {
        Some(build_decoration_map(&repo)?)
    } else {
        None
    };

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

        // Filter by merge status
        if args.merges && commit.parents.len() < 2 {
            continue;
        }
        if args.no_merges && commit.parents.len() > 1 {
            continue;
        }

        // Filter by pathspec: skip commits that don't touch any of the given paths
        if !pathspecs.is_empty() {
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
                let touches_path = result.files.iter().any(|f| {
                    let path = f.path().to_str_lossy();
                    pathspecs.iter().any(|ps| path.starts_with(ps.as_str()))
                });
                if !touches_path {
                    continue;
                }
            }
        }

        // Format the commit
        let formatted = if let Some(ref fmt) = custom_format {
            format_commit_with_decorations(&commit, &oid, fmt, &format_options, decorations.as_ref())
        } else {
            format_builtin_with_decorations(&commit, &oid, builtin, &format_options, decorations.as_ref())
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

fn build_decoration_map(repo: &git_repository::Repository) -> Result<HashMap<ObjectId, Vec<String>>> {
    let mut map: HashMap<ObjectId, Vec<String>> = HashMap::new();

    // Get HEAD info
    let head_oid = repo.head_oid()?;
    let current_branch = repo.current_branch()?;

    // Collect all decorations per OID, then order: HEAD -> current, tag:*, other branches, remotes
    // We build separate lists and merge at the end.
    let mut head_entries: HashMap<ObjectId, String> = HashMap::new();
    let mut tag_entries: HashMap<ObjectId, Vec<String>> = HashMap::new();
    let mut branch_entries: HashMap<ObjectId, Vec<String>> = HashMap::new();
    let mut remote_entries: HashMap<ObjectId, Vec<String>> = HashMap::new();

    // Local branches
    if let Ok(refs) = repo.refs().iter(Some("refs/heads/")) {
        for r in refs {
            let r = r?;
            let name = r.name().as_str().to_string();
            let short = name.strip_prefix("refs/heads/").unwrap_or(&name).to_string();
            if let Ok(oid) = r.peel_to_oid(repo.refs()) {
                if current_branch.as_deref() == Some(short.as_str()) {
                    if let Some(ho) = head_oid {
                        if ho == oid {
                            head_entries.insert(oid, format!("HEAD -> {}", short));
                            continue;
                        }
                    }
                }
                branch_entries.entry(oid).or_default().push(short);
            }
        }
    }

    // Tags
    if let Ok(refs) = repo.refs().iter(Some("refs/tags/")) {
        for r in refs {
            let r = r?;
            let name = r.name().as_str().to_string();
            let short = name.strip_prefix("refs/tags/").unwrap_or(&name).to_string();
            if let Ok(oid) = r.peel_to_oid(repo.refs()) {
                tag_entries.entry(oid).or_default().push(format!("tag: {}", short));
            }
        }
    }

    // Remote-tracking branches
    if let Ok(refs) = repo.refs().iter(Some("refs/remotes/")) {
        for r in refs {
            let r = r?;
            let name = r.name().as_str().to_string();
            let short = name.strip_prefix("refs/remotes/").unwrap_or(&name).to_string();
            if let Ok(oid) = r.peel_to_oid(repo.refs()) {
                remote_entries.entry(oid).or_default().push(short);
            }
        }
    }

    // Merge: order is HEAD -> current, tag:*, local branches, remote branches
    let all_oids: std::collections::HashSet<ObjectId> = head_entries.keys()
        .chain(tag_entries.keys())
        .chain(branch_entries.keys())
        .chain(remote_entries.keys())
        .copied()
        .collect();

    for oid in all_oids {
        let entry = map.entry(oid).or_default();
        if let Some(head) = head_entries.get(&oid) {
            entry.push(head.clone());
        }
        if let Some(tags) = tag_entries.get(&oid) {
            entry.extend(tags.iter().cloned());
        }
        if let Some(branches) = branch_entries.get(&oid) {
            entry.extend(branches.iter().cloned());
        }
        if let Some(remotes) = remote_entries.get(&oid) {
            entry.extend(remotes.iter().cloned());
        }
    }

    Ok(map)
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

fn parse_date_format(s: &str) -> DateFormat {
    match s {
        "iso" | "iso8601" => DateFormat::Iso,
        "iso-strict" | "iso8601-strict" => DateFormat::IsoStrict,
        "relative" => DateFormat::Relative,
        "short" => DateFormat::Short,
        "default" => DateFormat::Default,
        "raw" => DateFormat::Raw,
        "unix" => DateFormat::Unix,
        "rfc" | "rfc2822" => DateFormat::Rfc2822,
        "local" => DateFormat::Local,
        "human" => DateFormat::Human,
        other => {
            if let Some(strftime) = other.strip_prefix("format:") {
                DateFormat::Custom(strftime.to_string())
            } else {
                DateFormat::Default
            }
        }
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
        write!(out, "\n{}", output)?;
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

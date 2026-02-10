use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};

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
use git_utils::color::{ColorConfig, ColorSlot};
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

    /// When to show colored output (auto, always, never)
    #[arg(long, value_name = "when")]
    color: Option<String>,

    /// Abbreviate commit hash to 7 chars
    #[arg(long)]
    abbrev_commit: bool,

    /// Suppress decoration output
    #[arg(long)]
    no_decorate: bool,

    /// Walk reflogs instead of commit graph
    #[arg(short = 'g', long = "walk-reflogs")]
    walk_reflogs: bool,

    /// Show <> markers for symmetric diff
    #[arg(long)]
    left_right: bool,

    /// Omit equivalent commits in symmetric diff
    #[arg(long)]
    cherry_pick: bool,

    /// Mark equivalent commits with = vs + in symmetric diff
    #[arg(long)]
    cherry_mark: bool,

    /// Only show commits on ancestry path
    #[arg(long)]
    ancestry_path: bool,

    /// Only show decorated commits
    #[arg(long)]
    simplify_by_decoration: bool,

    /// Show which ref led to each commit
    #[arg(long)]
    source: bool,

    /// Apply mailmap transformations
    #[arg(long)]
    use_mailmap: bool,

    /// Track file renames
    #[arg(long)]
    follow: bool,

    /// Detect renames (optionally with similarity threshold percentage, e.g. -M50)
    #[arg(short = 'M', num_args = 0..=1, default_missing_value = "50")]
    find_renames: Option<u8>,

    /// Detect copies (optionally with similarity threshold percentage, e.g. -C50)
    #[arg(short = 'C', num_args = 0..=1, default_missing_value = "50")]
    find_copies: Option<u8>,

    /// Filter commits by diff status (A=Added, D=Deleted, M=Modified, etc.)
    #[arg(long = "diff-filter", value_name = "ACDMRTUXB*")]
    diff_filter: Option<String>,

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

    // Determine color settings
    let color_config = load_color_config(&repo);
    let cli_color_mode = args.color.as_deref().map(git_utils::color::parse_color_mode);
    let effective_mode = color_config.effective_mode("log", cli_color_mode);
    let color_enabled = git_utils::color::use_color(effective_mode, io::stdout().is_terminal());

    // Validate flag combinations
    if args.follow && pathspecs_from_args(&args.revisions, &args.path_args).len() > 1 {
        anyhow::bail!("--follow requires exactly one path");
    }
    if (args.cherry_pick || args.cherry_mark || args.left_right)
        && !args.revisions.iter().any(|r| r.contains("..."))
    {
        // These flags require a symmetric range (A...B)
        // If no symmetric range provided, they're silently ignored (matching git behavior)
    }

    // Load mailmap if requested
    let mailmap = if args.use_mailmap {
        let work_tree = repo.work_tree().map(|p| p.to_path_buf());
        if let Some(ref wt) = work_tree {
            let mailmap_path = wt.join(".mailmap");
            if mailmap_path.exists() {
                git_utils::mailmap::Mailmap::from_file(&mailmap_path).ok()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Parse format
    let (builtin, custom_format) = parse_format(args);
    let mut format_options = FormatOptions::default();
    // --oneline is shorthand for --format=oneline --abbrev-commit
    if args.oneline || args.abbrev_commit {
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
        left_right: args.left_right,
        cherry_pick: args.cherry_pick,
        cherry_mark: args.cherry_mark,
        ancestry_path: args.ancestry_path,
        source: args.source,
        follow_path: if args.follow {
            pathspecs_from_args(&args.revisions, &args.path_args)
                .into_iter()
                .next()
                .map(bstr::BString::from)
        } else {
            None
        },
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
    let follow_path = walk_opts.follow_path.clone();
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

    // Detect symmetric range for left-right/cherry-pick/cherry-mark
    let symmetric_range = if args.left_right || args.cherry_pick || args.cherry_mark {
        revs.iter().find(|r| r.contains("...")).cloned()
    } else {
        None
    };

    // Build commit annotation map for symmetric diff flags
    let mut commit_annotations: HashMap<ObjectId, (char, git_revwalk::DiffSide)> = HashMap::new();
    if let Some(ref sym_range) = symmetric_range {
        let parts: Vec<&str> = sym_range.splitn(2, "...").collect();
        if parts.len() == 2 {
            let left_rev = if parts[0].is_empty() { "HEAD" } else { parts[0] };
            let right_rev = if parts[1].is_empty() { "HEAD" } else { parts[1] };
            let left_oid = git_revwalk::resolve_revision(&repo, left_rev)?;
            let right_oid = git_revwalk::resolve_revision(&repo, right_rev)?;

            let (left_entries, right_entries) =
                git_revwalk::symmetric_diff_with_cherry(&repo, &left_oid, &right_oid)?;

            for entry in &left_entries {
                commit_annotations.insert(entry.oid, (entry.marker, git_revwalk::DiffSide::Left));
            }
            for entry in &right_entries {
                commit_annotations.insert(entry.oid, (entry.marker, git_revwalk::DiffSide::Right));
            }
        }
    }

    // Build source ref tracking map (for --source)
    let mut source_map: HashMap<ObjectId, String> = HashMap::new();

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
                // Track source ref for --source
                if args.source {
                    source_map.insert(oid, rev.clone());
                }
            }
        }
    }

    // For --source with --all, build source tracking from all refs
    if args.source && args.all {
        if let Ok(refs) = repo.refs().iter(None) {
            for r in refs.flatten() {
                if let Some(oid) = r.target_oid() {
                    let name = r.name().as_str().to_string();
                    source_map.entry(oid).or_insert(name);
                }
            }
        }
    }

    // Build decoration map if needed (--no-decorate suppresses)
    let decorations = if args.decorate && !args.no_decorate {
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

    // --walk-reflogs: walk reflog entries instead of the commit graph
    if args.walk_reflogs {
        return walk_reflogs_mode(
            &repo,
            &revs,
            builtin,
            &custom_format,
            &format_options,
            decorations.as_ref(),
            color_enabled,
            &color_config,
            args,
            &mut out,
        );
    }

    // Walk commits
    let mut first_commit = true;
    for oid_result in walker {
        let oid = oid_result?;

        // For symmetric diff flags: filter/annotate using commit_annotations map
        if !commit_annotations.is_empty() {
            if let Some(&(marker, _side)) = commit_annotations.get(&oid) {
                // --cherry-pick: omit equivalent commits
                if args.cherry_pick && marker == '=' {
                    continue;
                }
            } else {
                // Commit not in symmetric diff — skip it if we're doing symmetric diff filtering
                continue;
            }
        }

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

        // --simplify-by-decoration: only show commits that have decorations
        if args.simplify_by_decoration {
            if let Some(ref dec) = decorations {
                if !dec.contains_key(&oid) {
                    continue;
                }
            } else {
                continue;
            }
        }

        // Filter by pathspec: skip commits that don't touch any of the given paths
        if !pathspecs.is_empty() && !args.follow {
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

        // --follow: filter by followed path, detecting renames
        if args.follow && follow_path.is_some() {
            // The follow path is also used as a pathspec for filtering
            let parent_tree = commit.first_parent().and_then(|p| {
                repo.odb().read(p).ok().flatten().and_then(|o| match o {
                    Object::Commit(c) => Some(c.tree),
                    _ => None,
                })
            });
            let diff_opts = DiffOptions { detect_renames: true, ..Default::default() };
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

        // --diff-filter: only include commits whose diffs contain files with matching status
        if let Some(ref filter) = args.diff_filter {
            let parent_tree = commit.first_parent().and_then(|p| {
                repo.odb().read(p).ok().flatten().and_then(|o| match o {
                    Object::Commit(c) => Some(c.tree),
                    _ => None,
                })
            });
            let diff_opts = DiffOptions::default();
            if let Ok(diff_result) = git_diff::tree::diff_trees(
                repo.odb(),
                parent_tree.as_ref(),
                Some(&commit.tree),
                &diff_opts,
            ) {
                let filter_upper = filter.to_uppercase();
                let has_matching = diff_result
                    .files
                    .iter()
                    .any(|f| filter_upper.contains(f.status.as_char()));
                if !has_matching {
                    continue;
                }
            }
        }

        // Apply mailmap transformations if requested
        let commit = if let Some(ref mm) = mailmap {
            apply_mailmap(&commit, mm)
        } else {
            commit
        };

        // Format the commit
        let formatted = if let Some(ref fmt) = custom_format {
            format_commit_with_decorations(&commit, &oid, fmt, &format_options, decorations.as_ref())
        } else {
            format_builtin_with_decorations(&commit, &oid, builtin, &format_options, decorations.as_ref())
        };

        // Add prefix annotations for --left-right, --cherry-mark, --source
        let formatted = {
            let mut prefix = String::new();
            if let Some(&(marker, side)) = commit_annotations.get(&oid) {
                if args.left_right {
                    match side {
                        git_revwalk::DiffSide::Left => prefix.push_str("< "),
                        git_revwalk::DiffSide::Right => prefix.push_str("> "),
                    }
                }
                if args.cherry_mark {
                    prefix.push(marker);
                    prefix.push(' ');
                }
            }
            if args.source {
                if let Some(ref src) = source_map.get(&oid) {
                    // Source is appended after the first line's commit hash
                    // For simplicity, prepend it
                    if prefix.is_empty() {
                        prefix.push_str(&format!("{}\t", src));
                    }
                }
            }
            if prefix.is_empty() {
                formatted
            } else {
                format!("{}{}", prefix, formatted)
            }
        };

        // Add separator between commits for multi-line formats
        let needs_separator = custom_format.is_none() && builtin != BuiltinFormat::Oneline;
        if needs_separator && !first_commit {
            writeln!(out)?;
        }
        first_commit = false;

        // Optionally colorize the formatted output
        let formatted = if color_enabled {
            colorize_log_output(&formatted, &color_config, builtin, custom_format.is_some())
        } else {
            formatted
        };

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
            show_commit_diff(&repo, &commit, &oid, args, color_enabled, &color_config, &mut out)?;
        }
    }

    Ok(0)
}

/// Public wrapper for building a decoration map, used by show.rs.
pub fn build_decoration_map_for_show(
    repo: &git_repository::Repository,
) -> Result<HashMap<ObjectId, Vec<String>>> {
    build_decoration_map(repo)
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
    color_enabled: bool,
    color_config: &ColorConfig,
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

    let mut diff_opts = DiffOptions {
        detect_renames: args.find_renames.is_some(),
        rename_threshold: args.find_renames.unwrap_or(50),
        detect_copies: args.find_copies.is_some(),
        copy_threshold: args.find_copies.unwrap_or(50),
        ..DiffOptions::default()
    };
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
        if color_enabled && diff_opts.output_format == DiffOutputFormat::Unified {
            let reset = color_config.get_color(ColorSlot::Reset);
            writeln!(out)?;
            for line in output.lines() {
                let colored = super::diff::colorize_diff_output_line(line, color_config, reset);
                writeln!(out, "{}", colored)?;
            }
        } else {
            write!(out, "\n{}", output)?;
        }
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
            if color_enabled {
                let reset = color_config.get_color(ColorSlot::Reset);
                for line in output.lines() {
                    let colored = super::diff::colorize_diff_output_line(line, color_config, reset);
                    writeln!(out, "{}", colored)?;
                }
            } else {
                write!(out, "{}", output)?;
            }
        }
    }

    Ok(())
}

/// Colorize the formatted log output for a single commit.
fn colorize_log_output(
    formatted: &str,
    cc: &ColorConfig,
    builtin: BuiltinFormat,
    is_custom: bool,
) -> String {
    let reset = cc.get_color(ColorSlot::Reset);
    let mut result = String::with_capacity(formatted.len() + 64);

    for line in formatted.lines() {
        let colored = if is_custom {
            // For custom formats, just try to colorize hex hashes at the start
            colorize_oneline_hash(line, reset)
        } else {
            match builtin {
                BuiltinFormat::Oneline => colorize_oneline_hash(line, reset),
                _ => colorize_log_line(line, cc, reset),
            }
        };
        result.push_str(&colored);
        result.push('\n');
    }

    // Remove the trailing newline to match original formatting
    if result.ends_with('\n') && !formatted.ends_with('\n') {
        result.pop();
    }

    result
}

/// Colorize a "commit <hash>" line or a line with decorations.
fn colorize_log_line(line: &str, cc: &ColorConfig, reset: &str) -> String {
    let hash_color = "\x1b[33m"; // Yellow for commit hashes

    if let Some(rest) = line.strip_prefix("commit ") {
        // Check for decorations: "commit <hash> (ref1, ref2)"
        if let Some(paren_pos) = rest.find(" (") {
            let hash = &rest[..paren_pos];
            let after_hash = &rest[paren_pos..];
            return format!(
                "{}commit {}{}{}",
                hash_color,
                hash,
                reset,
                colorize_decorations(after_hash, cc, reset),
            );
        }
        // No decorations: "commit <hash>"
        return format!("{}commit {}{}", hash_color, rest, reset);
    }

    line.to_string()
}

/// Colorize a oneline format line: "<hash> <subject>" or "<hash> (decorations) <subject>"
fn colorize_oneline_hash(line: &str, reset: &str) -> String {
    let hash_color = "\x1b[33m"; // Yellow for commit hashes

    // Check if line starts with hex characters (a commit hash)
    if line.len() >= 7 {
        let first_non_hex = line.find(|c: char| !c.is_ascii_hexdigit());
        if let Some(pos) = first_non_hex {
            if pos >= 7 && line.as_bytes().get(pos) == Some(&b' ') {
                let hash = &line[..pos];
                let rest = &line[pos..];
                return format!("{}{}{}{}", hash_color, hash, reset, rest);
            }
        }
    }

    line.to_string()
}

/// Colorize decoration text like " (HEAD -> main, tag: v1.0, origin/main)".
fn colorize_decorations(text: &str, cc: &ColorConfig, reset: &str) -> String {
    // Look for " (ref1, ref2, ref3)" pattern
    if let Some(start) = text.find(" (") {
        if let Some(end) = text[start..].find(')') {
            let before = &text[..start];
            let refs_str = &text[start + 2..start + end];
            let after = &text[start + end + 1..];

            let colored_refs: Vec<String> = refs_str
                .split(", ")
                .map(|r| {
                    let r = r.trim();
                    if let Some(branch) = r.strip_prefix("HEAD -> ") {
                        // Colorize HEAD and branch separately
                        format!(
                            "{}HEAD -> {}{}{}",
                            cc.get_color(ColorSlot::DecorateHead),
                            cc.get_color(ColorSlot::DecorateBranch),
                            branch,
                            reset,
                        )
                    } else if r == "HEAD" {
                        format!(
                            "{}{}{}",
                            cc.get_color(ColorSlot::DecorateHead),
                            r,
                            reset
                        )
                    } else if r.starts_with("tag: ") {
                        format!(
                            "{}{}{}",
                            cc.get_color(ColorSlot::DecorateTag),
                            r,
                            reset
                        )
                    } else if r.contains('/') {
                        format!(
                            "{}{}{}",
                            cc.get_color(ColorSlot::DecorateRemote),
                            r,
                            reset
                        )
                    } else {
                        format!(
                            "{}{}{}",
                            cc.get_color(ColorSlot::DecorateBranch),
                            r,
                            reset
                        )
                    }
                })
                .collect();

            return format!(
                "{} \x1b[33m(\x1b[m{}\x1b[33m)\x1b[m{}",
                before,
                colored_refs.join("\x1b[33m, \x1b[m"),
                after,
            );
        }
    }
    text.to_string()
}

/// Load color configuration from the repository config (best-effort).
fn load_color_config(repo: &git_repository::Repository) -> ColorConfig {
    let config = repo.config();
    ColorConfig::from_config(|key| config.get_string(key).ok().flatten())
}

/// Apply mailmap transformations to a commit's author and committer.
fn apply_mailmap(commit: &Commit, mm: &git_utils::mailmap::Mailmap) -> Commit {
    let mut commit = commit.clone();
    let (author_name, author_email) = mm.lookup(&commit.author.name, &commit.author.email);
    commit.author.name = author_name;
    commit.author.email = author_email;
    let (committer_name, committer_email) =
        mm.lookup(&commit.committer.name, &commit.committer.email);
    commit.committer.name = committer_name;
    commit.committer.email = committer_email;
    commit
}

/// Walk reflog entries instead of the commit graph (-g/--walk-reflogs).
#[allow(clippy::too_many_arguments)]
fn walk_reflogs_mode(
    repo: &git_repository::Repository,
    revs: &[String],
    builtin: BuiltinFormat,
    custom_format: &Option<String>,
    format_options: &FormatOptions,
    decorations: Option<&HashMap<ObjectId, Vec<String>>>,
    color_enabled: bool,
    color_config: &ColorConfig,
    args: &LogArgs,
    out: &mut impl Write,
) -> Result<i32> {
    use git_ref::RefName;

    let ref_name_str = if revs.is_empty() {
        "HEAD".to_string()
    } else {
        let r = &revs[0];
        if r.starts_with("refs/") {
            r.clone()
        } else {
            format!("refs/heads/{}", r)
        }
    };

    let ref_name = RefName::new(ref_name_str.as_str())
        .map_err(|e| anyhow::anyhow!("invalid ref name '{}': {}", ref_name_str, e))?;
    let entries = repo.refs().reflog(&ref_name)?;

    let mut first_commit = true;
    for entry in entries.iter().rev() {
        let oid = entry.new_oid;
        let obj = match repo.odb().read(&oid)? {
            Some(o) => o,
            None => continue,
        };
        let commit = match obj {
            Object::Commit(c) => c,
            _ => continue,
        };

        let formatted = if let Some(ref fmt) = custom_format {
            format_commit_with_decorations(
                &commit,
                &oid,
                fmt,
                format_options,
                decorations,
            )
        } else {
            format_builtin_with_decorations(
                &commit,
                &oid,
                builtin,
                format_options,
                decorations,
            )
        };

        let needs_separator = custom_format.is_none() && builtin != BuiltinFormat::Oneline;
        if needs_separator && !first_commit {
            writeln!(out)?;
        }
        first_commit = false;

        let formatted = if color_enabled {
            colorize_log_output(&formatted, color_config, builtin, custom_format.is_some())
        } else {
            formatted
        };

        write!(out, "{}", formatted)?;
        if custom_format.is_some() || builtin == BuiltinFormat::Oneline {
            writeln!(out)?;
        }

        // Append stat/patch if requested
        if args.stat || args.patch || args.name_only || args.name_status {
            show_commit_diff(repo, &commit, &oid, args, color_enabled, color_config, out)?;
        }
    }

    Ok(0)
}

/// Extract pathspecs from revision args and path_args.
fn pathspecs_from_args(revisions: &[String], path_args: &[String]) -> Vec<String> {
    let mut pathspecs: Vec<String> = path_args.to_vec();
    let mut saw_separator = false;
    for arg in revisions {
        if arg == "--" {
            saw_separator = true;
            continue;
        }
        if saw_separator {
            pathspecs.push(arg.clone());
        }
    }
    pathspecs
}

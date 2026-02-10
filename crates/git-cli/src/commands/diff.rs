use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::pickaxe::{self, PickaxeMode};
use git_diff::{DiffAlgorithm, DiffOptions, DiffOutputFormat};
use git_object::Object;
use git_utils::color::{ColorConfig, ColorMode, ColorSlot};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DiffArgs {
    /// Show staged changes (index vs HEAD)
    #[arg(long)]
    cached: bool,

    /// Alias for --cached
    #[arg(long)]
    staged: bool,

    /// Show diffstat instead of patch
    #[arg(long)]
    stat: bool,

    /// Show short stat
    #[arg(long)]
    shortstat: bool,

    /// Show numstat
    #[arg(long)]
    numstat: bool,

    /// Show name-only
    #[arg(long)]
    name_only: bool,

    /// Show name-status
    #[arg(long)]
    name_status: bool,

    /// Show summary
    #[arg(long)]
    summary: bool,

    /// Show raw diff output
    #[arg(long)]
    raw: bool,

    /// Don't show diff, just check for changes
    #[arg(long)]
    quiet: bool,

    /// Generate diff in unified format with <n> lines of context
    #[arg(short = 'U', long = "unified")]
    context_lines: Option<u32>,

    /// Detect renames (optionally with similarity threshold percentage, e.g. -M50)
    #[arg(short = 'M', long, num_args = 0..=1, default_missing_value = "50")]
    find_renames: Option<u8>,

    /// Detect copies (optionally with similarity threshold percentage, e.g. -C50)
    #[arg(short = 'C', long, num_args = 0..=1, default_missing_value = "50")]
    find_copies: Option<u8>,

    /// Show word-level diff using [-removed-]{+added+} markers
    #[arg(long)]
    word_diff: bool,

    /// When to show colored output (auto, always, never)
    #[arg(long, value_name = "when")]
    color: Option<String>,

    /// Show word-level diff with color (implies --word-diff --color)
    #[arg(long)]
    color_words: bool,

    /// Show full object IDs in diff header
    #[arg(long)]
    full_index: bool,

    /// Reverse diff (swap old/new)
    #[arg(short = 'R')]
    reverse: bool,

    /// Pickaxe: find diffs that change occurrences of string
    #[arg(short = 'S', value_name = "string")]
    pickaxe_string: Option<String>,

    /// Pickaxe: find diffs whose patch text matches regex
    #[arg(short = 'G', value_name = "regex")]
    pickaxe_regex: Option<String>,

    /// Filter files by status (A=Added, D=Deleted, M=Modified, R=Renamed, etc.)
    #[arg(long = "diff-filter", value_name = "ACDMRTUXB*")]
    diff_filter: Option<String>,

    /// Use patience diff algorithm
    #[arg(long)]
    patience: bool,

    /// Use histogram diff algorithm
    #[arg(long)]
    histogram: bool,

    /// Use minimal diff algorithm
    #[arg(long)]
    minimal: bool,

    /// Compare two paths outside a git repo
    #[arg(long)]
    no_index: bool,

    /// Check for whitespace errors
    #[arg(long)]
    check: bool,

    /// Custom source prefix (default "a/")
    #[arg(long = "src-prefix", value_name = "prefix")]
    src_prefix: Option<String>,

    /// Custom destination prefix (default "b/")
    #[arg(long = "dst-prefix", value_name = "prefix")]
    dst_prefix: Option<String>,

    /// No prefix on paths
    #[arg(long)]
    no_prefix: bool,

    /// NUL-terminated output
    #[arg(short = 'z')]
    nul_terminated: bool,

    /// Commits or paths to diff
    #[arg(value_name = "commit-or-path")]
    args: Vec<String>,
}

pub fn run(args: &DiffArgs, cli: &Cli) -> Result<i32> {
    // --no-index: TODO not yet implemented (requires comparing two arbitrary paths)
    if args.no_index {
        eprintln!("warning: --no-index is accepted but not yet implemented");
    }
    // --check: TODO not yet implemented (whitespace error checking)
    if args.check {
        eprintln!("warning: --check is accepted but not yet implemented");
    }

    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut diff_opts = DiffOptions {
        output_format: determine_output_format(args),
        detect_renames: args.find_renames.is_some(),
        rename_threshold: args.find_renames.unwrap_or(50),
        detect_copies: args.find_copies.is_some(),
        copy_threshold: args.find_copies.unwrap_or(50),
        ..DiffOptions::default()
    };
    if let Some(ctx) = args.context_lines {
        diff_opts.context_lines = ctx;
    }

    // Apply algorithm flags
    if args.patience {
        diff_opts.algorithm = DiffAlgorithm::Patience;
    } else if args.histogram {
        diff_opts.algorithm = DiffAlgorithm::Histogram;
    } else if args.minimal {
        diff_opts.algorithm = DiffAlgorithm::Minimal;
    }

    let is_cached = args.cached || args.staged;

    // Parse arguments: figure out if we have commits, pathspecs, or both
    let (commits, _pathspecs) = parse_diff_args(&args.args, &repo);

    let result = if commits.len() == 2 {
        // git diff <commit1> <commit2>
        let oid_a = git_revwalk::resolve_revision(&repo, &commits[0])?;
        let oid_b = git_revwalk::resolve_revision(&repo, &commits[1])?;

        let tree_a = get_commit_tree(&repo, &oid_a)?;
        let tree_b = get_commit_tree(&repo, &oid_b)?;

        git_diff::tree::diff_trees(repo.odb(), Some(&tree_a), Some(&tree_b), &diff_opts)?
    } else if commits.len() == 1 && commits[0].contains("..") {
        // git diff A..B
        let parts: Vec<&str> = commits[0].split("..").collect();
        let oid_a = git_revwalk::resolve_revision(&repo, parts[0])?;
        let oid_b = git_revwalk::resolve_revision(&repo, parts[1])?;

        let tree_a = get_commit_tree(&repo, &oid_a)?;
        let tree_b = get_commit_tree(&repo, &oid_b)?;

        git_diff::tree::diff_trees(repo.odb(), Some(&tree_a), Some(&tree_b), &diff_opts)?
    } else if commits.len() == 1 {
        // git diff <commit> — diff commit against working tree (or index if --cached)
        let oid = git_revwalk::resolve_revision(&repo, &commits[0])?;
        let tree = get_commit_tree(&repo, &oid)?;

        if is_cached {
            // Compare commit tree vs index
            let index_path = repo.git_dir().join("index");
            let index = if index_path.exists() {
                git_index::Index::read_from(&index_path)?
            } else {
                git_index::Index::new()
            };
            let index_tree = index.write_tree(repo.odb())?;
            git_diff::tree::diff_trees(repo.odb(), Some(&tree), Some(&index_tree), &diff_opts)?
        } else {
            // Compare commit tree against worktree:
            // First get HEAD-vs-index diff and index-vs-worktree diff, combine them.
            // Simpler approach: get index tree and worktree changes, show combined.
            // Actually for `diff HEAD`, we need: HEAD tree -> worktree content.
            // This equals: (HEAD -> index) + (index -> worktree) merged.
            let staged = git_diff::worktree::diff_head_to_index(&mut repo, &diff_opts)?;
            let unstaged = git_diff::worktree::diff_index_to_worktree(&mut repo, &diff_opts)?;
            // Merge: include all staged files and all unstaged files
            let mut files_map: std::collections::BTreeMap<bstr::BString, git_diff::FileDiff> = std::collections::BTreeMap::new();
            for file in staged.files {
                files_map.insert(file.path().clone(), file);
            }
            for file in unstaged.files {
                let path = file.path().clone();
                match files_map.entry(path) {
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        // File appears in both staged and unstaged — re-diff HEAD vs worktree
                        // Read the HEAD blob and worktree content
                        let commit_tree = tree;
                        let key = entry.key().clone();
                        if let Ok(old_data) = resolve_blob_from_tree(&repo, &commit_tree, &key) {
                            let work_tree = repo.work_tree().unwrap().to_path_buf();
                            let fs_path = work_tree.join(key.to_str_lossy().as_ref());
                            if let Ok(new_data) = std::fs::read(&fs_path) {
                                let binary = git_diff::binary::is_binary(&old_data) || git_diff::binary::is_binary(&new_data);
                                let hunks = if binary {
                                    Vec::new()
                                } else {
                                    git_diff::algorithm::diff_lines(&old_data, &new_data, diff_opts.algorithm, diff_opts.context_lines)
                                };
                                let old_oid = git_hash::hasher::Hasher::hash_object(git_hash::HashAlgorithm::Sha1, "blob", &old_data).ok();
                                let new_oid = git_hash::hasher::Hasher::hash_object(git_hash::HashAlgorithm::Sha1, "blob", &new_data).ok();
                                entry.insert(git_diff::FileDiff {
                                    status: git_diff::FileStatus::Modified,
                                    old_path: Some(file.path().clone()),
                                    new_path: Some(file.path().clone()),
                                    old_mode: file.old_mode,
                                    new_mode: file.new_mode,
                                    old_oid,
                                    new_oid,
                                    hunks,
                                    is_binary: binary,
                                    similarity: None,
                                });
                            }
                        }
                    }
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(file);
                    }
                }
            }
            git_diff::DiffResult { files: files_map.into_values().collect() }
        }
    } else if is_cached {
        // git diff --cached — staged changes (index vs HEAD)
        git_diff::worktree::diff_head_to_index(&mut repo, &diff_opts)?
    } else {
        // git diff — unstaged changes (worktree vs index)
        git_diff::worktree::diff_index_to_worktree(&mut repo, &diff_opts)?
    };

    // Apply pickaxe filtering (-S / -G)
    let result = if let Some(ref pattern) = args.pickaxe_string {
        let mode = PickaxeMode::string(pattern);
        pickaxe::filter_by_pickaxe(&result, &mode)
    } else if let Some(ref pattern) = args.pickaxe_regex {
        let mode = PickaxeMode::regex(pattern)
            .map_err(|e| anyhow::anyhow!("invalid -G regex '{}': {}", pattern, e))?;
        pickaxe::filter_by_pickaxe(&result, &mode)
    } else {
        result
    };

    // Apply --diff-filter
    let result = if let Some(ref filter) = args.diff_filter {
        filter_by_status(&result, filter)
    } else {
        result
    };

    // Apply -R (reverse diff): swap old/new paths and invert hunks
    let result = if args.reverse {
        reverse_diff(&result)
    } else {
        result
    };

    if args.quiet {
        return Ok(if result.is_empty() { 0 } else { 1 });
    }

    // Determine color settings
    let color_config = load_color_config(&repo);
    let cli_color_mode = args.color.as_deref().map(git_utils::color::parse_color_mode);
    // --color-words implies color=always if no explicit --color flag
    let cli_color_mode = if args.color_words && cli_color_mode.is_none() {
        Some(ColorMode::Always)
    } else {
        cli_color_mode
    };
    let effective_mode = color_config.effective_mode("diff", cli_color_mode);
    let color_enabled = git_utils::color::use_color(effective_mode, io::stdout().is_terminal());

    if !result.is_empty() {
        // If --color-words is set, switch to word diff format
        if args.color_words && diff_opts.output_format == DiffOutputFormat::Unified {
            diff_opts.output_format = DiffOutputFormat::WordDiff;
        }

        let mut output = format_diff(&result, &diff_opts);

        // Apply prefix transformations (--no-prefix, --src-prefix, --dst-prefix)
        if args.no_prefix {
            output = rewrite_prefixes(&output, "", "");
        } else if args.src_prefix.is_some() || args.dst_prefix.is_some() {
            let src = args.src_prefix.as_deref().unwrap_or("a/");
            let dst = args.dst_prefix.as_deref().unwrap_or("b/");
            output = rewrite_prefixes(&output, src, dst);
        }

        // Apply --full-index: expand abbreviated OIDs in "index" lines
        if args.full_index {
            output = expand_index_oids(&output);
        }

        // Apply -z: NUL-terminated output for name-only / name-status formats
        if args.nul_terminated {
            output = output.replace('\n', "\0");
        }

        if color_enabled {
            let reset = color_config.get_color(ColorSlot::Reset);
            for line in output.lines() {
                let colored = colorize_diff_output_line(line, &color_config, reset);
                writeln!(out, "{}", colored)?;
            }
        } else {
            write!(out, "{}", output)?;
        }
    }

    Ok(0)
}

/// Colorize a single line of diff output based on its prefix.
pub(crate) fn colorize_diff_output_line(line: &str, cc: &ColorConfig, reset: &str) -> String {
    if line.starts_with("diff --git")
        || line.starts_with("---")
        || line.starts_with("+++")
        || line.starts_with("index ")
        || line.starts_with("old mode")
        || line.starts_with("new mode")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
        || line.starts_with("similarity")
        || line.starts_with("rename")
        || line.starts_with("copy")
    {
        format!(
            "{}{}{}",
            cc.get_color(ColorSlot::DiffMetaInfo),
            line,
            reset
        )
    } else if line.starts_with("@@") {
        format!(
            "{}{}{}",
            cc.get_color(ColorSlot::DiffFragInfo),
            line,
            reset
        )
    } else if line.starts_with('-') {
        format!(
            "{}{}{}",
            cc.get_color(ColorSlot::DiffOldNormal),
            line,
            reset
        )
    } else if line.starts_with('+') {
        format!(
            "{}{}{}",
            cc.get_color(ColorSlot::DiffNewNormal),
            line,
            reset
        )
    } else {
        line.to_string()
    }
}

/// Load color configuration from the repository config (best-effort).
fn load_color_config(repo: &git_repository::Repository) -> ColorConfig {
    let config = repo.config();
    ColorConfig::from_config(|key| config.get_string(key).ok().flatten())
}

fn determine_output_format(args: &DiffArgs) -> DiffOutputFormat {
    if args.stat {
        DiffOutputFormat::Stat
    } else if args.shortstat {
        DiffOutputFormat::ShortStat
    } else if args.numstat {
        DiffOutputFormat::NumStat
    } else if args.name_only {
        DiffOutputFormat::NameOnly
    } else if args.name_status {
        DiffOutputFormat::NameStatus
    } else if args.summary {
        DiffOutputFormat::Summary
    } else if args.raw {
        DiffOutputFormat::Raw
    } else if args.word_diff {
        DiffOutputFormat::WordDiff
    } else {
        DiffOutputFormat::Unified
    }
}

fn parse_diff_args(args: &[String], repo: &git_repository::Repository) -> (Vec<String>, Vec<String>) {
    let mut commits = Vec::new();
    let mut pathspecs = Vec::new();
    let mut saw_separator = false;
    let mut in_pathspec_mode = false;

    for arg in args {
        if arg == "--" {
            saw_separator = true;
            in_pathspec_mode = true;
            continue;
        }
        if saw_separator || in_pathspec_mode {
            pathspecs.push(arg.clone());
        } else {
            // Try as revision first; if that fails and the path exists, treat as pathspec
            match git_revwalk::resolve_revision(repo, arg) {
                Ok(_) => commits.push(arg.clone()),
                Err(_) => {
                    if std::path::Path::new(arg).exists()
                        || repo
                            .work_tree()
                            .map(|wt| wt.join(arg).exists())
                            .unwrap_or(false)
                    {
                        pathspecs.push(arg.clone());
                        in_pathspec_mode = true;
                    } else {
                        commits.push(arg.clone());
                    }
                }
            }
        }
    }

    (commits, pathspecs)
}

fn get_commit_tree(
    repo: &git_repository::Repository,
    oid: &git_hash::ObjectId,
) -> Result<git_hash::ObjectId> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;
    match obj {
        Object::Commit(c) => Ok(c.tree),
        _ => anyhow::bail!("not a commit: {}", oid),
    }
}

fn resolve_blob_from_tree(
    repo: &git_repository::Repository,
    tree_oid: &git_hash::ObjectId,
    path: &bstr::BString,
) -> Result<Vec<u8>> {
    let path_str = path.to_str_lossy();
    let components: Vec<&str> = path_str.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = *tree_oid;
    for component in &components {
        let obj = repo.odb().read(&current)?
            .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
        let tree = match obj {
            Object::Tree(t) => t,
            _ => anyhow::bail!("not a tree"),
        };
        let entry = tree.entries.iter()
            .find(|e| e.name.as_bstr() == component.as_bytes().as_bstr())
            .ok_or_else(|| anyhow::anyhow!("path not found"))?;
        current = entry.oid;
    }
    let obj = repo.odb().read(&current)?
        .ok_or_else(|| anyhow::anyhow!("blob not found"))?;
    match obj {
        Object::Blob(b) => Ok(b.data.to_vec()),
        _ => anyhow::bail!("not a blob"),
    }
}

/// Filter diff result files by their status character.
/// filter_str contains characters like "ACDMRT" etc.
fn filter_by_status(result: &git_diff::DiffResult, filter_str: &str) -> git_diff::DiffResult {
    let filter_upper = filter_str.to_uppercase();
    let files: Vec<git_diff::FileDiff> = result
        .files
        .iter()
        .filter(|f| {
            let ch = f.status.as_char();
            filter_upper.contains(ch)
        })
        .cloned()
        .collect();
    git_diff::DiffResult { files }
}

/// Reverse a diff result: swap old/new paths, modes, OIDs and invert hunk lines.
fn reverse_diff(result: &git_diff::DiffResult) -> git_diff::DiffResult {
    let files = result
        .files
        .iter()
        .map(|f| {
            let reversed_hunks: Vec<git_diff::Hunk> = f
                .hunks
                .iter()
                .map(|h| {
                    let lines = h
                        .lines
                        .iter()
                        .map(|l| match l {
                            git_diff::DiffLine::Addition(s) => git_diff::DiffLine::Deletion(s.clone()),
                            git_diff::DiffLine::Deletion(s) => git_diff::DiffLine::Addition(s.clone()),
                            git_diff::DiffLine::Context(s) => git_diff::DiffLine::Context(s.clone()),
                        })
                        .collect();
                    git_diff::Hunk {
                        old_start: h.new_start,
                        old_count: h.new_count,
                        new_start: h.old_start,
                        new_count: h.old_count,
                        header: h.header.clone(),
                        lines,
                    }
                })
                .collect();

            let status = match f.status {
                git_diff::FileStatus::Added => git_diff::FileStatus::Deleted,
                git_diff::FileStatus::Deleted => git_diff::FileStatus::Added,
                other => other,
            };

            git_diff::FileDiff {
                status,
                old_path: f.new_path.clone(),
                new_path: f.old_path.clone(),
                old_mode: f.new_mode,
                new_mode: f.old_mode,
                old_oid: f.new_oid,
                new_oid: f.old_oid,
                hunks: reversed_hunks,
                is_binary: f.is_binary,
                similarity: f.similarity,
            }
        })
        .collect();
    git_diff::DiffResult { files }
}

/// Rewrite "--- a/" and "+++ b/" prefixes in unified diff output.
fn rewrite_prefixes(output: &str, src: &str, dst: &str) -> String {
    let mut result = String::with_capacity(output.len());
    for line in output.lines() {
        if let Some(path) = line.strip_prefix("--- a/") {
            result.push_str("--- ");
            result.push_str(src);
            result.push_str(path);
        } else if let Some(path) = line.strip_prefix("+++ b/") {
            result.push_str("+++ ");
            result.push_str(dst);
            result.push_str(path);
        } else if line.starts_with("diff --git a/") {
            // Rewrite "diff --git a/X b/X" header
            if let Some(b_pos) = line.rfind(" b/") {
                let path_a = &line[13..b_pos];
                let path_b = &line[b_pos + 3..];
                result.push_str("diff --git ");
                result.push_str(src);
                result.push_str(path_a);
                result.push(' ');
                result.push_str(dst);
                result.push_str(path_b);
            } else {
                result.push_str(line);
            }
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

/// Expand abbreviated OIDs in "index" lines to full 40-char hashes.
/// The flag is accepted for CLI parity; the formatter already emits whatever OID
/// length it has available, so this is effectively a pass-through.
fn expand_index_oids(output: &str) -> String {
    output.to_string()
}

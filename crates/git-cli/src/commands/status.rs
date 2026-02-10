use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::Result;
use bstr::{BString, ByteSlice};
use clap::Args;
use git_diff::{DiffOptions, FileStatus};
use git_index::IgnoreStack;
use git_utils::color::{self, ColorConfig, ColorSlot};

use crate::Cli;
use super::open_repo;

#[derive(Args, Default)]
pub struct StatusArgs {
    /// Give the output in the short-format
    #[arg(short, long)]
    pub short: bool,

    /// Show the branch and tracking info in short-format
    #[arg(short, long)]
    pub branch: bool,

    /// Give the output in machine-readable format
    #[arg(long)]
    pub porcelain: bool,

    /// Show the output in the long-format (default)
    #[arg(long)]
    pub long: bool,

    /// When to show colored output (auto, always, never)
    #[arg(long, value_name = "when")]
    pub color: Option<String>,

    /// Show staged diff inline (like `git diff --cached`)
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Terminate entries with NUL instead of LF
    #[arg(short = 'z')]
    pub nul_terminated: bool,

    /// Show untracked files (mode: no, normal, all)
    #[arg(short = 'u', long = "untracked-files", value_name = "mode", num_args = 0..=1, default_missing_value = "all")]
    pub untracked_files: Option<String>,

    /// Show ignored files as well
    #[arg(long)]
    pub ignored: bool,

    /// Display untracked files in columns
    #[arg(long)]
    pub column: bool,

    /// Do not display untracked files in columns
    #[arg(long)]
    pub no_column: bool,

    /// Show detailed ahead/behind counts relative to upstream
    #[arg(long)]
    pub ahead_behind: bool,

    /// Do not show ahead/behind counts relative to upstream
    #[arg(long)]
    pub no_ahead_behind: bool,
}

pub fn run(args: &StatusArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let options = DiffOptions {
        detect_renames: true,
        ..DiffOptions::default()
    };

    let cli_color = args.color.as_deref().map(color::parse_color_mode);
    let color_config = load_color_config(cli);
    let effective = color_config.effective_mode("status", cli_color);
    let color_on = color::use_color(effective, io::stdout().is_terminal());

    if args.short || args.porcelain {
        print_short_status(&mut repo, &work_tree, &options, args, color_on, &color_config, &mut out)?;
    } else {
        print_long_status(&mut repo, &work_tree, &options, args, color_on, &color_config, &mut out)?;
    }

    // --verbose: show staged diff inline after normal output
    if args.verbose {
        let staged_diff = git_diff::worktree::diff_head_to_index(&mut repo, &options)?;
        if !staged_diff.files.is_empty() {
            let diff_text = git_diff::format::format_diff(&staged_diff, &options);
            write!(out, "{}", diff_text)?;
        }
    }

    Ok(0)
}

/// Print status output (for use by other commands like stash pop).
pub fn print_status(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    options: &DiffOptions,
    args: &StatusArgs,
    out: &mut impl Write,
) -> Result<()> {
    // When called from other commands (e.g., stash), default to no color
    let cc = ColorConfig::new();
    if args.short || args.porcelain {
        print_short_status(repo, work_tree, options, args, false, &cc, out)
    } else {
        print_long_status(repo, work_tree, options, args, false, &cc, out)
    }
}

fn print_short_status(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    options: &DiffOptions,
    args: &StatusArgs,
    color_on: bool,
    cc: &ColorConfig,
    out: &mut impl Write,
) -> Result<()> {
    let reset = if color_on { cc.get_color(ColorSlot::Reset) } else { "" };
    let line_term: &str = if args.nul_terminated { "\0" } else { "\n" };

    if args.branch && !args.porcelain {
        if let Ok(Some(branch)) = repo.current_branch() {
            write!(out, "## {}{}", branch, line_term)?;
        } else {
            write!(out, "## HEAD (no branch){}", line_term)?;
        }
    }

    // Staged changes (HEAD vs index)
    let staged = git_diff::worktree::diff_head_to_index(repo, options)?;

    // Unstaged changes (index vs worktree)
    let unstaged = git_diff::worktree::diff_index_to_worktree(repo, options)?;

    // Build a combined status map and track renames
    let mut status_map: std::collections::BTreeMap<BString, (char, char)> =
        std::collections::BTreeMap::new();
    let mut rename_map: std::collections::HashMap<BString, BString> =
        std::collections::HashMap::new();

    for file in &staged.files {
        let path = file.path().clone();
        let code = file.status.as_char();
        if file.status == FileStatus::Renamed {
            if let Some(ref old) = file.old_path {
                rename_map.insert(path.clone(), old.clone());
            }
        }
        status_map.entry(path).or_insert((' ', ' ')).0 = code;
    }

    for file in &unstaged.files {
        let path = file.path().clone();
        let code = file.status.as_char();
        status_map.entry(path).or_insert((' ', ' ')).1 = code;
    }

    // Determine untracked files mode
    let show_untracked = resolve_untracked_mode(args);

    // Untracked files
    if show_untracked != UntrackedMode::No {
        let untracked = if show_untracked == UntrackedMode::All {
            find_untracked_all(repo, work_tree)?
        } else {
            find_untracked(repo, work_tree)?
        };
        for path in &untracked {
            status_map.entry(path.clone()).or_insert(('?', '?'));
        }
    }

    // Ignored files (if requested)
    if args.ignored {
        let ignored_files = find_ignored_files(repo, work_tree)?;
        for path in &ignored_files {
            status_map.entry(path.clone()).or_insert(('!', '!'));
        }
    }

    for (path, (idx, wt)) in &status_map {
        let idx_color = if color_on && *idx != ' ' && *idx != '?' && *idx != '!' {
            cc.get_color(ColorSlot::StatusAdded)
        } else if color_on && *idx == '?' {
            cc.get_color(ColorSlot::StatusUntracked)
        } else {
            ""
        };
        let wt_color = if color_on && *wt != ' ' && *wt != '?' && *wt != '!' {
            cc.get_color(ColorSlot::StatusChanged)
        } else if color_on && *wt == '?' {
            cc.get_color(ColorSlot::StatusUntracked)
        } else {
            ""
        };

        // For renames, show "R  old -> new" format
        if *idx == 'R' {
            if let Some(old_path) = rename_map.get(path) {
                write!(
                    out, "{}{}{}{}{} {} -> {}{}",
                    idx_color, idx, reset, wt_color, wt,
                    old_path.to_str_lossy(), path.to_str_lossy(), line_term
                )?;
                if color_on {
                    write!(out, "{}", reset)?;
                }
                continue;
            }
        }
        write!(
            out, "{}{}{}{}{} {}{}{}",
            idx_color, idx, reset, wt_color, wt, path.to_str_lossy(), reset, line_term
        )?;
    }

    Ok(())
}

fn print_long_status(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    options: &DiffOptions,
    args: &StatusArgs,
    color_on: bool,
    cc: &ColorConfig,
    out: &mut impl Write,
) -> Result<()> {
    let reset = if color_on { cc.get_color(ColorSlot::Reset) } else { "" };

    // Branch info
    match repo.current_branch() {
        Ok(Some(branch)) => writeln!(out, "On branch {}", branch)?,
        Ok(None) => {
            // Detached HEAD — show the short OID
            if let Ok(Some(oid)) = repo.head_oid() {
                let hex = oid.to_hex();
                let short = &hex[..7.min(hex.len())];
                writeln!(out, "HEAD detached at {}", short)?;
            } else {
                writeln!(out, "HEAD detached")?;
            }
        }
        Err(_) => {}
    }

    if repo.is_unborn()? {
        writeln!(out, "\nNo commits yet\n")?;
    }

    // Staged changes
    let is_initial = repo.is_unborn()?;
    let staged = git_diff::worktree::diff_head_to_index(repo, options)?;
    if !staged.files.is_empty() {
        writeln!(out, "Changes to be committed:")?;
        if is_initial {
            writeln!(
                out,
                "  (use \"git rm --cached <file>...\" to unstage)"
            )?;
        } else {
            writeln!(
                out,
                "  (use \"git restore --staged <file>...\" to unstage)"
            )?;
        }
        let staged_color = if color_on { cc.get_color(ColorSlot::StatusAdded) } else { "" };
        for file in &staged.files {
            let status_word = match file.status {
                FileStatus::Added => "new file",
                FileStatus::Deleted => "deleted",
                FileStatus::Modified => "modified",
                FileStatus::Renamed => "renamed",
                FileStatus::Copied => "copied",
                FileStatus::TypeChanged => "typechange",
                _ => "unknown",
            };
            writeln!(
                out,
                "\t{}{}:   {}{}",
                staged_color,
                status_word,
                file.path().to_str_lossy(),
                reset,
            )?;
        }
        writeln!(out)?;
    }

    // Unstaged changes
    let unstaged = git_diff::worktree::diff_index_to_worktree(repo, options)?;
    if !unstaged.files.is_empty() {
        writeln!(out, "Changes not staged for commit:")?;
        writeln!(
            out,
            "  (use \"git add <file>...\" to update what will be committed)"
        )?;
        writeln!(
            out,
            "  (use \"git restore <file>...\" to discard changes in working directory)"
        )?;
        let changed_color = if color_on { cc.get_color(ColorSlot::StatusChanged) } else { "" };
        for file in &unstaged.files {
            let status_word = match file.status {
                FileStatus::Deleted => "deleted",
                FileStatus::Modified => "modified",
                FileStatus::TypeChanged => "typechange",
                _ => "modified",
            };
            writeln!(
                out,
                "\t{}{}:   {}{}",
                changed_color,
                status_word,
                file.path().to_str_lossy(),
                reset,
            )?;
        }
        writeln!(out)?;
    }

    // Determine untracked files mode
    let show_untracked = resolve_untracked_mode(args);

    // Untracked files
    let untracked = if show_untracked == UntrackedMode::No {
        Vec::new()
    } else if show_untracked == UntrackedMode::All {
        find_untracked_all(repo, work_tree)?
    } else {
        find_untracked(repo, work_tree)?
    };
    if !untracked.is_empty() {
        writeln!(out, "Untracked files:")?;
        writeln!(
            out,
            "  (use \"git add <file>...\" to include in what will be committed)"
        )?;
        let untracked_color = if color_on { cc.get_color(ColorSlot::StatusUntracked) } else { "" };
        for path in &untracked {
            writeln!(out, "\t{}{}{}", untracked_color, path.to_str_lossy(), reset)?;
        }
        writeln!(out)?;
    }

    // Ignored files (if requested)
    if args.ignored {
        let ignored_files = find_ignored_files(repo, work_tree)?;
        if !ignored_files.is_empty() {
            writeln!(out, "Ignored files:")?;
            writeln!(
                out,
                "  (use \"git add -f <file>...\" to include in what will be committed)"
            )?;
            for path in &ignored_files {
                writeln!(out, "\t{}", path.to_str_lossy())?;
            }
            writeln!(out)?;
        }
    }

    if staged.files.is_empty() && unstaged.files.is_empty() && untracked.is_empty() {
        writeln!(out, "nothing to commit, working tree clean")?;
    } else if staged.files.is_empty() {
        if !untracked.is_empty() {
            writeln!(
                out,
                "nothing added to commit but untracked files present (use \"git add\" to track)"
            )?;
        } else {
            writeln!(
                out,
                "no changes added to commit (use \"git add\" and/or \"git commit -a\")"
            )?;
        }
    }

    Ok(())
}

fn find_untracked(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
) -> Result<Vec<BString>> {
    let mut ignores = IgnoreStack::new();
    let gitignore_path = work_tree.join(".gitignore");
    if gitignore_path.exists() {
        ignores.add_file(&gitignore_path, work_tree)?;
    }
    let info_exclude = repo.git_dir().join("info").join("exclude");
    if info_exclude.exists() {
        ignores.add_file(&info_exclude, work_tree)?;
    }

    // Collect all index paths
    let indexed_paths: std::collections::HashSet<BString> = {
        let index = repo.index()?;
        index.iter().map(|e| e.path.clone()).collect()
    };

    let mut untracked = Vec::new();
    find_untracked_recursive(work_tree, work_tree, &indexed_paths, &ignores, &mut untracked)?;
    Ok(untracked)
}

fn find_untracked_recursive(
    work_tree: &Path,
    dir: &Path,
    indexed: &std::collections::HashSet<BString>,
    ignores: &IgnoreStack,
    result: &mut Vec<BString>,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();

        if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            continue;
        }

        let rel = path.strip_prefix(work_tree).unwrap_or(&path);
        let rel_bstr = BString::from(rel.to_str().unwrap_or("").as_bytes());
        let is_dir = path.is_dir();

        if ignores.is_ignored(rel_bstr.as_ref(), is_dir) {
            continue;
        }

        if is_dir {
            // Check if all files in this dir are untracked — collapse to "dir/"
            let has_tracked = has_tracked_files(work_tree, &path, indexed, ignores);
            if !has_tracked {
                let mut dir_entry = rel_bstr.clone();
                dir_entry.extend_from_slice(b"/");
                result.push(dir_entry);
            } else {
                find_untracked_recursive(work_tree, &path, indexed, ignores, result)?;
            }
        } else if !indexed.contains(&rel_bstr) {
            result.push(rel_bstr);
        }
    }
    Ok(())
}

/// Check if a directory has any tracked files (files that exist in the index).
fn has_tracked_files(
    work_tree: &Path,
    dir: &Path,
    indexed: &std::collections::HashSet<BString>,
    ignores: &IgnoreStack,
) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            continue;
        }
        let rel = path.strip_prefix(work_tree).unwrap_or(&path);
        let rel_bstr = BString::from(rel.to_str().unwrap_or("").as_bytes());
        let is_dir = path.is_dir();

        if ignores.is_ignored(rel_bstr.as_ref(), is_dir) {
            continue;
        }

        if is_dir {
            if has_tracked_files(work_tree, &path, indexed, ignores) {
                return true;
            }
        } else if indexed.contains(&rel_bstr) {
            return true;
        }
    }
    false
}

fn load_color_config(cli: &Cli) -> ColorConfig {
    let config = if let Some(ref git_dir) = cli.git_dir {
        git_config::ConfigSet::load(Some(git_dir)).ok()
    } else {
        git_repository::Repository::discover(".")
            .ok()
            .and_then(|repo| git_config::ConfigSet::load(Some(repo.git_dir())).ok())
    };
    match config {
        Some(c) => ColorConfig::from_config(|key| c.get_string(key).ok().flatten()),
        None => ColorConfig::new(),
    }
}

/// Mode for displaying untracked files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UntrackedMode {
    /// Do not show untracked files.
    No,
    /// Show untracked directories (default).
    Normal,
    /// Show individual untracked files (recurse into directories).
    All,
}

/// Resolve the --untracked-files / -u flag value into an UntrackedMode.
fn resolve_untracked_mode(args: &StatusArgs) -> UntrackedMode {
    match args.untracked_files.as_deref() {
        None => UntrackedMode::Normal,   // not specified: default behavior
        Some("no") => UntrackedMode::No,
        Some("normal") => UntrackedMode::Normal,
        Some("all") => UntrackedMode::All,
        Some(_) => UntrackedMode::Normal, // unknown value: fall back to normal
    }
}

/// Find untracked files showing every individual file (--untracked-files=all).
/// Unlike `find_untracked`, this does not collapse untracked directories.
fn find_untracked_all(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
) -> Result<Vec<BString>> {
    let mut ignores = IgnoreStack::new();
    let gitignore_path = work_tree.join(".gitignore");
    if gitignore_path.exists() {
        ignores.add_file(&gitignore_path, work_tree)?;
    }
    let info_exclude = repo.git_dir().join("info").join("exclude");
    if info_exclude.exists() {
        ignores.add_file(&info_exclude, work_tree)?;
    }

    let indexed_paths: std::collections::HashSet<BString> = {
        let index = repo.index()?;
        index.iter().map(|e| e.path.clone()).collect()
    };

    let mut untracked = Vec::new();
    find_untracked_all_recursive(work_tree, work_tree, &indexed_paths, &ignores, &mut untracked)?;
    Ok(untracked)
}

fn find_untracked_all_recursive(
    work_tree: &Path,
    dir: &Path,
    indexed: &std::collections::HashSet<BString>,
    ignores: &IgnoreStack,
    result: &mut Vec<BString>,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();

        if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            continue;
        }

        let rel = path.strip_prefix(work_tree).unwrap_or(&path);
        let rel_bstr = BString::from(rel.to_str().unwrap_or("").as_bytes());
        let is_dir = path.is_dir();

        if ignores.is_ignored(rel_bstr.as_ref(), is_dir) {
            continue;
        }

        if is_dir {
            find_untracked_all_recursive(work_tree, &path, indexed, ignores, result)?;
        } else if !indexed.contains(&rel_bstr) {
            result.push(rel_bstr);
        }
    }
    Ok(())
}

/// Find ignored files in the work tree (for --ignored).
fn find_ignored_files(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
) -> Result<Vec<BString>> {
    let mut ignores = IgnoreStack::new();
    let gitignore_path = work_tree.join(".gitignore");
    if gitignore_path.exists() {
        ignores.add_file(&gitignore_path, work_tree)?;
    }
    let info_exclude = repo.git_dir().join("info").join("exclude");
    if info_exclude.exists() {
        ignores.add_file(&info_exclude, work_tree)?;
    }

    let indexed_paths: std::collections::HashSet<BString> = {
        let index = repo.index()?;
        index.iter().map(|e| e.path.clone()).collect()
    };

    let mut ignored = Vec::new();
    find_ignored_recursive(work_tree, work_tree, &indexed_paths, &ignores, &mut ignored)?;
    Ok(ignored)
}

fn find_ignored_recursive(
    work_tree: &Path,
    dir: &Path,
    indexed: &std::collections::HashSet<BString>,
    ignores: &IgnoreStack,
    result: &mut Vec<BString>,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();

        if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            continue;
        }

        let rel = path.strip_prefix(work_tree).unwrap_or(&path);
        let rel_bstr = BString::from(rel.to_str().unwrap_or("").as_bytes());
        let is_dir = path.is_dir();

        if ignores.is_ignored(rel_bstr.as_ref(), is_dir) {
            if is_dir {
                let mut dir_entry = rel_bstr;
                dir_entry.extend_from_slice(b"/");
                result.push(dir_entry);
            } else if !indexed.contains(&rel_bstr) {
                result.push(rel_bstr);
            }
        } else if is_dir {
            find_ignored_recursive(work_tree, &path, indexed, ignores, result)?;
        }
    }
    Ok(())
}

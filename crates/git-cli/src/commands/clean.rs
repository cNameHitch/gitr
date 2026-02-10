use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_index::IgnoreStack;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CleanArgs {
    /// Force (required unless clean.requireForce is false)
    #[arg(short, long)]
    force: bool,

    /// Remove untracked directories too
    #[arg(short, long)]
    directories: bool,

    /// Dry run - show what would be removed
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Remove ignored files too
    #[arg(short = 'x')]
    ignored: bool,

    /// Remove only ignored files
    #[arg(short = 'X')]
    only_ignored: bool,

    /// Be quiet
    #[arg(short, long)]
    quiet: bool,

    /// Interactive clean
    #[arg(short = 'i')]
    interactive: bool,

    /// Additional exclude patterns
    #[arg(short = 'e', long = "exclude")]
    exclude: Vec<String>,
}

pub fn run(args: &CleanArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    if !args.force && !args.dry_run && !args.interactive {
        bail!("fatal: clean.requireForce defaults to true and neither -i, -n, nor -f given; refusing to clean");
    }

    let mut ignores = IgnoreStack::new();
    if !args.ignored && !args.only_ignored {
        let gitignore = work_tree.join(".gitignore");
        if gitignore.exists() {
            ignores.add_file(&gitignore, &work_tree)?;
        }
        let info_exclude = repo.git_dir().join("info").join("exclude");
        if info_exclude.exists() {
            ignores.add_file(&info_exclude, &work_tree)?;
        }
    }

    let indexed_paths: std::collections::HashSet<BString> = {
        let index = repo.index()?;
        index.iter().map(|e| e.path.clone()).collect()
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut items = Vec::new();
    collect_clean_items(&work_tree, &work_tree, &indexed_paths, &ignores, args, &mut items)?;
    items.sort();

    if args.interactive {
        return run_interactive_clean(&items, &work_tree, args.quiet);
    }

    for item in &items {
        if args.dry_run {
            writeln!(out, "Would remove {}", item)?;
        } else {
            let full = work_tree.join(item.trim_end_matches('/'));
            if item.ends_with('/') {
                if !args.quiet {
                    writeln!(out, "Removing {}", item)?;
                }
                std::fs::remove_dir_all(&full)?;
            } else {
                if !args.quiet {
                    writeln!(out, "Removing {}", item)?;
                }
                std::fs::remove_file(&full)?;
            }
        }
    }

    Ok(0)
}

fn collect_clean_items(
    work_tree: &Path,
    dir: &Path,
    indexed: &std::collections::HashSet<BString>,
    ignores: &IgnoreStack,
    args: &CleanArgs,
    items: &mut Vec<String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            continue;
        }

        let rel = path.strip_prefix(work_tree).unwrap_or(&path);
        let rel_bstr = BString::from(rel.to_str().unwrap_or("").as_bytes());
        let is_dir = path.is_dir();

        let is_ignored = ignores.is_ignored(rel_bstr.as_ref(), is_dir);

        if args.only_ignored {
            if !is_ignored {
                if is_dir {
                    collect_clean_items(work_tree, &path, indexed, ignores, args, items)?;
                }
                continue;
            }
        } else if !args.ignored && is_ignored {
            continue;
        }

        if is_dir {
            if args.directories {
                let has_tracked = has_tracked_files(work_tree, &path, indexed);
                if !has_tracked {
                    items.push(format!("{}/", rel.display()));
                } else {
                    collect_clean_items(work_tree, &path, indexed, ignores, args, items)?;
                }
            } else {
                // Without -d, only recurse into directories that contain tracked files
                // (skip entirely untracked directories - matches git behavior)
                if has_tracked_files(work_tree, &path, indexed) {
                    collect_clean_items(work_tree, &path, indexed, ignores, args, items)?;
                }
            }
        } else if !indexed.contains(&rel_bstr) {
            items.push(format!("{}", rel.display()));
        }
    }
    Ok(())
}

fn has_tracked_files(
    work_tree: &Path,
    dir: &Path,
    indexed: &std::collections::HashSet<BString>,
) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(work_tree).unwrap_or(&path);
            let rel_bstr = BString::from(rel.to_str().unwrap_or("").as_bytes());
            if indexed.contains(&rel_bstr) {
                return true;
            }
            if path.is_dir() && has_tracked_files(work_tree, &path, indexed) {
                return true;
            }
        }
    }
    false
}

/// Run interactive clean mode matching git's `clean -i` UI.
///
/// Displays a numbered list of untracked files and presents an interactive menu
/// that lets the user clean all, filter by pattern, select by numbers, or quit.
/// Input is read from `/dev/tty` so the prompt works even when stdin is piped.
fn run_interactive_clean(items: &[String], work_tree: &Path, quiet: bool) -> Result<i32> {
    if items.is_empty() {
        return Ok(0);
    }

    let tty = std::fs::File::open("/dev/tty")
        .map_err(|e| anyhow::anyhow!("failed to open /dev/tty: {}", e))?;
    let mut tty_reader = BufReader::new(tty);

    let stderr = io::stderr();
    let mut err = stderr.lock();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Track which items are selected (all selected by default)
    let mut selected = vec![true; items.len()];

    loop {
        // Display numbered file list with selection markers
        writeln!(err)?;
        display_file_list(&mut err, items, &selected)?;
        writeln!(err)?;

        // Show menu prompt
        write!(
            err,
            "Would you like to [c]lean, [f]ilter by pattern, [s]elect by numbers, or [q]uit? "
        )?;
        err.flush()?;

        let mut input = String::new();
        if tty_reader.read_line(&mut input)? == 0 {
            // EOF on tty — treat as quit
            return Ok(0);
        }
        let cmd = input.trim();

        match cmd {
            "c" => {
                // Collect selected items
                let to_clean: Vec<&String> = items
                    .iter()
                    .zip(selected.iter())
                    .filter(|(_, &s)| s)
                    .map(|(item, _)| item)
                    .collect();

                if to_clean.is_empty() {
                    writeln!(err, "Nothing to clean")?;
                    continue;
                }

                // Ask for confirmation
                write!(err, "Remove {} item(s)? [y/n] ", to_clean.len())?;
                err.flush()?;

                let mut confirm = String::new();
                if tty_reader.read_line(&mut confirm)? == 0 {
                    return Ok(0);
                }

                if confirm.trim() == "y" {
                    for item in &to_clean {
                        let full = work_tree.join(item.trim_end_matches('/'));
                        if item.ends_with('/') {
                            if !quiet {
                                writeln!(out, "Removing {}", item)?;
                            }
                            std::fs::remove_dir_all(&full)?;
                        } else {
                            if !quiet {
                                writeln!(out, "Removing {}", item)?;
                            }
                            std::fs::remove_file(&full)?;
                        }
                    }
                    return Ok(0);
                }
                // If not confirmed, loop back to menu
            }
            "f" => {
                // Filter by pattern
                write!(err, "Input ignore pattern: ")?;
                err.flush()?;

                let mut pattern = String::new();
                if tty_reader.read_line(&mut pattern)? == 0 {
                    return Ok(0);
                }
                let pattern = pattern.trim();

                if pattern.is_empty() {
                    continue;
                }

                // Apply glob pattern to select matching files
                for (i, item) in items.iter().enumerate() {
                    selected[i] = glob_matches(pattern, item);
                }
            }
            "s" => {
                // Select by numbers
                display_file_list(&mut err, items, &selected)?;
                write!(err, "Select items to delete (e.g., 1-3,5): ")?;
                err.flush()?;

                let mut number_input = String::new();
                if tty_reader.read_line(&mut number_input)? == 0 {
                    return Ok(0);
                }
                let number_input = number_input.trim();

                if number_input.is_empty() {
                    continue;
                }

                // Parse number ranges and toggle selection
                let indices = parse_number_selection(number_input, items.len());
                // Toggle: if input is given, set exactly those indices as selected
                for s in selected.iter_mut() {
                    *s = false;
                }
                for idx in indices {
                    if idx < selected.len() {
                        selected[idx] = true;
                    }
                }
            }
            "q" => {
                return Ok(0);
            }
            _ => {
                writeln!(
                    err,
                    "Huh ({})?\n",
                    cmd
                )?;
            }
        }
    }
}

/// Display the numbered file list with selection markers.
///
/// Selected files are shown with `*` prefix, unselected with a space.
fn display_file_list(
    out: &mut impl Write,
    items: &[String],
    selected: &[bool],
) -> io::Result<()> {
    for (i, item) in items.iter().enumerate() {
        let marker = if selected[i] { '*' } else { ' ' };
        writeln!(out, "  {} {:>2}) {}", marker, i + 1, item)?;
    }
    Ok(())
}

/// Parse a number selection string like "1-3,5,7-9" into a list of 0-based indices.
///
/// Invalid tokens are silently ignored. Out-of-range numbers are clamped.
fn parse_number_selection(input: &str, max: usize) -> Vec<usize> {
    let mut indices = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(dash_pos) = part.find('-') {
            // Range: "start-end"
            let start_str = part[..dash_pos].trim();
            let end_str = part[dash_pos + 1..].trim();
            if let (Ok(start), Ok(end)) = (start_str.parse::<usize>(), end_str.parse::<usize>()) {
                if start >= 1 && end >= start {
                    let start_idx = start.saturating_sub(1);
                    let end_idx = end.min(max).saturating_sub(1);
                    for idx in start_idx..=end_idx {
                        if idx < max {
                            indices.push(idx);
                        }
                    }
                }
            }
        } else if let Ok(num) = part.parse::<usize>() {
            // Single number
            if num >= 1 && num <= max {
                indices.push(num - 1);
            }
        }
    }

    indices
}

/// Simple glob pattern matching supporting `*` and `?` wildcards.
///
/// `*` matches any sequence of characters (including empty).
/// `?` matches any single character.
fn glob_matches(pattern: &str, text: &str) -> bool {
    glob_matches_inner(pattern.as_bytes(), text.as_bytes())
}

fn glob_matches_inner(pattern: &[u8], text: &[u8]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX;
    let mut star_ti = 0;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
}

use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use bstr::{BString, ByteSlice};
use clap::Args;
use git_diff::{DiffOptions, FileStatus};
use git_index::IgnoreStack;

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

    if args.short || args.porcelain {
        print_short_status(&mut repo, &work_tree, &options, args, &mut out)?;
    } else {
        print_long_status(&mut repo, &work_tree, &options, args, &mut out)?;
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
    if args.short || args.porcelain {
        print_short_status(repo, work_tree, options, args, out)
    } else {
        print_long_status(repo, work_tree, options, args, out)
    }
}

fn print_short_status(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    options: &DiffOptions,
    args: &StatusArgs,
    out: &mut impl Write,
) -> Result<()> {
    if args.branch && !args.porcelain {
        if let Ok(Some(branch)) = repo.current_branch() {
            writeln!(out, "## {}", branch)?;
        } else {
            writeln!(out, "## HEAD (no branch)")?;
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

    // Untracked files
    let untracked = find_untracked(repo, work_tree)?;
    for path in &untracked {
        status_map.entry(path.clone()).or_insert(('?', '?'));
    }

    for (path, (idx, wt)) in &status_map {
        // For renames, show "R  old -> new" format
        if *idx == 'R' {
            if let Some(old_path) = rename_map.get(path) {
                writeln!(out, "{}{} {} -> {}", idx, wt, old_path.to_str_lossy(), path.to_str_lossy())?;
                continue;
            }
        }
        writeln!(out, "{}{} {}", idx, wt, path.to_str_lossy())?;
    }

    Ok(())
}

fn print_long_status(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    options: &DiffOptions,
    _args: &StatusArgs,
    out: &mut impl Write,
) -> Result<()> {
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
                "\t{}:   {}",
                status_word,
                file.path().to_str_lossy()
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
        for file in &unstaged.files {
            let status_word = match file.status {
                FileStatus::Deleted => "deleted",
                FileStatus::Modified => "modified",
                FileStatus::TypeChanged => "typechange",
                _ => "modified",
            };
            writeln!(
                out,
                "\t{}:   {}",
                status_word,
                file.path().to_str_lossy()
            )?;
        }
        writeln!(out)?;
    }

    // Untracked files
    let untracked = find_untracked(repo, work_tree)?;
    if !untracked.is_empty() {
        writeln!(out, "Untracked files:")?;
        writeln!(
            out,
            "  (use \"git add <file>...\" to include in what will be committed)"
        )?;
        for path in &untracked {
            writeln!(out, "\t{}", path.to_str_lossy())?;
        }
        writeln!(out)?;
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

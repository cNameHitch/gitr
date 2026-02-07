use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use bstr::{BString, ByteSlice};
use clap::Args;
use git_diff::{DiffOptions, FileStatus};
use git_index::IgnoreStack;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct StatusArgs {
    /// Give the output in the short-format
    #[arg(short, long)]
    short: bool,

    /// Show the branch and tracking info in short-format
    #[arg(short, long)]
    branch: bool,

    /// Give the output in machine-readable format
    #[arg(long)]
    porcelain: bool,

    /// Show the output in the long-format (default)
    #[arg(long)]
    long: bool,
}

pub fn run(args: &StatusArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let options = DiffOptions::default();

    if args.short || args.porcelain {
        print_short_status(&mut repo, &work_tree, &options, args, &mut out)?;
    } else {
        print_long_status(&mut repo, &work_tree, &options, args, &mut out)?;
    }

    Ok(0)
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

    // Build a combined status map
    let mut status_map: std::collections::BTreeMap<BString, (char, char)> =
        std::collections::BTreeMap::new();

    for file in &staged.files {
        let path = file.path().clone();
        let code = file.status.as_char();
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
    let staged = git_diff::worktree::diff_head_to_index(repo, options)?;
    if !staged.files.is_empty() {
        writeln!(out, "Changes to be committed:")?;
        writeln!(
            out,
            "  (use \"git restore --staged <file>...\" to unstage)"
        )?;
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
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
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
            find_untracked_recursive(work_tree, &path, indexed, ignores, result)?;
        } else if !indexed.contains(&rel_bstr) {
            result.push(rel_bstr);
        }
    }
    Ok(())
}

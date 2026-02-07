use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice};
use clap::Args;
use git_index::{EntryFlags, IgnoreStack, IndexEntry, Stage, StatData};
use git_object::{FileMode, ObjectType};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct AddArgs {
    /// Add all tracked and untracked files
    #[arg(short = 'A', long = "all")]
    all: bool,

    /// Update tracked files only (no new files)
    #[arg(short, long)]
    update: bool,

    /// Don't actually add the files, just show what would happen
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Allow adding otherwise ignored files
    #[arg(short, long)]
    force: bool,

    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Files to add
    #[arg(value_name = "pathspec")]
    files: Vec<String>,
}

pub fn run(args: &AddArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    // Build ignore stack
    let mut ignores = IgnoreStack::new();
    let gitignore_path = work_tree.join(".gitignore");
    if gitignore_path.exists() {
        ignores.add_file(&gitignore_path, &work_tree)?;
    }
    let info_exclude = repo.git_dir().join("info").join("exclude");
    if info_exclude.exists() {
        ignores.add_file(&info_exclude, &work_tree)?;
    }

    let stderr = io::stderr();
    let mut err_out = stderr.lock();

    if args.all || (args.files.len() == 1 && args.files[0] == ".") {
        // Add everything
        add_all(&mut repo, &work_tree, &ignores, args, &mut err_out)?;
    } else if args.update {
        // Update tracked files only
        add_update(&mut repo, &work_tree, args, &mut err_out)?;
    } else if args.files.is_empty() {
        bail!("Nothing specified, nothing added.\nMaybe you wanted to say 'git add .'?");
    } else {
        // Add specific files
        add_files(&mut repo, &work_tree, &ignores, args, &mut err_out)?;
    }

    if !args.dry_run {
        repo.write_index()?;
    }

    Ok(0)
}

fn add_all(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    ignores: &IgnoreStack,
    args: &AddArgs,
    err_out: &mut impl Write,
) -> Result<()> {
    // First, handle deleted files (remove from index)
    let deleted: Vec<BString> = {
        let index = repo.index()?;
        index
            .iter()
            .filter(|e| e.stage == Stage::Normal)
            .filter(|e| {
                let path = work_tree.join(e.path.to_str_lossy().as_ref());
                !path.exists()
            })
            .map(|e| e.path.clone())
            .collect()
    };

    for path in &deleted {
        if args.verbose {
            writeln!(err_out, "remove '{}'", path.to_str_lossy())?;
        }
        if !args.dry_run {
            let index = repo.index_mut()?;
            index.remove(path.as_ref(), Stage::Normal);
        }
    }

    // Then add/update all files in working tree
    add_directory_recursive(repo, work_tree, work_tree, ignores, args, err_out)?;
    Ok(())
}

fn add_update(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    args: &AddArgs,
    err_out: &mut impl Write,
) -> Result<()> {
    // Collect paths from index first
    let tracked_paths: Vec<BString> = {
        let index = repo.index()?;
        index
            .iter()
            .filter(|e| e.stage == Stage::Normal)
            .map(|e| e.path.clone())
            .collect()
    };

    for path in &tracked_paths {
        let fs_path = work_tree.join(path.to_str_lossy().as_ref());
        if !fs_path.exists() {
            if args.verbose {
                writeln!(err_out, "remove '{}'", path.to_str_lossy())?;
            }
            if !args.dry_run {
                let index = repo.index_mut()?;
                index.remove(path.as_ref(), Stage::Normal);
            }
        } else {
            add_single_file(repo, work_tree, &fs_path, args, err_out)?;
        }
    }
    Ok(())
}

fn add_files(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    ignores: &IgnoreStack,
    args: &AddArgs,
    err_out: &mut impl Write,
) -> Result<()> {
    for file in &args.files {
        let fs_path = work_tree.join(file);
        if !fs_path.exists() {
            bail!("pathspec '{}' did not match any files", file);
        }

        if fs_path.is_dir() {
            add_directory_recursive(repo, work_tree, &fs_path, ignores, args, err_out)?;
        } else {
            let rel = pathdiff(work_tree, &fs_path);
            if !args.force && ignores.is_ignored(rel.as_ref(), false) {
                writeln!(
                    err_out,
                    "The following paths are ignored by one of your .gitignore files:\n{}",
                    rel.to_str_lossy()
                )?;
                writeln!(err_out, "hint: Use -f if you really want to add them.")?;
                return Ok(());
            }
            add_single_file(repo, work_tree, &fs_path, args, err_out)?;
        }
    }
    Ok(())
}

fn add_directory_recursive(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    dir: &Path,
    ignores: &IgnoreStack,
    args: &AddArgs,
    err_out: &mut impl Write,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip .git directory
        if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            continue;
        }

        let rel = pathdiff(work_tree, &path);
        let is_dir = path.is_dir();

        if !args.force && ignores.is_ignored(rel.as_ref(), is_dir) {
            continue;
        }

        if is_dir {
            add_directory_recursive(repo, work_tree, &path, ignores, args, err_out)?;
        } else {
            add_single_file(repo, work_tree, &path, args, err_out)?;
        }
    }
    Ok(())
}

fn add_single_file(
    repo: &mut git_repository::Repository,
    work_tree: &Path,
    fs_path: &Path,
    args: &AddArgs,
    err_out: &mut impl Write,
) -> Result<()> {
    let rel_path = pathdiff(work_tree, fs_path);
    let metadata = std::fs::symlink_metadata(fs_path)?;
    let content = std::fs::read(fs_path)?;

    let mode = file_mode_from_metadata(&metadata);

    // Check if file has changed compared to index
    let needs_update = {
        let index = repo.index()?;
        match index.get(rel_path.as_ref(), Stage::Normal) {
            Some(entry) => !entry.stat.matches(&metadata),
            None => true, // new file
        }
    };

    if !needs_update {
        return Ok(());
    }

    if args.verbose {
        writeln!(err_out, "add '{}'", rel_path.to_str_lossy())?;
    }

    if args.dry_run {
        return Ok(());
    }

    // Write blob to ODB
    let oid = repo.odb().write_raw(ObjectType::Blob, &content)?;

    let entry = IndexEntry {
        path: rel_path,
        oid,
        mode,
        stage: Stage::Normal,
        stat: StatData::from_metadata(&metadata),
        flags: EntryFlags::default(),
    };

    let index = repo.index_mut()?;
    index.add(entry);

    Ok(())
}

fn file_mode_from_metadata(meta: &std::fs::Metadata) -> FileMode {
    if meta.is_symlink() {
        FileMode::Symlink
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 != 0 {
                return FileMode::Executable;
            }
        }
        FileMode::Regular
    }
}

fn pathdiff(base: &Path, path: &Path) -> BString {
    let rel = path.strip_prefix(base).unwrap_or(path);
    BString::from(rel.to_str().unwrap_or("").as_bytes())
}

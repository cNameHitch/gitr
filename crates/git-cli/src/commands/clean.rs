use std::io::{self, Write};
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

    /// Interactive clean (stub)
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

    if !args.force && !args.dry_run {
        bail!("fatal: clean.requireForce defaults to true and neither -n nor -f given; refusing to clean");
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

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_utils::path::quote_path;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct LsFilesArgs {
    /// Show staged entries (mode, oid, stage, path)
    #[arg(short = 's', long)]
    stage: bool,

    /// Show cached (tracked) files (default)
    #[arg(long)]
    cached: bool,

    /// Show deleted files
    #[arg(short = 'd', long)]
    deleted: bool,

    /// Show modified files
    #[arg(short = 'm', long)]
    modified: bool,

    /// Show untracked (other) files
    #[arg(short = 'o', long)]
    others: bool,

    /// Show unmerged files
    #[arg(short = 'u', long)]
    unmerged: bool,

    /// Use NUL as line terminator
    #[arg(short = 'z')]
    nul_terminated: bool,

    /// Files to list (default: all)
    #[arg(value_name = "file")]
    files: Vec<String>,
}

pub fn run(args: &LsFilesArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    // Capture work_tree before mutable borrow
    let work_tree: Option<PathBuf> = repo.work_tree().map(|p| p.to_path_buf());

    let index = repo.index()?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let terminator = if args.nul_terminated { '\0' } else { '\n' };

    // Default to --cached if no flags given
    let show_cached = args.cached || (!args.stage && !args.deleted && !args.modified && !args.others && !args.unmerged);

    if args.stage || args.unmerged {
        for entry in index.iter() {
            if args.unmerged && entry.stage == git_index::Stage::Normal {
                continue;
            }
            let mode_val = entry.mode.raw();
            write!(
                out,
                "{:06o} {} {}\t{}{}",
                mode_val,
                entry.oid.to_hex(),
                entry.stage.as_u8(),
                entry.path.as_bstr(),
                terminator,
            )?;
        }
    } else if show_cached {
        for entry in index.iter() {
            if args.nul_terminated {
                // -z: raw bytes, no quoting
                out.write_all(entry.path.as_bytes())?;
                write!(out, "\0")?;
            } else {
                let quoted = quote_path(entry.path.as_bytes());
                writeln!(out, "{}", quoted)?;
            }
        }
    }

    if args.deleted || args.modified {
        if let Some(ref wt) = work_tree {
            for entry in index.iter() {
                let file_path = wt.join(entry.path.to_str_lossy().as_ref());
                if args.deleted && !file_path.exists() {
                    write!(out, "{}{}", entry.path.as_bstr(), terminator)?;
                }
                if args.modified && file_path.exists() {
                    if let Ok(meta) = file_path.metadata() {
                        if !entry.stat.matches(&meta) {
                            write!(out, "{}{}", entry.path.as_bstr(), terminator)?;
                        }
                    }
                }
            }
        }
    }

    // --others: show untracked files
    if args.others {
        if let Some(ref wt) = work_tree {
            list_untracked(wt, index, &mut out, terminator)?;
        }
    }

    Ok(0)
}

fn list_untracked(
    work_tree: &std::path::Path,
    index: &git_index::Index,
    out: &mut impl Write,
    terminator: char,
) -> Result<()> {
    list_untracked_dir(work_tree, work_tree, index, out, terminator)
}

fn list_untracked_dir(
    root: &std::path::Path,
    dir: &std::path::Path,
    index: &git_index::Index,
    out: &mut impl Write,
    terminator: char,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip .git
        if name == ".git" {
            continue;
        }

        if path.is_dir() {
            list_untracked_dir(root, &path, index, out, terminator)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy();
            let rel_bstr = bstr::BStr::new(rel_str.as_bytes());
            if index.get(rel_bstr, git_index::Stage::Normal).is_none() {
                write!(out, "{}{}", rel_str, terminator)?;
            }
        }
    }

    Ok(())
}

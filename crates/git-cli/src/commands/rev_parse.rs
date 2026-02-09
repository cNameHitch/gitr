use std::io::{self, Write};

use anyhow::{bail, Result};
use clap::Args;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RevParseArgs {
    /// Show the .git directory
    #[arg(long)]
    git_dir: bool,

    /// Show the top-level directory of the working tree
    #[arg(long)]
    show_toplevel: bool,

    /// Show the absolute path of the root of the working tree
    #[arg(long)]
    show_cdup: bool,

    /// Show the prefix path from the top-level of the working tree
    #[arg(long)]
    show_prefix: bool,

    /// Check if inside a git work tree
    #[arg(long)]
    is_inside_work_tree: bool,

    /// Check if the repo is bare
    #[arg(long)]
    is_bare_repository: bool,

    /// Verify that the argument is a valid object name
    #[arg(long)]
    verify: bool,

    /// Only output, no error messages
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Show abbreviated ref name
    #[arg(long)]
    abbrev_ref: bool,

    /// Resolve abbreviations
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "7")]
    short: Option<usize>,

    /// Revision or option arguments
    #[arg(value_name = "arg")]
    args: Vec<String>,
}

pub fn run(args: &RevParseArgs, cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Info-only queries that need a repo
    if args.git_dir || args.show_toplevel || args.show_cdup || args.show_prefix
        || args.is_inside_work_tree || args.is_bare_repository
    {
        let repo = open_repo(cli)?;

        if args.git_dir {
            // Match C git: show relative path if possible
            let git_dir = repo.git_dir();
            if let Ok(cwd) = std::env::current_dir() {
                if let Ok(rel) = git_dir.strip_prefix(&cwd) {
                    writeln!(out, "{}", rel.display())?;
                } else {
                    writeln!(out, "{}", git_dir.display())?;
                }
            } else {
                writeln!(out, "{}", git_dir.display())?;
            }
        }
        if args.show_toplevel {
            if let Some(wt) = repo.work_tree() {
                writeln!(out, "{}", wt.display())?;
            }
        }
        if args.show_cdup {
            if let Some(wt) = repo.work_tree() {
                let cwd = std::env::current_dir()?;
                if let Ok(rel) = cwd.strip_prefix(wt) {
                    if rel.as_os_str().is_empty() {
                        writeln!(out)?;
                    } else {
                        // Output "../" for each component
                        let depth = rel.components().count();
                        let cdup: String = (0..depth).map(|_| "../").collect();
                        writeln!(out, "{cdup}")?;
                    }
                } else {
                    writeln!(out)?;
                }
            }
        }
        if args.show_prefix {
            if let Some(wt) = repo.work_tree() {
                let cwd = std::env::current_dir()?;
                if let Ok(rel) = cwd.strip_prefix(wt) {
                    if rel.as_os_str().is_empty() {
                        writeln!(out)?;
                    } else {
                        writeln!(out, "{}/", rel.display())?;
                    }
                } else {
                    writeln!(out)?;
                }
            }
        }
        if args.is_inside_work_tree {
            writeln!(out, "{}", repo.work_tree().is_some())?;
        }
        if args.is_bare_repository {
            writeln!(out, "{}", repo.is_bare())?;
        }

        // If only info flags were given, return
        if args.args.is_empty() {
            return Ok(0);
        }
    }

    // Process revision arguments
    if args.args.is_empty() && !args.git_dir && !args.show_toplevel && !args.show_cdup
        && !args.show_prefix && !args.is_inside_work_tree && !args.is_bare_repository
    {
        return Ok(0);
    }

    let repo = open_repo(cli)?;

    for arg in &args.args {
        if args.abbrev_ref {
            // For HEAD, show the branch name instead of OID
            if arg == "HEAD" {
                if let Ok(Some(branch)) = repo.current_branch() {
                    writeln!(out, "{}", branch)?;
                    continue;
                }
            }
            // For refs/heads/X, show X
            let short = arg
                .strip_prefix("refs/heads/")
                .or_else(|| arg.strip_prefix("refs/remotes/"))
                .or_else(|| arg.strip_prefix("refs/tags/"))
                .unwrap_or(arg);
            writeln!(out, "{}", short)?;
            continue;
        }

        if args.verify {
            match git_revwalk::resolve_revision(&repo, arg) {
                Ok(oid) => {
                    let hex = oid.to_hex();
                    let output = if let Some(len) = args.short {
                        hex[..len.min(hex.len())].to_string()
                    } else {
                        hex
                    };
                    writeln!(out, "{}", output)?;
                }
                Err(_) => {
                    if !args.quiet {
                        eprintln!("fatal: Needed a single revision");
                    }
                    return Ok(128);
                }
            }
        } else {
            match git_revwalk::resolve_revision(&repo, arg) {
                Ok(oid) => {
                    let hex = oid.to_hex();
                    let output = if let Some(len) = args.short {
                        hex[..len.min(hex.len())].to_string()
                    } else {
                        hex
                    };
                    writeln!(out, "{}", output)?;
                }
                Err(e) => {
                    if !args.quiet {
                        bail!("ambiguous argument '{}': {}", arg, e);
                    }
                    return Ok(128);
                }
            }
        }
    }

    Ok(0)
}

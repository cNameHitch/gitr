use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::{Args, Subcommand};
use git_ref::reflog::read_reflog;
use git_ref::RefName;
use git_utils::date::DateFormat;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct ReflogArgs {
    #[command(subcommand)]
    command: Option<ReflogSubcommand>,
}

#[derive(Subcommand)]
pub enum ReflogSubcommand {
    /// Show reflog entries
    Show {
        /// Ref name (defaults to HEAD)
        ref_name: Option<String>,

        /// Date format for reflog entries
        #[arg(long)]
        date: Option<String>,
    },
    /// Expire old reflog entries
    Expire {
        /// Expire entries older than this time
        #[arg(long)]
        expire: Option<String>,

        /// Expire unreachable entries older than this time
        #[arg(long)]
        expire_unreachable: Option<String>,

        /// Process reflogs for all refs
        #[arg(long)]
        all: bool,
    },
    /// Delete a specific reflog entry
    Delete {
        /// Reflog entry to delete (e.g., HEAD@{2})
        ref_entry: String,
    },
    /// Check whether a ref has a reflog
    Exists {
        /// Ref name to check
        ref_name: String,
    },
}

pub fn run(args: &ReflogArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match &args.command {
        None => {
            // Default: show HEAD reflog
            reflog_show_ref(&repo, "HEAD", &mut out)
        }
        Some(ReflogSubcommand::Show { ref_name, date: _ }) => {
            let ref_str = ref_name.as_deref().unwrap_or("HEAD");
            reflog_show_ref(&repo, ref_str, &mut out)
        }
        Some(ReflogSubcommand::Expire { .. }) => {
            reflog_expire(&repo, &mut out)
        }
        Some(ReflogSubcommand::Delete { ref_entry: _ }) => {
            reflog_delete(&repo, &mut out)
        }
        Some(ReflogSubcommand::Exists { ref_name }) => {
            reflog_exists(&repo, ref_name)
        }
    }
}

fn reflog_show_ref(
    repo: &git_repository::Repository,
    ref_str: &str,
    out: &mut impl Write,
) -> Result<i32> {
    let ref_name = resolve_reflog_name(ref_str)?;
    let entries = read_reflog(repo.git_dir(), &ref_name)?;

    let display_name = if ref_str == "HEAD" {
        "HEAD".to_string()
    } else {
        ref_str.to_string()
    };

    for (i, entry) in entries.iter().enumerate() {
        let hex = entry.new_oid.to_hex();
        let short = &hex[..7.min(hex.len())];
        let _date = entry.identity.date.format(&DateFormat::Relative);
        let message = String::from_utf8_lossy(&entry.message);

        writeln!(
            out,
            "{} {}@{{{}}}: {}",
            short, display_name, i, message
        )?;
    }

    Ok(0)
}

fn reflog_expire(
    _repo: &git_repository::Repository,
    out: &mut impl Write,
) -> Result<i32> {
    // Expire old reflog entries (simplified: not implemented yet)
    writeln!(out, "reflog expire: not yet implemented")?;
    Ok(0)
}

fn reflog_delete(
    _repo: &git_repository::Repository,
    out: &mut impl Write,
) -> Result<i32> {
    // Delete specific reflog entries (simplified: not implemented yet)
    writeln!(out, "reflog delete: not yet implemented")?;
    Ok(0)
}

fn reflog_exists(
    repo: &git_repository::Repository,
    ref_name: &str,
) -> Result<i32> {
    let rn = resolve_reflog_name(ref_name)?;
    let reflog_path = repo.git_dir().join("logs").join(rn.as_str());
    if reflog_path.exists() {
        Ok(0)
    } else {
        Ok(1)
    }
}

fn resolve_reflog_name(name: &str) -> Result<RefName> {
    if name == "HEAD" {
        Ok(RefName::new(BString::from("HEAD"))?)
    } else if name.starts_with("refs/") {
        Ok(RefName::new(BString::from(name))?)
    } else {
        // Try as branch name
        Ok(RefName::new(BString::from(format!("refs/heads/{}", name)))?)
    }
}

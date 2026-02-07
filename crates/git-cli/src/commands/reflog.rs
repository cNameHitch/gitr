use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_ref::reflog::read_reflog;
use git_ref::RefName;
use git_utils::date::DateFormat;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct ReflogArgs {
    /// Subcommand (show, expire, delete). Default: show
    subcommand: Option<String>,

    /// Ref name (defaults to HEAD)
    #[arg(long, value_name = "ref")]
    ref_name: Option<String>,

    /// Additional arguments
    args: Vec<String>,
}

pub fn run(args: &ReflogArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let subcmd = args.subcommand.as_deref().unwrap_or("show");

    match subcmd {
        "show" => reflog_show(&repo, args, &mut out),
        "expire" => reflog_expire(&repo, args, &mut out),
        "delete" => reflog_delete(&repo, args, &mut out),
        // If the "subcommand" is actually a ref name, treat as show
        other => {
            let ref_name = other;
            reflog_show_ref(&repo, ref_name, &mut out)
        }
    }
}

fn reflog_show(
    repo: &git_repository::Repository,
    args: &ReflogArgs,
    out: &mut impl Write,
) -> Result<i32> {
    let ref_str = if !args.args.is_empty() {
        args.args[0].as_str()
    } else if let Some(ref name) = args.ref_name {
        name.as_str()
    } else {
        "HEAD"
    };

    reflog_show_ref(repo, ref_str, out)
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
        let _date = entry.identity.date.format(DateFormat::Relative);
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
    _args: &ReflogArgs,
    out: &mut impl Write,
) -> Result<i32> {
    // Expire old reflog entries (simplified: not implemented yet)
    writeln!(out, "reflog expire: not yet implemented")?;
    Ok(0)
}

fn reflog_delete(
    _repo: &git_repository::Repository,
    _args: &ReflogArgs,
    out: &mut impl Write,
) -> Result<i32> {
    // Delete specific reflog entries (simplified: not implemented yet)
    writeln!(out, "reflog delete: not yet implemented")?;
    Ok(0)
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

use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::{Args, Subcommand};
use git_ref::reflog::{read_reflog, expire_reflog, delete_reflog_entry};
use git_ref::{RefName, RefStore};
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
        Some(ReflogSubcommand::Expire { expire, expire_unreachable, all }) => {
            reflog_expire_cmd(&repo, expire.as_deref(), expire_unreachable.as_deref(), *all)
        }
        Some(ReflogSubcommand::Delete { ref_entry }) => {
            reflog_delete_cmd(&repo, ref_entry)
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

fn parse_expire_time(spec: &str) -> Result<i64> {
    if spec == "now" {
        return Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64);
    }

    // Handle relative times like "90.days.ago", "30.days.ago", "2.weeks.ago"
    let parts: Vec<&str> = spec.split('.').collect();
    if parts.len() == 3 && parts[2] == "ago" {
        let amount: u64 = parts[0]
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid expire time: {}", spec))?;
        let seconds = match parts[1] {
            "second" | "seconds" => amount,
            "minute" | "minutes" => amount * 60,
            "hour" | "hours" => amount * 3600,
            "day" | "days" => amount * 86400,
            "week" | "weeks" => amount * 604800,
            "month" | "months" => amount * 2592000, // 30 days
            "year" | "years" => amount * 31536000,  // 365 days
            _ => return Err(anyhow::anyhow!("unknown time unit in: {}", spec)),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        return Ok((now - seconds) as i64);
    }

    // Try parsing as a unix timestamp
    spec.parse::<i64>()
        .map_err(|_| anyhow::anyhow!("invalid expire time: {}", spec))
}

fn reflog_expire_cmd(
    repo: &git_repository::Repository,
    expire: Option<&str>,
    _expire_unreachable: Option<&str>,
    all: bool,
) -> Result<i32> {
    // Default expire time: 90 days ago
    let expire_timestamp = if let Some(spec) = expire {
        parse_expire_time(spec)?
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        (now - 90 * 86400) as i64
    };

    if all {
        // Process all refs
        let refs_iter = repo.refs().iter(Some("refs/"))?;
        for ref_result in refs_iter {
            let reference: git_ref::Reference = ref_result?;
            let name = reference.name().clone();
            let _ = expire_reflog(repo.git_dir(), &name, expire_timestamp);
        }
        // Also process HEAD
        let head = RefName::new("HEAD")?;
        let _ = expire_reflog(repo.git_dir(), &head, expire_timestamp);
    } else {
        // Default: just expire HEAD
        let head = RefName::new("HEAD")?;
        expire_reflog(repo.git_dir(), &head, expire_timestamp)?;
    }

    Ok(0)
}

fn reflog_delete_cmd(
    repo: &git_repository::Repository,
    ref_entry: &str,
) -> Result<i32> {
    // Parse format like "HEAD@{3}" or "refs/heads/main@{1}"
    let at_pos = ref_entry.find("@{")
        .ok_or_else(|| anyhow::anyhow!("invalid reflog entry format: {}, expected REF@{{N}}", ref_entry))?;
    let ref_str = &ref_entry[..at_pos];
    let index_str = &ref_entry[at_pos + 2..];
    let index_str = index_str.trim_end_matches('}');
    let index: usize = index_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid reflog index: {}", index_str))?;

    let ref_name = resolve_reflog_name(ref_str)?;
    delete_reflog_entry(repo.git_dir(), &ref_name, index)?;
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

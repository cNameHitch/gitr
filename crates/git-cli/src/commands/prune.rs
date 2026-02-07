use std::collections::HashSet;
use std::io::{self, Write};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_ref::RefStore;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct PruneArgs {
    /// Do not remove anything; just report what would be removed
    #[arg(short = 'n', long)]
    pub(crate) dry_run: bool,

    /// Report all removed objects
    #[arg(short, long)]
    pub(crate) verbose: bool,

    /// Show progress
    #[arg(long)]
    pub(crate) progress: bool,

    /// Only expire loose objects older than <time>
    #[arg(long, default_value = "2.weeks.ago")]
    pub(crate) expire: String,

    /// Heads to keep (additional tips besides refs)
    #[arg(trailing_var_arg = true)]
    pub(crate) heads: Vec<String>,
}

/// Parse an expire time string into a SystemTime threshold.
/// Supports: "now", "2.weeks.ago", "N.days.ago", "N.hours.ago", epoch timestamps.
fn parse_expire_time(expire: &str) -> Result<SystemTime> {
    let expire = expire.trim();

    if expire == "now" {
        return Ok(SystemTime::now());
    }

    // Parse "N.unit.ago" format
    let parts: Vec<&str> = expire.split('.').collect();
    if parts.len() == 3 && parts[2] == "ago" {
        let n: u64 = parts[0].parse().map_err(|_| {
            anyhow::anyhow!("invalid expire time: {}", expire)
        })?;
        let secs = match parts[1] {
            "seconds" | "second" => n,
            "minutes" | "minute" => n * 60,
            "hours" | "hour" => n * 3600,
            "days" | "day" => n * 86400,
            "weeks" | "week" => n * 7 * 86400,
            "months" | "month" => n * 30 * 86400,
            "years" | "year" => n * 365 * 86400,
            _ => return Err(anyhow::anyhow!("unknown time unit: {}", parts[1])),
        };
        return Ok(SystemTime::now() - Duration::from_secs(secs));
    }

    // Try parsing as epoch timestamp
    if let Ok(epoch) = expire.parse::<u64>() {
        return Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(epoch));
    }

    Err(anyhow::anyhow!("cannot parse expire time: {}", expire))
}

pub fn run(args: &PruneArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Parse expire threshold
    let expire_threshold = parse_expire_time(&args.expire)?;

    // Collect all ref tips as reachable roots
    let mut tips: Vec<ObjectId> = Vec::new();

    // All refs
    let refs = repo.refs().iter(None)?;
    for r in refs {
        let r = r?;
        let oid = r.peel_to_oid(repo.refs())?;
        tips.push(oid);
    }

    // HEAD
    if let Some(head) = repo.head_oid()? {
        tips.push(head);
    }

    // Additional heads from args
    for spec in &args.heads {
        if let Ok(oid) = git_revwalk::resolve_revision(&repo, spec) {
            tips.push(oid);
        }
    }

    // Find all reachable objects
    let reachable: HashSet<ObjectId> = git_revwalk::list_objects(&repo, &tips, &[], None)?
        .into_iter()
        .collect();

    // Find all loose objects
    let objects_dir = repo.odb().objects_dir();
    let mut pruned_count = 0u32;

    // Walk the fanout directories
    for fanout in 0..=255u8 {
        let hex = format!("{:02x}", fanout);
        let fanout_dir = objects_dir.join(&hex);
        if !fanout_dir.is_dir() {
            continue;
        }

        let entries: Vec<_> = match std::fs::read_dir(&fanout_dir) {
            Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
            Err(_) => continue,
        };

        for entry in entries {
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            // Skip non-object files (temp files, etc.)
            if !filename_str.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }

            let full_hex = format!("{}{}", hex, filename_str);
            let oid = match ObjectId::from_hex(&full_hex) {
                Ok(oid) => oid,
                Err(_) => continue,
            };

            if reachable.contains(&oid) {
                continue;
            }

            // Check object mtime against expire threshold
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if mtime > expire_threshold {
                        continue; // Object is newer than expire threshold, skip
                    }
                }
            }

            // Object is unreachable and old enough — prune it
            if args.dry_run || args.verbose {
                // Get object type for display
                let type_str = match repo.odb().read_header(&oid)? {
                    Some(info) => format!("{}", info.obj_type),
                    None => "unknown".to_string(),
                };
                writeln!(err, "{} {}", oid.to_hex(), type_str)?;
            }

            if !args.dry_run {
                let obj_path = entry.path();
                std::fs::remove_file(&obj_path)?;
                pruned_count += 1;

                // Remove empty fanout dir
                let _ = std::fs::remove_dir(&fanout_dir);
            } else {
                pruned_count += 1;
            }
        }
    }

    // Clean up temporary files in pack directory
    let pack_dir = objects_dir.join("pack");
    if pack_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&pack_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("tmp_") || name_str.ends_with(".tmp") {
                    if args.verbose {
                        writeln!(err, "Removing stale temporary file {}", name_str)?;
                    }
                    if !args.dry_run {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    if (args.verbose || args.dry_run) && pruned_count > 0 {
        writeln!(
            err,
            "{}pruned {} unreachable object{}",
            if args.dry_run { "Would have " } else { "" },
            pruned_count,
            if pruned_count == 1 { "" } else { "s" }
        )?;
    }

    Ok(0)
}

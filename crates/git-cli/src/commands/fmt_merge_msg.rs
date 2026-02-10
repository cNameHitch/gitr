use std::io::{self, BufRead, Write};

use anyhow::Result;
use clap::Args;

use crate::Cli;

#[derive(Args)]
pub struct FmtMergeMsgArgs {
    /// Add log message with at most <n> one-line descriptions (default 20)
    #[arg(long, num_args = 0..=1, default_missing_value = "20")]
    log: Option<u32>,

    /// Suppress log message
    #[arg(long)]
    no_log: bool,

    /// Prepend the given message to the merge message
    #[arg(short = 'm', long = "message")]
    message: Option<String>,

    /// Read message from file instead of generating
    #[arg(short = 'F', long = "file")]
    file: Option<String>,
}

pub fn run(args: &FmtMergeMsgArgs, _cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // If -F is given, just output the file contents
    if let Some(ref path) = args.file {
        let content = std::fs::read_to_string(path)?;
        write!(out, "{}", content)?;
        return Ok(0);
    }

    // Read FETCH_HEAD-like input from stdin
    // Lines look like: <oid>\t\tbranch '<name>' of <url>
    //                   <oid>\tnot-for-merge\tbranch '<name>' of <url>
    let stdin = io::stdin();
    let mut branches: Vec<String> = Vec::new();
    let mut descriptions: Vec<(String, String)> = Vec::new(); // (branch_name, oid)

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        // Parse FETCH_HEAD format
        // Format: <oid>\t[not-for-merge\t]branch '<name>' of <url>
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let oid = parts[0].to_string();
        let rest = parts[1];

        // Skip "not-for-merge" entries (they start with "not-for-merge\t")
        let desc_part = if let Some(stripped) = rest.strip_prefix("not-for-merge\t") {
            stripped
        } else if rest.starts_with('\t') {
            // Double-tab: merge candidate
            rest.trim_start_matches('\t')
        } else {
            rest
        };

        // Extract branch name from patterns like:
        //   branch 'name' of url
        //   tag 'name' of url
        if let Some(branch_name) = extract_branch_name(desc_part) {
            branches.push(branch_name.clone());
            descriptions.push((branch_name, oid));
        }
    }

    if branches.is_empty() {
        return Ok(0);
    }

    // Build the merge message
    let mut msg = String::new();

    // Prepend -m message if given
    if let Some(ref prepend) = args.message {
        msg.push_str(prepend);
        msg.push('\n');
        msg.push('\n');
    }

    // Format: "Merge branch 'X'" or "Merge branches 'X', 'Y' and 'Z'"
    match branches.len() {
        1 => {
            msg.push_str(&format!("Merge branch '{}'", branches[0]));
        }
        _ => {
            msg.push_str("Merge branches ");
            for (i, name) in branches.iter().enumerate() {
                if i == branches.len() - 1 {
                    msg.push_str(&format!(" and '{}'", name));
                } else if i == 0 {
                    msg.push_str(&format!("'{}'", name));
                } else {
                    msg.push_str(&format!(", '{}'", name));
                }
            }
        }
    }
    msg.push('\n');

    // With --log (and not --no-log), append one-line descriptions
    let include_log = args.log.is_some() && !args.no_log;
    if include_log {
        let max_entries = args.log.unwrap_or(20) as usize;
        if !descriptions.is_empty() {
            msg.push('\n');
            let count = descriptions.len().min(max_entries);
            for (branch_name, oid) in descriptions.iter().take(count) {
                let short_oid = if oid.len() >= 7 { &oid[..7] } else { oid };
                msg.push_str(&format!("  {} {}\n", short_oid, branch_name));
            }
        }
    }

    write!(out, "{}", msg)?;

    Ok(0)
}

/// Extract a branch or tag name from a FETCH_HEAD description.
///
/// Patterns:
///   "branch 'name' of url" -> Some("name")
///   "tag 'name' of url"    -> Some("name")
fn extract_branch_name(desc: &str) -> Option<String> {
    // Try "branch '<name>'" pattern
    if let Some(rest) = desc.strip_prefix("branch '") {
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }

    // Try "tag '<name>'" pattern
    if let Some(rest) = desc.strip_prefix("tag '") {
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }

    None
}

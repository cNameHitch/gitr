use std::io::{self, BufRead, Write};
use std::process::Command;

use anyhow::{bail, Result};
use clap::Args;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct LsRemoteArgs {
    /// Show only refs/heads
    #[arg(long)]
    heads: bool,

    /// Show only refs/tags
    #[arg(long)]
    tags: bool,

    /// Do not show peeled tags
    #[arg(long)]
    refs: bool,

    /// Suppress informational messages
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Exit with status 2 when no matching refs are found
    #[arg(long)]
    exit_code: bool,

    /// Sort refs by key (e.g., version:refname)
    #[arg(long, value_name = "key")]
    sort: Option<String>,

    /// Show the URL of the remote instead of refs
    #[arg(long)]
    get_url: bool,

    /// Upload pack program on remote end (advanced)
    #[arg(long, value_name = "exec")]
    upload_pack: Option<String>,

    /// Show only the OID column (with no ref name)
    #[arg(long)]
    symref: bool,

    /// Repository (remote name or URL)
    #[arg(value_name = "repository")]
    repository: Option<String>,

    /// Ref patterns to match
    #[arg(value_name = "patterns")]
    patterns: Vec<String>,
}

pub fn run(args: &LsRemoteArgs, cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Determine the remote URL
    let remote_url = resolve_remote_url(args, cli)?;

    // Handle --get-url: just print the URL and exit
    if args.get_url {
        writeln!(out, "{}", remote_url)?;
        return Ok(0);
    }

    // Delegate to git ls-remote for the actual transport
    let mut cmd = Command::new("git");
    cmd.arg("ls-remote");

    if args.heads {
        cmd.arg("--heads");
    }
    if args.tags {
        cmd.arg("--tags");
    }
    if args.refs {
        cmd.arg("--refs");
    }
    if args.quiet {
        cmd.arg("--quiet");
    }
    if args.symref {
        cmd.arg("--symref");
    }
    if let Some(ref sort_key) = args.sort {
        cmd.arg(format!("--sort={}", sort_key));
    }
    if let Some(ref upload_pack) = args.upload_pack {
        cmd.arg(format!("--upload-pack={}", upload_pack));
    }

    cmd.arg(&remote_url);

    // Add pattern arguments
    for pattern in &args.patterns {
        cmd.arg(pattern);
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            eprint!("{}", stderr);
        }
        let code = output.status.code().unwrap_or(128);
        return Ok(code);
    }

    // Parse and optionally filter the output
    let result_lines = output.stdout.lines().collect::<std::result::Result<Vec<_>, _>>()?;

    if result_lines.is_empty() && args.exit_code {
        return Ok(2);
    }

    // Filter by ref type if needed (git ls-remote already does this, but
    // we also support client-side pattern filtering)
    let filtered = filter_refs(&result_lines, args);

    if filtered.is_empty() && args.exit_code {
        return Ok(2);
    }

    for line in &filtered {
        writeln!(out, "{}", line)?;
    }

    Ok(0)
}

/// Resolve the remote URL from args or repo config.
fn resolve_remote_url(args: &LsRemoteArgs, cli: &Cli) -> Result<String> {
    if let Some(ref repo_arg) = args.repository {
        // Check if it looks like a URL already
        if repo_arg.contains("://")
            || repo_arg.contains('@')
            || repo_arg.starts_with('/')
            || repo_arg.ends_with(".git")
        {
            return Ok(repo_arg.clone());
        }

        // Try to resolve as a remote name from config
        if let Ok(repo) = open_repo(cli) {
            if let Some(url) = get_remote_url(&repo, repo_arg) {
                return Ok(url);
            }
        }

        // Fall back to using it as-is (could be a relative path or short URL)
        Ok(repo_arg.clone())
    } else {
        // Default: use "origin" remote from config
        let repo = open_repo(cli)?;
        if let Some(url) = get_remote_url(&repo, "origin") {
            Ok(url)
        } else {
            bail!("fatal: No remote configured and no repository specified");
        }
    }
}

/// Read a remote's URL from the repository config.
fn get_remote_url(repo: &git_repository::Repository, remote_name: &str) -> Option<String> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).ok()?;

    let section_header = format!("[remote \"{}\"]", remote_name);
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with('[') {
            break;
        }
        if in_section {
            if let Some(url) = trimmed.strip_prefix("url = ") {
                return Some(url.to_string());
            }
        }
    }

    None
}

/// Filter ref lines based on args patterns (client-side filtering).
fn filter_refs(lines: &[String], args: &LsRemoteArgs) -> Vec<String> {
    if args.patterns.is_empty() {
        return lines.to_vec();
    }

    lines
        .iter()
        .filter(|line| {
            // Each line is "OID\trefname" or "ref: sym\trefname\tOID\trefname"
            let ref_name = line.split('\t').nth(1).unwrap_or("");
            args.patterns.iter().any(|pattern| {
                ref_matches_pattern(ref_name, pattern)
            })
        })
        .cloned()
        .collect()
}

/// Check if a ref name matches a pattern (simple glob matching).
fn ref_matches_pattern(ref_name: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        // Simple glob: convert to prefix/suffix match
        let parts: Vec<&str> = pattern.splitn(2, '*').collect();
        if parts.len() == 2 {
            ref_name.starts_with(parts[0]) && ref_name.ends_with(parts[1])
        } else {
            ref_name == pattern
        }
    } else {
        // Exact match or suffix match
        ref_name == pattern || ref_name.ends_with(&format!("/{}", pattern))
    }
}

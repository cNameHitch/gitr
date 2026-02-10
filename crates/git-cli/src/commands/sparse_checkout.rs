use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::{Args, Subcommand};
use git_index::sparse::SparseCheckout;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct SparseCheckoutArgs {
    #[command(subcommand)]
    command: SparseCheckoutCommand,
}

#[derive(Subcommand)]
pub enum SparseCheckoutCommand {
    /// Initialize sparse checkout
    Init {
        /// Use cone mode (default)
        #[arg(long)]
        cone: bool,

        /// Use non-cone mode (full pattern matching)
        #[arg(long)]
        no_cone: bool,
    },
    /// Set the sparse checkout patterns (replaces existing)
    Set {
        /// Patterns to include
        patterns: Vec<String>,

        /// Use cone mode (default)
        #[arg(long)]
        cone: bool,

        /// Use non-cone mode
        #[arg(long)]
        no_cone: bool,

        /// Read patterns from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Add patterns to sparse checkout
    Add {
        /// Patterns to add
        patterns: Vec<String>,

        /// Use cone mode (default)
        #[arg(long)]
        cone: bool,

        /// Use non-cone mode
        #[arg(long)]
        no_cone: bool,
    },
    /// Reapply the sparse checkout rules to the working tree
    Reapply,
    /// Disable sparse checkout
    Disable,
    /// List the current sparse checkout patterns
    List,
}

pub fn run(args: &SparseCheckoutArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        SparseCheckoutCommand::Init { cone, no_cone } => run_init(cli, *cone, *no_cone),
        SparseCheckoutCommand::Set { patterns, cone, no_cone, stdin } => {
            run_set(cli, patterns, *cone, *no_cone, *stdin)
        }
        SparseCheckoutCommand::Add { patterns, cone, no_cone } => {
            run_add(cli, patterns, *cone, *no_cone)
        }
        SparseCheckoutCommand::Reapply => run_reapply(cli),
        SparseCheckoutCommand::Disable => run_disable(cli),
        SparseCheckoutCommand::List => run_list(cli),
    }
}

fn run_init(cli: &Cli, _cone: bool, no_cone: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    // Set core.sparseCheckout = true in config
    set_config_value(&git_dir, "core.sparseCheckout", "true")?;

    // Set core.sparseCheckoutCone if applicable
    if !no_cone {
        set_config_value(&git_dir, "core.sparseCheckoutCone", "true")?;
    }

    // Create the sparse-checkout file with default patterns
    let info_dir = git_dir.join("info");
    std::fs::create_dir_all(&info_dir)?;
    let sparse_path = info_dir.join("sparse-checkout");

    if !sparse_path.exists() {
        // Default: include everything at root level
        std::fs::write(&sparse_path, "/*\n!/*/\n")?;
    }

    Ok(0)
}

fn run_set(
    cli: &Cli,
    patterns: &[String],
    _cone: bool,
    _no_cone: bool,
    stdin: bool,
) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    // Ensure sparse checkout is enabled
    set_config_value(&git_dir, "core.sparseCheckout", "true")?;

    let mut all_patterns: Vec<String> = patterns.to_vec();

    // Read patterns from stdin if requested
    if stdin {
        let stdin_handle = io::stdin();
        let mut line = String::new();
        while stdin_handle.read_line(&mut line)? > 0 {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                all_patterns.push(trimmed);
            }
            line.clear();
        }
    }

    if all_patterns.is_empty() {
        bail!("error: no patterns specified");
    }

    // Build sparse checkout structure
    let mut sc = SparseCheckout::new();
    sc.enabled = true;
    for pattern in &all_patterns {
        if let Some(stripped) = pattern.strip_prefix('!') {
            sc.exclude_patterns.push(BString::from(stripped.as_bytes()));
        } else {
            sc.include_patterns.push(BString::from(pattern.as_bytes()));
        }
    }

    sc.save(&git_dir)?;

    Ok(0)
}

fn run_add(cli: &Cli, patterns: &[String], _cone: bool, _no_cone: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    if patterns.is_empty() {
        bail!("error: no patterns specified");
    }

    // Ensure sparse checkout is enabled
    set_config_value(&git_dir, "core.sparseCheckout", "true")?;

    // Load existing patterns
    let mut sc = SparseCheckout::from_file(&git_dir)
        .unwrap_or_else(|_| SparseCheckout::new());
    sc.enabled = true;

    // Append new patterns
    for pattern in patterns {
        if let Some(stripped) = pattern.strip_prefix('!') {
            sc.exclude_patterns.push(BString::from(stripped.as_bytes()));
        } else {
            sc.include_patterns.push(BString::from(pattern.as_bytes()));
        }
    }

    sc.save(&git_dir)?;

    Ok(0)
}

fn run_reapply(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    let sc = SparseCheckout::from_file(&git_dir)?;
    if !sc.enabled {
        eprintln!("warning: sparse checkout is not enabled");
        return Ok(0);
    }

    // Re-read patterns (they are already loaded from file)
    // In a full implementation, this would update the skip-worktree bits
    // on the index and check out / remove files accordingly.
    eprintln!(
        "Reapplying sparse checkout with {} include pattern(s)",
        sc.include_patterns.len()
    );

    Ok(0)
}

fn run_disable(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    // Set core.sparseCheckout = false
    set_config_value(&git_dir, "core.sparseCheckout", "false")?;

    // Remove the sparse-checkout file
    let sparse_path = git_dir.join("info").join("sparse-checkout");
    if sparse_path.exists() {
        std::fs::remove_file(&sparse_path)?;
    }

    // Remove cone mode config
    remove_config_value(&git_dir, "core.sparseCheckoutCone")?;

    Ok(0)
}

fn run_list(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let sc = SparseCheckout::from_file(&git_dir)?;

    for pattern in &sc.include_patterns {
        writeln!(out, "{}", pattern)?;
    }
    for pattern in &sc.exclude_patterns {
        writeln!(out, "!{}", pattern)?;
    }

    Ok(0)
}

/// Set a config value in the repo-level config file.
fn set_config_value(git_dir: &std::path::Path, key: &str, value: &str) -> Result<()> {
    let config_path = git_dir.join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    // Parse the key into section and name
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() != 2 {
        bail!("invalid config key: {}", key);
    }
    let section = parts[0];
    let name = parts[1];

    let section_header = format!("[{}]", section);
    let key_line = format!("\t{} = {}", name, value);

    // Check if section exists and key exists within it
    let mut new_content = String::new();
    let mut in_section = false;
    let mut key_replaced = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case(&section_header) {
            in_section = true;
            new_content.push_str(line);
            new_content.push('\n');
            continue;
        }

        if in_section && trimmed.starts_with('[') {
            if !key_replaced {
                // Key didn't exist in section, add it before next section
                new_content.push_str(&key_line);
                new_content.push('\n');
                key_replaced = true;
            }
            in_section = false;
        }

        if in_section
            && trimmed
                .split('=')
                .next()
                .map(|k| k.trim().eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            // Replace existing key
            new_content.push_str(&key_line);
            new_content.push('\n');
            key_replaced = true;
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    // If we were in the section at EOF and didn't replace yet
    if in_section && !key_replaced {
        new_content.push_str(&key_line);
        new_content.push('\n');
        key_replaced = true;
    }

    // If section doesn't exist at all, append it
    if !key_replaced {
        new_content.push_str(&section_header);
        new_content.push('\n');
        new_content.push_str(&key_line);
        new_content.push('\n');
    }

    std::fs::write(&config_path, new_content)?;
    Ok(())
}

/// Remove a config key from the repo-level config file.
fn remove_config_value(git_dir: &std::path::Path, key: &str) -> Result<()> {
    let config_path = git_dir.join("config");
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Ok(());
    }
    let section = parts[0];
    let name = parts[1];
    let section_header = format!("[{}]", section);

    let mut new_content = String::new();
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case(&section_header) {
            in_section = true;
            new_content.push_str(line);
            new_content.push('\n');
            continue;
        }

        if in_section && trimmed.starts_with('[') {
            in_section = false;
        }

        if in_section
            && trimmed
                .split('=')
                .next()
                .map(|k| k.trim().eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            // Skip this line (remove it)
            continue;
        }

        new_content.push_str(line);
        new_content.push('\n');
    }

    std::fs::write(&config_path, new_content)?;
    Ok(())
}

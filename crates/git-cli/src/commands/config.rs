use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct ConfigArgs {
    /// Get the value for a given key
    #[arg(long)]
    get: bool,

    /// List all config variables
    #[arg(short = 'l', long)]
    list: bool,

    /// Show the origin (file path) of each config entry
    #[arg(long)]
    show_origin: bool,

    /// Use only repository-local config
    #[arg(long)]
    local: bool,

    /// Configuration key (e.g., remote.origin.url)
    key: Option<String>,

    /// Value to set
    value: Option<String>,
}

pub fn run(args: &ConfigArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // If key and value are given, this is a set operation (must come before immutable borrows)
    if let (Some(ref key), Some(ref value)) = (&args.key, &args.value) {
        repo.config_mut().set(key, value, git_config::ConfigScope::Local)?;
        return Ok(0);
    }

    if args.list {
        let cwd = std::env::current_dir().ok();
        let entries = repo.config().all_entries();
        for entry in &entries {
            // Filter by local scope if --local
            if args.local && entry.scope != git_config::ConfigScope::Local {
                continue;
            }
            let key_str = entry.key.to_canonical();
            let value_str = entry
                .value
                .as_ref()
                .map(|v| v.to_str_lossy().to_string())
                .unwrap_or_default();
            if args.show_origin {
                let origin = entry
                    .source_file
                    .as_ref()
                    .map(|p| {
                        let display_path = if let Some(ref cwd) = cwd {
                            p.strip_prefix(cwd).unwrap_or(p)
                        } else {
                            p
                        };
                        format!("file:{}", display_path.display())
                    })
                    .unwrap_or_else(|| "command line:".to_string());
                writeln!(out, "{}\t{}={}", origin, key_str, value_str)?;
            } else {
                writeln!(out, "{}={}", key_str, value_str)?;
            }
        }
        return Ok(0);
    }

    if args.get || args.key.is_some() {
        let key = args.key.as_deref().ok_or_else(|| {
            anyhow::anyhow!("error: key is required for --get")
        })?;

        let cwd = std::env::current_dir().ok();

        if args.local {
            // Only look in local config
            let entries = repo.config().all_entries();
            for entry in entries.iter().rev() {
                if entry.scope == git_config::ConfigScope::Local {
                    let parsed_key = git_config::ConfigKey::parse(key)?;
                    if entry.key.matches(&parsed_key) {
                        let val = entry
                            .value
                            .as_ref()
                            .map(|v| v.to_str_lossy().to_string())
                            .unwrap_or_default();
                        if args.show_origin {
                            let origin = entry
                                .source_file
                                .as_ref()
                                .map(|p| {
                                    let display_path = if let Some(ref cwd) = cwd {
                                        p.strip_prefix(cwd).unwrap_or(p)
                                    } else { p };
                                    format!("file:{}", display_path.display())
                                })
                                .unwrap_or_else(|| "file:.git/config".to_string());
                            writeln!(out, "{}\t{}", origin, val)?;
                        } else {
                            writeln!(out, "{}", val)?;
                        }
                        return Ok(0);
                    }
                }
            }
            return Ok(1);
        }

        if args.show_origin {
            let entries = repo.config().all_entries();
            for entry in entries.iter().rev() {
                let parsed_key = git_config::ConfigKey::parse(key)?;
                if entry.key.matches(&parsed_key) {
                    let val = entry
                        .value
                        .as_ref()
                        .map(|v| v.to_str_lossy().to_string())
                        .unwrap_or_default();
                    let origin = entry
                        .source_file
                        .as_ref()
                        .map(|p| {
                            let display_path = if let Some(ref cwd) = cwd {
                                p.strip_prefix(cwd).unwrap_or(p)
                            } else { p };
                            format!("file:{}", display_path.display())
                        })
                        .unwrap_or_else(|| "command line:".to_string());
                    writeln!(out, "{}\t{}", origin, val)?;
                    return Ok(0);
                }
            }
            return Ok(1);
        }

        match repo.config().get_string(key)? {
            Some(value) => {
                writeln!(out, "{}", value)?;
                Ok(0)
            }
            None => Ok(1),
        }
    } else {
        eprintln!("error: usage: git config [--get] [--list] [--show-origin] [--local] [key] [value]");
        Ok(1)
    }
}

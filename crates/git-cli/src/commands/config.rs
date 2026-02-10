use std::io::{self, Write};
use std::path::PathBuf;

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

    /// Use global config (~/.gitconfig)
    #[arg(long)]
    global: bool,

    /// Remove a configuration entry
    #[arg(long)]
    unset: bool,

    /// Use system-wide config file
    #[arg(long)]
    system: bool,

    /// Use given config file
    #[arg(short = 'f', long = "file", value_name = "config-file")]
    file: Option<PathBuf>,

    /// Get all values for a multi-valued key
    #[arg(long)]
    get_all: bool,

    /// Get values matching a regex
    #[arg(long)]
    get_regexp: bool,

    /// Replace all matching lines for a multi-valued key
    #[arg(long)]
    replace_all: bool,

    /// Add a new value without altering existing ones
    #[arg(long)]
    add: bool,

    /// Remove all matching lines for a multi-valued key
    #[arg(long)]
    unset_all: bool,

    /// Rename a section
    #[arg(long)]
    rename_section: bool,

    /// Remove a section
    #[arg(long)]
    remove_section: bool,

    /// Open config in editor
    #[arg(short = 'e', long)]
    edit: bool,

    /// Ensure value matches a given type
    #[arg(long = "type", value_name = "type")]
    value_type: Option<String>,

    /// Type-check: value is "true" or "false"
    #[arg(long = "bool")]
    bool_type: bool,

    /// Type-check: value is a decimal number
    #[arg(long = "int")]
    int_type: bool,

    /// Type-check: value is a path
    #[arg(long = "path")]
    path_type: bool,

    /// Terminate values with NUL byte
    #[arg(short = 'z')]
    null_terminate: bool,

    /// Show only variable names
    #[arg(long)]
    name_only: bool,

    /// Respect include directives
    #[arg(long)]
    includes: bool,

    /// Configuration key (e.g., remote.origin.url)
    key: Option<String>,

    /// Value to set
    value: Option<String>,
}

pub fn run(args: &ConfigArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Handle --unset
    if args.unset {
        let key = args.key.as_deref().ok_or_else(|| {
            anyhow::anyhow!("error: key is required for --unset")
        })?;
        let scope = if args.global {
            git_config::ConfigScope::Global
        } else {
            git_config::ConfigScope::Local
        };
        repo.config_mut().unset(key, scope)?;
        return Ok(0);
    }

    // If key and value are given, this is a set operation (must come before immutable borrows)
    if let (Some(ref key), Some(ref value)) = (&args.key, &args.value) {
        let scope = if args.global {
            git_config::ConfigScope::Global
        } else {
            git_config::ConfigScope::Local
        };
        repo.config_mut().set(key, value, scope)?;
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

        if args.global {
            // Only look in global config
            match repo.config().get_string_from_scope(key, git_config::ConfigScope::Global) {
                Ok(Some(value)) => {
                    writeln!(out, "{}", value)?;
                    return Ok(0);
                }
                Ok(None) => return Ok(1),
                Err(e) => return Err(e.into()),
            }
        }

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

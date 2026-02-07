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

    /// Configuration key (e.g., remote.origin.url)
    key: Option<String>,
}

pub fn run(args: &ConfigArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.list {
        let entries = repo.config().all_entries();
        for entry in &entries {
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
                    .map(|p| format!("file:{}", p.display()))
                    .unwrap_or_else(|| "command line:".to_string());
                writeln!(out, "{}\t{}={}", origin, key_str, value_str)?;
            } else {
                writeln!(out, "{}={}", key_str, value_str)?;
            }
        }
        return Ok(0);
    }

    if args.get {
        let key = args.key.as_deref().ok_or_else(|| {
            anyhow::anyhow!("error: key is required for --get")
        })?;
        match repo.config().get_string(key)? {
            Some(value) => {
                writeln!(out, "{}", value)?;
                Ok(0)
            }
            None => Ok(1),
        }
    } else if args.key.is_some() {
        // Default behavior with just a key: same as --get
        let key = args.key.as_deref().unwrap();
        match repo.config().get_string(key)? {
            Some(value) => {
                writeln!(out, "{}", value)?;
                Ok(0)
            }
            None => Ok(1),
        }
    } else {
        eprintln!("error: usage: git config [--get] [--list] [--show-origin] [key]");
        Ok(1)
    }
}

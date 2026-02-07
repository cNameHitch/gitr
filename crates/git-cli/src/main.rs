mod commands;

use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::Parser;

use commands::Commands;

#[derive(Parser)]
#[command(name = "gitr", about = "A Git implementation in Rust")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Run as if started in <path>
    #[arg(short = 'C', global = true)]
    directory: Option<PathBuf>,

    /// Set a configuration value (key=value)
    #[arg(short = 'c', global = true)]
    config: Vec<String>,

    /// Set the path to the .git directory
    #[arg(long = "git-dir")]
    git_dir: Option<PathBuf>,
}

/// Preprocess raw args to handle git-style `-<n>` count limiters for format-patch.
/// Transforms e.g. `format-patch -1 HEAD` into `format-patch --max-count 1 HEAD`.
fn preprocess_args() -> Vec<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Vec::with_capacity(args.len());

    // Find if format-patch is the subcommand
    let is_format_patch = args.iter().any(|a| a == "format-patch");

    for arg in args {
        if is_format_patch && arg.starts_with('-') && !arg.starts_with("--") && arg.len() >= 2 {
            // Check if this is -<n> (all digits after the dash)
            let rest = &arg[1..];
            if rest.chars().all(|c| c.is_ascii_digit()) {
                result.push("--max-count".to_string());
                result.push(rest.to_string());
                continue;
            }
        }
        result.push(arg);
    }

    result
}

fn main() {
    let cli = Cli::parse_from(preprocess_args());

    if let Some(dir) = &cli.directory {
        if let Err(e) = std::env::set_current_dir(dir) {
            eprintln!("fatal: cannot change to '{}': {}", dir.display(), e);
            process::exit(128);
        }
    }

    match run(cli) {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("fatal: {e}");
            process::exit(128);
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    commands::run(cli)
}

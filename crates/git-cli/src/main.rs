mod commands;

use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{error::ErrorKind, Parser};

use commands::Commands;

#[derive(Parser)]
#[command(name = "gitr", about = "A Git implementation in Rust", version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Run as if started in <path>
    #[arg(short = 'C', global = true)]
    change_dir: Option<PathBuf>,

    /// Set a configuration value (key=value)
    #[arg(short = 'c', global = true)]
    config: Vec<String>,

    /// Set the path to the .git directory
    #[arg(long = "git-dir")]
    git_dir: Option<PathBuf>,
}

/// Preprocess raw args to handle git-style `-<n>` count limiters.
/// Transforms e.g. `log -3` or `format-patch -1 HEAD` into `--max-count N`.
fn preprocess_args() -> Vec<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Vec::with_capacity(args.len());

    // Commands that support -<n> shorthand for --max-count
    let supports_dash_n = args
        .iter()
        .any(|a| a == "log" || a == "format-patch" || a == "shortlog");

    for arg in args {
        if supports_dash_n && arg.starts_with('-') && !arg.starts_with("--") && arg.len() >= 2 {
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
    let cli = match Cli::try_parse_from(preprocess_args()) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => process::exit(0),
                _ => process::exit(128),
            }
        }
    };

    if let Some(dir) = &cli.change_dir {
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

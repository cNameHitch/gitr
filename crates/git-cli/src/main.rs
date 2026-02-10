mod commands;
pub mod interactive;

use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{error::ErrorKind, Parser};

use commands::Commands;

#[derive(Parser)]
#[command(name = "gitr", about = "A Git implementation in Rust", version = concat!("version ", env!("CARGO_PKG_VERSION")))]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Run as if started in <path>
    #[arg(long = "change-dir")]
    change_dir: Option<PathBuf>,

    /// Set a configuration value (key=value)
    #[arg(long)]
    config: Vec<String>,

    /// Set the path to the .git directory
    #[arg(long = "git-dir")]
    git_dir: Option<PathBuf>,

    /// Set the path to the working tree
    #[arg(long = "work-tree", global = true)]
    work_tree: Option<PathBuf>,

    /// Treat the repository as a bare repository
    #[arg(long = "bare", global = true)]
    bare: bool,

    /// Do not use replacement refs
    #[arg(long = "no-replace-objects", global = true)]
    no_replace_objects: bool,

    /// Namespace for refs
    #[arg(long = "namespace", global = true)]
    namespace: Option<String>,

    /// Force pager usage
    #[arg(long = "paginate", global = true)]
    paginate: bool,

    /// Disable pager
    #[arg(short = 'P', long = "no-pager", global = true)]
    no_pager: bool,
}

/// Preprocess raw args to handle git-style shorthand:
/// 1. `-<n>` count limiters for log/format-patch/shortlog -> `--max-count N`
/// 2. `-C <path>` before subcommand -> `--change-dir <path>`
/// 3. `-c key=value` before subcommand -> `--config key=value`
/// 4. `-p` before subcommand -> `--paginate` (so subcommands can use `-p` for `--patch`)
fn preprocess_args() -> Vec<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Vec::with_capacity(args.len() + 4);

    // Commands that support -<n> shorthand for --max-count
    let supports_dash_n = args
        .iter()
        .any(|a| a == "log" || a == "format-patch" || a == "shortlog");

    // Find where the subcommand starts
    let subcommand_pos = find_subcommand_pos(&args);

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // Before the subcommand, handle -C, -c, and -p
        if i > 0 && i < subcommand_pos {
            if arg == "-C" && i + 1 < args.len() {
                result.push("--change-dir".to_string());
                i += 1;
                result.push(args[i].clone());
                i += 1;
                continue;
            }
            if arg == "-c" && i + 1 < args.len() {
                result.push("--config".to_string());
                i += 1;
                result.push(args[i].clone());
                i += 1;
                continue;
            }
            if arg == "-p" {
                result.push("--paginate".to_string());
                i += 1;
                continue;
            }
        }

        // Handle -<n> shorthand (after subcommand)
        if supports_dash_n && arg.starts_with('-') && !arg.starts_with("--") && arg.len() >= 2 {
            let rest = &arg[1..];
            if rest.chars().all(|c| c.is_ascii_digit()) {
                result.push("--max-count".to_string());
                result.push(rest.to_string());
                i += 1;
                continue;
            }
        }

        result.push(arg.clone());
        i += 1;
    }

    result
}

/// Find the position of the subcommand in the args list.
///
/// The subcommand is the first arg (after the program name) that doesn't
/// start with `-` and isn't a value for a preceding flag.
fn find_subcommand_pos(args: &[String]) -> usize {
    let mut i = 1; // Skip program name
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            return i + 1;
        }
        if !arg.starts_with('-') {
            return i;
        }
        // Skip value for known flags that take a value
        if (arg == "-C"
            || arg == "-c"
            || arg == "--git-dir"
            || arg == "--work-tree"
            || arg == "--namespace"
            || arg == "--change-dir"
            || arg == "--config")
            && i + 1 < args.len()
        {
            i += 2;
            continue;
        }
        i += 1;
    }
    args.len()
}

fn is_auto_paged_command(cmd: &Commands) -> bool {
    matches!(
        cmd,
        Commands::Log(_)
            | Commands::Diff(_)
            | Commands::Show(_)
            | Commands::Blame(_)
            | Commands::Shortlog(_)
            | Commands::Grep(_)
            | Commands::Branch(_)
            | Commands::Tag(_)
    )
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
    // Set up pager if needed
    let should_page = if cli.no_pager {
        false
    } else if cli.paginate {
        true
    } else {
        is_auto_paged_command(&cli.command)
    };

    let _pager_guard = if should_page {
        // Try to read core.pager from config (best-effort)
        let config_pager = load_pager_config(&cli);
        git_utils::pager::setup_pager_for_stdout(config_pager.as_deref())
            .ok()
            .flatten()
    } else {
        None
    };

    commands::run(cli)
}

/// Load pager configuration from git config (best-effort).
///
/// Checks per-command pager config (`pager.<cmd>`) first, then `core.pager`.
fn load_pager_config(cli: &Cli) -> Option<String> {
    let config = if let Some(ref git_dir) = cli.git_dir {
        git_config::ConfigSet::load(Some(git_dir)).ok()
    } else {
        // Try to discover the repo's git dir from the current directory
        git_repository::Repository::discover(".")
            .ok()
            .and_then(|repo| git_config::ConfigSet::load(Some(repo.git_dir())).ok())
    };

    if let Some(ref config) = config {
        let cmd_name = cli.command.command_name();
        let per_cmd_key = format!("pager.{}", cmd_name);
        if let Ok(Some(val)) = config.get_string(&per_cmd_key) {
            return Some(val);
        }
        if let Ok(Some(val)) = config.get_string("core.pager") {
            return Some(val);
        }
    }
    None
}

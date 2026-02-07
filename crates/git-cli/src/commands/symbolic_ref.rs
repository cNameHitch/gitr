use std::io::{self, Write};

use anyhow::Result;
use bstr::{BString, ByteSlice};
use clap::Args;
use git_ref::{RefName, RefStore, Reference};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct SymbolicRefArgs {
    /// Delete the symbolic ref
    #[arg(short = 'd', long)]
    delete: bool,

    /// Be quiet
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Show only the short ref name
    #[arg(long)]
    short: bool,

    /// Reflog message
    #[arg(short = 'm')]
    message: Option<String>,

    /// Name of the symbolic ref (e.g., HEAD)
    name: String,

    /// Target ref (to set), omit to read
    target: Option<String>,
}

pub fn run(args: &SymbolicRefArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let refs = repo.refs();
    let refname = RefName::new(BString::from(args.name.as_str()))?;

    if args.delete {
        refs.delete_ref(&refname)?;
        return Ok(0);
    }

    if let Some(ref target_str) = args.target {
        // Set symbolic ref
        let target = RefName::new(BString::from(target_str.as_str()))?;
        refs.write_symbolic_ref(&refname, &target)?;
        return Ok(0);
    }

    // Read symbolic ref
    match refs.resolve(&refname)? {
        Some(Reference::Symbolic { target, .. }) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if args.short {
                writeln!(out, "{}", target.short_name().to_str_lossy())?;
            } else {
                writeln!(out, "{}", target.as_str())?;
            }
            Ok(0)
        }
        Some(Reference::Direct { .. }) => {
            if !args.quiet {
                eprintln!("fatal: ref {} is not a symbolic ref", args.name);
            }
            Ok(1)
        }
        None => {
            if !args.quiet {
                eprintln!("fatal: ref {} is not a symbolic ref", args.name);
            }
            Ok(1)
        }
    }
}

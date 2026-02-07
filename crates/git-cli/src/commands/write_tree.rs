use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_index::Index;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct WriteTreeArgs {
    /// Write a tree for a subdirectory <prefix>
    #[arg(long)]
    prefix: Option<String>,

    /// Only check if the tree can be created (missing objects)
    #[arg(long)]
    missing_ok: bool,
}

pub fn run(_args: &WriteTreeArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;

    // Load index directly to avoid mutable borrow of repo
    let index_path = repo.git_dir().join("index");
    let index = Index::read_from(&index_path)?;

    if !index.conflicts().is_empty() {
        anyhow::bail!("cannot write tree: you have unmerged entries");
    }

    let oid = index.write_tree(repo.odb())?;

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{}", oid.to_hex())?;

    Ok(0)
}

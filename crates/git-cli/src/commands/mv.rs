use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_index::{IndexEntry, Stage, StatData};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct MvArgs {
    /// Force rename even if target exists
    #[arg(short, long)]
    force: bool,

    /// Dry run
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Source path
    source: String,

    /// Destination path
    destination: String,
}

pub fn run(args: &MvArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    let src_rel = BString::from(args.source.as_bytes());
    let dst_rel = BString::from(args.destination.as_bytes());

    let src_fs = work_tree.join(&args.source);
    let dst_fs = work_tree.join(&args.destination);

    if !src_fs.exists() {
        bail!("bad source, source={}, destination={}", args.source, args.destination);
    }

    if dst_fs.exists() && !args.force {
        bail!("destination exists, source={}, destination={}", args.source, args.destination);
    }

    // Get the index entry for the source
    let entry = {
        let index = repo.index()?;
        index
            .get(src_rel.as_ref(), Stage::Normal)
            .ok_or_else(|| anyhow::anyhow!("pathspec '{}' is not in the index", args.source))?
            .clone()
    };

    if args.verbose || args.dry_run {
        let stderr = io::stderr();
        let mut err_out = stderr.lock();
        writeln!(err_out, "Renaming {} to {}", args.source, args.destination)?;
    }

    if args.dry_run {
        return Ok(0);
    }

    // Move file in working tree
    if let Some(parent) = dst_fs.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&src_fs, &dst_fs)?;

    // Update index: remove old, add new
    let metadata = std::fs::symlink_metadata(&dst_fs)?;
    let new_entry = IndexEntry {
        path: dst_rel,
        oid: entry.oid,
        mode: entry.mode,
        stage: Stage::Normal,
        stat: StatData::from_metadata(&metadata),
        flags: entry.flags,
    };

    {
        let index = repo.index_mut()?;
        index.remove(src_rel.as_ref(), Stage::Normal);
        index.add(new_entry);
    }
    repo.write_index()?;

    Ok(0)
}

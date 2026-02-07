use std::io::{self, BufRead, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_object::{FileMode, Tree, TreeEntry};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct MktreeArgs {
    /// Allow missing objects
    #[arg(long)]
    missing: bool,

    /// Read multiple trees separated by blank lines (batch mode)
    #[arg(long)]
    batch: bool,

    /// NUL line terminator
    #[arg(short = 'z')]
    nul_terminated: bool,
}

pub fn run(args: &MktreeArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let odb = repo.odb();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let stdin = io::stdin();
    let mut entries: Vec<TreeEntry> = Vec::new();

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim_end();

        if line.is_empty() {
            if args.batch && !entries.is_empty() {
                let oid = write_tree(&mut entries, odb)?;
                writeln!(out, "{}", oid.to_hex())?;
            }
            continue;
        }

        // Parse ls-tree format: <mode> SP <type> SP <oid> TAB <name>
        let entry = parse_tree_line(line)?;

        if !args.missing && !odb.contains(&entry.oid) {
            bail!("missing object {}", entry.oid.to_hex());
        }

        entries.push(entry);
    }

    if !entries.is_empty() {
        let oid = write_tree(&mut entries, odb)?;
        writeln!(out, "{}", oid.to_hex())?;
    }

    Ok(0)
}

fn parse_tree_line(line: &str) -> Result<TreeEntry> {
    // Format: <mode> SP <type> SP <oid> TAB <name>
    let tab_pos = line
        .find('\t')
        .ok_or_else(|| anyhow::anyhow!("invalid tree entry: missing tab"))?;

    let header = &line[..tab_pos];
    let name = &line[tab_pos + 1..];

    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() != 3 {
        bail!("invalid tree entry: expected 'mode type oid'");
    }

    let mode_raw = u32::from_str_radix(parts[0], 8)?;
    let mode = FileMode::from_raw(mode_raw);
    let oid = ObjectId::from_hex(parts[2])?;

    Ok(TreeEntry {
        mode,
        name: BString::from(name),
        oid,
    })
}

fn write_tree(
    entries: &mut Vec<TreeEntry>,
    odb: &git_odb::ObjectDatabase,
) -> Result<ObjectId> {
    let mut tree = Tree::new();
    tree.entries.append(entries);
    tree.sort();
    let obj = git_object::Object::Tree(tree);
    let oid = odb.write(&obj)?;
    Ok(oid)
}

use std::io::{self, Read, Write};

use anyhow::Result;
use clap::Args;
use git_object::{Object, Tag};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct MktagArgs {
    // No arguments — reads tag content from stdin
}

pub fn run(_args: &MktagArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let odb = repo.odb();

    let mut data = Vec::new();
    io::stdin().read_to_end(&mut data)?;

    // Validate by parsing
    let tag = Tag::parse(&data)?;

    // Verify the tagged object exists
    if !odb.contains(&tag.target) {
        anyhow::bail!(
            "tag target {} ({}) not found",
            tag.target.to_hex(),
            tag.target_type
        );
    }

    let obj = Object::Tag(tag);
    let oid = odb.write(&obj)?;

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{}", oid.to_hex())?;

    Ok(0)
}

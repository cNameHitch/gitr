use std::io::{self, Read, Write};

use anyhow::Result;
use clap::Args;
use git_hash::hasher::Hasher;
use git_object::ObjectType;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct HashObjectArgs {
    /// Read the object from stdin
    #[arg(long)]
    stdin: bool,

    /// Actually write the object into the object database
    #[arg(short = 'w')]
    write: bool,

    /// Object type (default: blob)
    #[arg(short = 't', default_value = "blob")]
    obj_type: ObjectType,

    /// Files to hash
    #[arg(value_name = "file")]
    files: Vec<String>,
}

pub fn run(args: &HashObjectArgs, cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // We only need a repo if writing
    let repo = if args.write { Some(open_repo(cli)?) } else { None };

    if args.stdin {
        let mut data = Vec::new();
        io::stdin().read_to_end(&mut data)?;
        let oid = hash_and_maybe_write(&data, &args.obj_type, repo.as_ref())?;
        writeln!(out, "{}", oid.to_hex())?;
    }

    for file in &args.files {
        let data = std::fs::read(file)?;
        let oid = hash_and_maybe_write(&data, &args.obj_type, repo.as_ref())?;
        writeln!(out, "{}", oid.to_hex())?;
    }

    Ok(0)
}

fn hash_and_maybe_write(
    data: &[u8],
    obj_type: &ObjectType,
    repo: Option<&git_repository::Repository>,
) -> Result<git_hash::ObjectId> {
    let algo = repo
        .map(|r| r.hash_algo())
        .unwrap_or(git_hash::HashAlgorithm::Sha1);

    if let Some(repo) = repo {
        let oid = repo.odb().write_raw(*obj_type, data)?;
        Ok(oid)
    } else {
        let type_str = std::str::from_utf8(obj_type.as_bytes())?;
        let oid = Hasher::hash_object(algo, type_str, data)?;
        Ok(oid)
    }
}

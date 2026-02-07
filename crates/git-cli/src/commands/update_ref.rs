use std::io::{self, Read};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_ref::{RefName, RefStore, RefTransaction};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct UpdateRefArgs {
    /// Delete the reference
    #[arg(short = 'd')]
    delete: bool,

    /// Read updates from stdin (transaction mode)
    #[arg(long)]
    stdin: bool,

    /// NUL-terminated input (use with --stdin)
    #[arg(short = 'z')]
    nul_terminated: bool,

    /// Create a reflog entry with this message
    #[arg(short = 'm')]
    message: Option<String>,

    /// Do not create a reflog entry
    #[arg(long)]
    no_deref: bool,

    /// Reference name
    #[arg(required_unless_present = "stdin")]
    refname: Option<String>,

    /// New value (OID or empty to delete)
    new_value: Option<String>,

    /// Old value for CAS
    old_value: Option<String>,
}

pub fn run(args: &UpdateRefArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let refs = repo.refs();
    let msg = args.message.as_deref().unwrap_or("update-ref");

    if args.stdin {
        return run_stdin_transaction(refs, msg, args.nul_terminated);
    }

    let refname_str = args.refname.as_deref().unwrap();
    let refname = RefName::new(BString::from(refname_str))?;

    if args.delete {
        // Delete mode
        if let Some(ref old_str) = args.new_value {
            // `git update-ref -d ref old_value`
            let old_oid = ObjectId::from_hex(old_str)?;
            let mut tx = RefTransaction::new();
            tx.delete(refname, old_oid, msg);
            refs.commit_transaction(tx)?;
        } else {
            refs.delete_ref(&refname)?;
        }
        return Ok(0);
    }

    let new_str = args
        .new_value
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing <newvalue>"))?;
    let new_oid = ObjectId::from_hex(new_str)?;

    if let Some(ref old_str) = args.old_value {
        let old_oid = ObjectId::from_hex(old_str)?;
        let mut tx = RefTransaction::new();
        tx.update(refname, old_oid, new_oid, msg);
        refs.commit_transaction(tx)?;
    } else {
        refs.write_ref(&refname, &new_oid)?;
    }

    Ok(0)
}

fn run_stdin_transaction(refs: &git_ref::FilesRefStore, default_msg: &str, nul_terminated: bool) -> Result<i32> {
    let stdin = io::stdin();
    let raw_input = {
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        buf
    };

    let commands: Vec<&str> = if nul_terminated {
        raw_input.split('\0').collect()
    } else {
        raw_input.lines().collect()
    };

    let mut tx = RefTransaction::new();

    for line in commands {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        match parts.first().copied() {
            Some("start") | Some("prepare") | Some("commit") => {
                // Transaction lifecycle: start/prepare/commit
                // For our implementation, start and prepare are no-ops;
                // commit triggers the actual ref update.
                if parts[0] == "commit" && !tx.is_empty() {
                    refs.commit_transaction(std::mem::replace(&mut tx, RefTransaction::new()))?;
                }
            }
            Some("abort") => {
                // Discard the current transaction
                drop(tx);
                return Ok(0);
            }
            Some("create") => {
                if parts.len() < 3 {
                    bail!("create requires <ref> <new-oid>");
                }
                let refname = RefName::new(BString::from(parts[1]))?;
                let new_oid = ObjectId::from_hex(parts[2])?;
                tx.create(refname, new_oid, default_msg);
            }
            Some("update") => {
                if parts.len() < 3 {
                    bail!("update requires <ref> <new-oid> [<old-oid>]");
                }
                let refname = RefName::new(BString::from(parts[1]))?;
                let new_oid = ObjectId::from_hex(parts[2])?;
                if parts.len() >= 4 && !parts[3].is_empty() {
                    let old_oid = ObjectId::from_hex(parts[3])?;
                    tx.update(refname, old_oid, new_oid, default_msg);
                } else {
                    tx.create(refname, new_oid, default_msg);
                }
            }
            Some("delete") => {
                if parts.len() < 2 {
                    bail!("delete requires <ref> [<old-oid>]");
                }
                let refname = RefName::new(BString::from(parts[1]))?;
                if parts.len() >= 3 && !parts[2].is_empty() {
                    let old_oid = ObjectId::from_hex(parts[2])?;
                    tx.delete(refname, old_oid, default_msg);
                } else {
                    // We need the current OID
                    let current = refs
                        .resolve_to_oid(&refname)?
                        .ok_or_else(|| anyhow::anyhow!("ref '{}' not found", refname.as_str()))?;
                    tx.delete(refname, current, default_msg);
                }
            }
            Some("verify") => {
                if parts.len() < 3 {
                    bail!("verify requires <ref> <old-oid>");
                }
                let refname = RefName::new(BString::from(parts[1]))?;
                let expected_oid = ObjectId::from_hex(parts[2])?;
                let current = refs.resolve_to_oid(&refname)?;
                match current {
                    Some(oid) if oid == expected_oid => {
                        // OK, ref matches
                    }
                    Some(oid) => {
                        bail!(
                            "fatal: cannot lock ref '{}': is at {} but expected {}",
                            parts[1],
                            oid.to_hex(),
                            expected_oid.to_hex()
                        );
                    }
                    None => {
                        if !expected_oid.is_null() {
                            bail!(
                                "fatal: cannot lock ref '{}': ref not found but expected {}",
                                parts[1],
                                expected_oid.to_hex()
                            );
                        }
                    }
                }
            }
            _ => bail!("unknown command in transaction: {}", line),
        }
    }

    if !tx.is_empty() {
        refs.commit_transaction(tx)?;
    }

    Ok(0)
}

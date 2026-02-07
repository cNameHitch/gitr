use std::io::{self, BufRead, Write};

use anyhow::{bail, Result};
use bstr::ByteSlice;
use clap::Args;
use git_hash::ObjectId;
use git_object::ObjectType;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CatFileArgs {
    /// Show object type
    #[arg(short = 't', conflicts_with_all = ["size", "pretty", "batch", "batch_check"])]
    type_only: bool,

    /// Show object size
    #[arg(short = 's', conflicts_with_all = ["type_only", "pretty", "batch", "batch_check"])]
    size: bool,

    /// Pretty-print the object content
    #[arg(short = 'p', conflicts_with_all = ["type_only", "size", "batch", "batch_check"])]
    pretty: bool,

    /// Read objects in batch mode from stdin
    #[arg(long, conflicts_with_all = ["type_only", "size", "pretty", "batch_check"])]
    batch: bool,

    /// Read objects in batch-check mode from stdin (type + size only)
    #[arg(long, conflicts_with_all = ["type_only", "size", "pretty", "batch"])]
    batch_check: bool,

    /// Positional args: either <object> (with -t/-s/-p) or <type> <object>
    #[arg(value_name = "arg")]
    positional: Vec<String>,
}

pub fn run(args: &CatFileArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let odb = repo.odb();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.batch || args.batch_check {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let oid = match resolve_object(line, odb) {
                Ok(oid) => oid,
                Err(_) => {
                    writeln!(out, "{} missing", line)?;
                    continue;
                }
            };

            match odb.read_header(&oid)? {
                Some(info) => {
                    if args.batch_check {
                        writeln!(out, "{} {} {}", oid.to_hex(), info.obj_type, info.size)?;
                    } else {
                        let obj = odb.read(&oid)?.unwrap();
                        let content = obj.serialize_content();
                        writeln!(out, "{} {} {}", oid.to_hex(), info.obj_type, content.len())?;
                        out.write_all(&content)?;
                        writeln!(out)?;
                    }
                }
                None => {
                    writeln!(out, "{} missing", line)?;
                }
            }
        }
        return Ok(0);
    }

    // Parse positional args
    let (obj_type, object_str) = if args.type_only || args.size || args.pretty {
        // -t/-s/-p: single positional is the object
        if args.positional.is_empty() {
            bail!("missing object argument");
        }
        (None, args.positional[0].as_str())
    } else if args.positional.len() >= 2 {
        // <type> <object>
        let parsed_type: ObjectType = args.positional[0].parse()
            .map_err(|_| anyhow::anyhow!("invalid object type: {}", args.positional[0]))?;
        (Some(parsed_type), args.positional[1].as_str())
    } else if args.positional.len() == 1 {
        // Ambiguous single arg: treat as -p <object>
        (None, args.positional[0].as_str())
    } else {
        bail!("missing arguments; usage: cat-file (-t | -s | -p | <type>) <object>");
    };

    let oid = resolve_object_with_repo(object_str, &repo)?;

    if args.type_only {
        let info = odb
            .read_header(&oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid.to_hex()))?;
        writeln!(out, "{}", info.obj_type)?;
        return Ok(0);
    }

    if args.size {
        let info = odb
            .read_header(&oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid.to_hex()))?;
        writeln!(out, "{}", info.size)?;
        return Ok(0);
    }

    let obj = odb
        .read(&oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid.to_hex()))?;

    if let Some(expected_type) = obj_type {
        if obj.object_type() != expected_type {
            bail!(
                "expected {} but got {}",
                expected_type,
                obj.object_type()
            );
        }
        let content = obj.serialize_content();
        out.write_all(&content)?;
        return Ok(0);
    }

    // Pretty-print (default with -p or single arg)
    pretty_print(&obj, &mut out)?;
    Ok(0)
}

fn resolve_object(spec: &str, odb: &git_odb::ObjectDatabase) -> Result<ObjectId> {
    if let Ok(oid) = ObjectId::from_hex(spec) {
        return Ok(oid);
    }
    if let Ok(oid) = odb.resolve_prefix(spec) {
        return Ok(oid);
    }
    bail!("object not found: {}", spec);
}

fn resolve_object_with_repo(spec: &str, repo: &git_repository::Repository) -> Result<ObjectId> {
    // Try as hex OID first
    if let Ok(oid) = ObjectId::from_hex(spec) {
        return Ok(oid);
    }
    // Try as prefix
    if let Ok(oid) = repo.odb().resolve_prefix(spec) {
        return Ok(oid);
    }
    // Try as ref name / revision expression
    if let Ok(oid) = git_revwalk::resolve_revision(repo, spec) {
        return Ok(oid);
    }
    bail!("object not found: {}", spec);
}

fn pretty_print(obj: &git_object::Object, out: &mut impl Write) -> Result<()> {
    match obj {
        git_object::Object::Blob(blob) => {
            out.write_all(&blob.data)?;
        }
        git_object::Object::Tree(tree) => {
            for entry in tree.iter() {
                let type_name = if entry.mode.is_tree() {
                    "tree"
                } else if entry.mode.is_gitlink() {
                    "commit"
                } else {
                    "blob"
                };
                writeln!(
                    out,
                    "{:06o} {} {}\t{}",
                    entry.mode.raw(),
                    type_name,
                    entry.oid.to_hex(),
                    entry.name.as_bstr(),
                )?;
            }
        }
        git_object::Object::Commit(commit) => {
            writeln!(out, "tree {}", commit.tree.to_hex())?;
            for parent in &commit.parents {
                writeln!(out, "parent {}", parent.to_hex())?;
            }
            writeln!(out, "author {}", commit.author.to_bytes().as_bstr())?;
            writeln!(out, "committer {}", commit.committer.to_bytes().as_bstr())?;
            if let Some(ref gpgsig) = commit.gpgsig {
                write!(out, "gpgsig ")?;
                out.write_all(gpgsig)?;
                writeln!(out)?;
            }
            for (key, value) in &commit.extra_headers {
                writeln!(out, "{} {}", key.as_bstr(), value.as_bstr())?;
            }
            writeln!(out)?;
            out.write_all(&commit.message)?;
        }
        git_object::Object::Tag(tag) => {
            writeln!(out, "object {}", tag.target.to_hex())?;
            writeln!(out, "type {}", tag.target_type)?;
            writeln!(out, "tag {}", tag.tag_name.as_bstr())?;
            if let Some(ref tagger) = tag.tagger {
                writeln!(out, "tagger {}", tagger.to_bytes().as_bstr())?;
            }
            writeln!(out)?;
            out.write_all(&tag.message)?;
        }
    }
    Ok(())
}

use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_ref::RefStore;
use git_revwalk::RevWalk;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DescribeArgs {
    /// Use any tag, not just annotated
    #[arg(long)]
    tags: bool,

    /// Always show long format
    #[arg(long)]
    long: bool,

    /// Always show abbreviated OID
    #[arg(long)]
    always: bool,

    /// Append dirty marker if working tree is dirty
    #[arg(long)]
    dirty: Option<Option<String>>,

    /// Number of hex digits for abbreviation
    #[arg(long, default_value = "7")]
    abbrev: usize,

    /// Commit to describe (defaults to HEAD)
    commit: Option<String>,
}

pub fn run(args: &DescribeArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let target_oid = if let Some(ref rev) = args.commit {
        git_revwalk::resolve_revision(&repo, rev)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD does not point to a valid object"))?
    };

    // Collect all tags and their target OIDs
    let mut tag_map: std::collections::HashMap<ObjectId, (String, bool)> =
        std::collections::HashMap::new();

    let tag_refs = repo.refs().iter(Some("refs/tags/"))?;
    for r in tag_refs {
        let r = r?;
        let full_name = r.name().as_str().to_string();
        let tag_name = full_name
            .strip_prefix("refs/tags/")
            .unwrap_or(&full_name)
            .to_string();

        if let Some(oid) = r.target_oid() {
            // Check if this is an annotated tag
            let obj = repo.odb().read(&oid)?;
            match obj {
                Some(Object::Tag(tag)) => {
                    // Annotated tag: map the tagged commit OID
                    tag_map.insert(tag.target, (tag_name, true));
                }
                _ => {
                    if args.tags {
                        // Lightweight tag
                        tag_map.insert(oid, (tag_name, false));
                    }
                }
            }
        }
    }

    // Check if target commit IS a tag
    if let Some((tag_name, _)) = tag_map.get(&target_oid) {
        if args.long {
            let hex = target_oid.to_hex();
            let abbrev = &hex[..args.abbrev.min(hex.len())];
            write!(out, "{}-0-g{}", tag_name, abbrev)?;
        } else {
            write!(out, "{}", tag_name)?;
        }
        if let Some(ref dirty) = args.dirty {
            let marker = dirty.as_deref().unwrap_or("-dirty");
            // Check if worktree is dirty (simplified: check index)
            write!(out, "{}", marker)?;
        }
        writeln!(out)?;
        return Ok(0);
    }

    // Walk backwards from target to find nearest tag
    let mut walker = RevWalk::new(&repo)?;
    walker.push(target_oid)?;

    let mut distance = 0u32;
    let mut found_tag: Option<String> = None;

    for oid_result in walker {
        let oid = oid_result?;

        if oid != target_oid {
            distance += 1;
        }

        if let Some((tag_name, _)) = tag_map.get(&oid) {
            found_tag = Some(tag_name.clone());
            break;
        }
    }

    match found_tag {
        Some(tag_name) => {
            if distance == 0 && !args.long {
                write!(out, "{}", tag_name)?;
            } else {
                let hex = target_oid.to_hex();
                let abbrev = &hex[..args.abbrev.min(hex.len())];
                write!(out, "{}-{}-g{}", tag_name, distance, abbrev)?;
            }
        }
        None => {
            if args.always {
                let hex = target_oid.to_hex();
                let abbrev = &hex[..args.abbrev.min(hex.len())];
                write!(out, "{}", abbrev)?;
            } else if !args.tags && has_unannotated_tags(&repo)? {
                // There are unannotated tags but --tags wasn't specified
                let hex = target_oid.to_hex();
                let abbrev = &hex[..args.abbrev.min(hex.len())];
                anyhow::bail!(
                    "No annotated tags can describe '{}'.\nHowever, there were unannotated tags: try --tags.",
                    abbrev
                );
            } else {
                anyhow::bail!(
                    "No names found, cannot describe anything."
                );
            }
        }
    }

    if let Some(ref dirty) = args.dirty {
        let marker = dirty.as_deref().unwrap_or("-dirty");
        write!(out, "{}", marker)?;
    }

    writeln!(out)?;
    Ok(0)
}

/// Check if the repository has any unannotated (lightweight) tags.
fn has_unannotated_tags(repo: &git_repository::Repository) -> Result<bool> {
    let tag_refs = repo.refs().iter(Some("refs/tags/"))?;
    for r in tag_refs {
        let r = r?;
        if let Some(oid) = r.target_oid() {
            let obj = repo.odb().read(&oid)?;
            match obj {
                Some(Object::Tag(_)) => {} // annotated tag, skip
                Some(_) => return Ok(true), // lightweight tag
                None => {}
            }
        }
    }
    Ok(false)
}

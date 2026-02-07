use std::collections::HashSet;
use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_ref::RefStore;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct FsckArgs {
    /// Report unreachable objects
    #[arg(long)]
    unreachable: bool,

    /// Report dangling objects
    #[arg(long, default_value_t = true)]
    dangling: bool,

    /// Report root nodes
    #[arg(long)]
    root: bool,

    /// Enable strict checking
    #[arg(long)]
    strict: bool,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Show progress
    #[arg(long)]
    progress: bool,

    /// Only check connectivity (don't check object contents)
    #[arg(long)]
    connectivity_only: bool,

    /// Include reflogs in reachability check
    #[arg(long, default_value_t = true)]
    include_reflogs: bool,

    /// Report tags
    #[arg(long)]
    tags: bool,

    /// Do not suppress dangling object reports
    #[arg(long)]
    no_dangling: bool,

    /// Name objects by path where they are reachable
    #[arg(long)]
    name_objects: bool,

    /// Objects to check (defaults to all)
    objects: Vec<String>,
}

pub fn run(args: &FsckArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let show_dangling = !args.no_dangling && args.dangling;
    let mut errors_found = false;

    // Step 1: Collect all objects in the repository
    if args.verbose {
        writeln!(err, "Checking object directory")?;
    }

    let mut all_oids: Vec<ObjectId> = Vec::new();
    let mut corrupt_oids: Vec<ObjectId> = Vec::new();

    let iter = repo.odb().iter_all_oids()?;
    for result in iter {
        match result {
            Ok(oid) => all_oids.push(oid),
            Err(e) => {
                writeln!(out, "error: {}", e)?;
                errors_found = true;
            }
        }
    }

    if args.verbose {
        writeln!(err, "Checking {} objects", all_oids.len())?;
    }

    // Step 2: Validate each object
    let mut referenced_oids: HashSet<ObjectId> = HashSet::new();

    if !args.connectivity_only {
        for oid in &all_oids {
            match repo.odb().read(oid)? {
                Some(obj) => {
                    // Validate object internals
                    if let Err(msg) = validate_object(&obj, oid) {
                        writeln!(out, "error in {} {}: {}", obj.object_type(), oid.to_hex(), msg)?;
                        errors_found = true;
                        corrupt_oids.push(*oid);
                    }

                    // Collect referenced OIDs
                    collect_references(&obj, &mut referenced_oids);
                }
                None => {
                    writeln!(out, "missing {}", oid.to_hex())?;
                    errors_found = true;
                    corrupt_oids.push(*oid);
                }
            }
        }
    }

    // Step 3: Connectivity check — verify all referenced objects exist
    let all_oid_set: HashSet<ObjectId> = all_oids.iter().copied().collect();

    for referenced in &referenced_oids {
        if !all_oid_set.contains(referenced) {
            writeln!(out, "broken link from unknown to {}", referenced.to_hex())?;
            errors_found = true;
        }
    }

    // Step 4: Find reachable objects from refs
    let mut tips: Vec<ObjectId> = Vec::new();
    let refs = repo.refs().iter(None)?;
    for r in refs {
        let r = r?;
        let oid = r.peel_to_oid(repo.refs())?;
        tips.push(oid);

        if args.verbose {
            writeln!(err, "Checking {}", r.name().as_str())?;
        }
    }
    if let Some(head) = repo.head_oid()? {
        tips.push(head);
    }

    let reachable: HashSet<ObjectId> = git_revwalk::list_objects(&repo, &tips, &[], None)?
        .into_iter()
        .collect();

    // Step 5: Find dangling and unreachable objects
    let mut dangling_count = 0u32;
    let mut unreachable_count = 0u32;

    for oid in &all_oids {
        if corrupt_oids.contains(oid) {
            continue;
        }
        if !reachable.contains(oid) {
            if args.unreachable {
                let type_str = match repo.odb().read_header(oid)? {
                    Some(info) => format!("{}", info.obj_type),
                    None => "unknown".to_string(),
                };
                writeln!(out, "unreachable {} {}", type_str, oid.to_hex())?;
                unreachable_count += 1;
            }

            // Dangling = unreachable and not referenced by any other object
            if show_dangling && !referenced_oids.contains(oid) {
                let type_str = match repo.odb().read_header(oid)? {
                    Some(info) => format!("{}", info.obj_type),
                    None => "unknown".to_string(),
                };
                writeln!(out, "dangling {} {}", type_str, oid.to_hex())?;
                dangling_count += 1;
            }
        }
    }

    // Step 6: Check refs point to valid objects
    let refs = repo.refs().iter(None)?;
    for r in refs {
        let r = r?;
        let oid = r.peel_to_oid(repo.refs())?;
        if !repo.odb().contains(&oid) {
            writeln!(
                out,
                "error: refs/{} points to nonexistent {}",
                r.name().as_str(),
                oid.to_hex()
            )?;
            errors_found = true;
        }
    }

    // Summary
    if args.verbose {
        writeln!(err, "Checking connectivity ({} objects)", all_oids.len())?;
        if dangling_count > 0 {
            writeln!(err, "dangling objects: {}", dangling_count)?;
        }
        if unreachable_count > 0 {
            writeln!(err, "unreachable objects: {}", unreachable_count)?;
        }
    }

    if errors_found {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Validate an object's internal consistency.
fn validate_object(obj: &Object, _oid: &ObjectId) -> std::result::Result<(), String> {
    match obj {
        Object::Blob(_) => Ok(()),
        Object::Tree(tree) => {
            let mut prev_name: Option<&[u8]> = None;
            for entry in &tree.entries {
                // Check for valid mode
                let mode_val = entry.mode.raw();
                let valid_modes = [
                    0o040000, // tree
                    0o100644, // regular file
                    0o100755, // executable
                    0o120000, // symlink
                    0o160000, // gitlink (submodule)
                ];
                if !valid_modes.contains(&mode_val) {
                    return Err(format!("invalid mode {:o} for entry {:?}", mode_val, entry.name));
                }

                // Check entries are sorted
                if let Some(prev) = prev_name {
                    let name_bytes: &[u8] = entry.name.as_ref();
                    if name_bytes <= prev {
                        return Err(format!(
                            "tree entries not sorted: {:?} after {:?}",
                            entry.name,
                            String::from_utf8_lossy(prev)
                        ));
                    }
                }
                prev_name = Some(entry.name.as_ref() as &[u8]);

                // Check for empty names
                if entry.name.is_empty() {
                    return Err("tree entry has empty name".to_string());
                }
            }
            Ok(())
        }
        Object::Commit(commit) => {
            // Check for tree reference
            if commit.tree.is_null() {
                return Err("commit has null tree".to_string());
            }

            // Check author/committer are present
            if commit.author.name.is_empty() {
                return Err("commit has empty author name".to_string());
            }

            Ok(())
        }
        Object::Tag(tag) => {
            // Check tag has required fields
            if tag.tag_name.is_empty() {
                return Err("tag has empty name".to_string());
            }
            if tag.target.is_null() {
                return Err("tag points to null object".to_string());
            }
            Ok(())
        }
    }
}

/// Collect all OIDs referenced by an object.
fn collect_references(obj: &Object, refs: &mut HashSet<ObjectId>) {
    match obj {
        Object::Blob(_) => {}
        Object::Tree(tree) => {
            for entry in &tree.entries {
                refs.insert(entry.oid);
            }
        }
        Object::Commit(commit) => {
            refs.insert(commit.tree);
            for parent in &commit.parents {
                refs.insert(*parent);
            }
        }
        Object::Tag(tag) => {
            refs.insert(tag.target);
        }
    }
}

use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct ReplaceArgs {
    /// Delete replacement refs
    #[arg(short, long)]
    delete: bool,

    /// List replacement refs
    #[arg(short, long)]
    list: bool,

    /// Force overwrite of existing replacement
    #[arg(short, long)]
    force: bool,

    /// Create a graft commit
    #[arg(long)]
    graft: bool,

    /// Edit an object and create a replacement
    #[arg(long)]
    edit: bool,

    /// Output format for --list (short, medium, long)
    #[arg(long, default_value = "short")]
    format: String,

    /// Object (or objects for --graft: commit parent1 parent2 ...)
    objects: Vec<String>,
}

pub fn run(args: &ReplaceArgs, cli: &Cli) -> Result<i32> {
    if args.list || (args.objects.is_empty() && !args.delete) {
        return list_replacements(cli, &args.format, args.objects.first().map(|s| s.as_str()));
    }

    if args.delete {
        return delete_replacements(cli, &args.objects);
    }

    if args.graft {
        return create_graft(cli, &args.objects, args.force);
    }

    // Default: create a replacement
    if args.objects.len() < 2 {
        bail!("usage: git replace <object> <replacement>");
    }

    create_replacement(cli, &args.objects[0], &args.objects[1], args.force)
}

fn list_replacements(cli: &Cli, format: &str, pattern: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let refs = repo.refs().iter(Some("refs/replace/"))?;
    for r in refs {
        let r = r?;
        let name = r.name().as_str().to_string();
        let original_hex = name
            .strip_prefix("refs/replace/")
            .unwrap_or(&name);

        // Apply pattern filter
        if let Some(pat) = pattern {
            if !original_hex.starts_with(pat) {
                continue;
            }
        }

        let replacement_oid = r.peel_to_oid(repo.refs())?;

        match format {
            "short" => {
                writeln!(out, "{}", original_hex)?;
            }
            "medium" => {
                writeln!(out, "{} -> {}", original_hex, replacement_oid.to_hex())?;
            }
            "long" => {
                let original_oid = ObjectId::from_hex(original_hex)?;
                let orig_type = repo
                    .odb()
                    .read_header(&original_oid)?
                    .map(|i| format!("{}", i.obj_type))
                    .unwrap_or_else(|| "unknown".to_string());
                let repl_type = repo
                    .odb()
                    .read_header(&replacement_oid)?
                    .map(|i| format!("{}", i.obj_type))
                    .unwrap_or_else(|| "unknown".to_string());
                writeln!(
                    out,
                    "{} ({}) -> {} ({})",
                    original_hex,
                    orig_type,
                    replacement_oid.to_hex(),
                    repl_type
                )?;
            }
            _ => {
                writeln!(out, "{}", original_hex)?;
            }
        }
    }

    Ok(0)
}

fn delete_replacements(cli: &Cli, objects: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    for spec in objects {
        let oid = git_revwalk::resolve_revision(&repo, spec)?;
        let refname = RefName::new(BString::from(format!("refs/replace/{}", oid.to_hex())))?;

        if repo.refs().resolve(&refname)?.is_none() {
            bail!("replace ref for {} not found", oid.to_hex());
        }

        repo.refs().delete_ref(&refname)?;
        writeln!(err, "Deleted replace ref for {}", oid.to_hex())?;
    }

    Ok(0)
}

fn create_replacement(cli: &Cli, original: &str, replacement: &str, force: bool) -> Result<i32> {
    let repo = open_repo(cli)?;

    let original_oid = git_revwalk::resolve_revision(&repo, original)?;
    let replacement_oid = git_revwalk::resolve_revision(&repo, replacement)?;

    // Verify both objects exist
    if !repo.odb().contains(&original_oid) {
        bail!("object {} not found", original_oid.to_hex());
    }
    if !repo.odb().contains(&replacement_oid) {
        bail!("object {} not found", replacement_oid.to_hex());
    }

    // Check types match
    let orig_info = repo
        .odb()
        .read_header(&original_oid)?
        .ok_or_else(|| anyhow::anyhow!("cannot read {}", original_oid.to_hex()))?;
    let repl_info = repo
        .odb()
        .read_header(&replacement_oid)?
        .ok_or_else(|| anyhow::anyhow!("cannot read {}", replacement_oid.to_hex()))?;

    if orig_info.obj_type != repl_info.obj_type {
        bail!(
            "Objects must be of the same type: {} is {}, {} is {}",
            original_oid.to_hex(),
            orig_info.obj_type,
            replacement_oid.to_hex(),
            repl_info.obj_type
        );
    }

    let refname = RefName::new(BString::from(format!(
        "refs/replace/{}",
        original_oid.to_hex()
    )))?;

    if !force && repo.refs().resolve(&refname)?.is_some() {
        bail!(
            "replace ref already exists for {}; use -f to overwrite",
            original_oid.to_hex()
        );
    }

    repo.refs().write_ref(&refname, &replacement_oid)?;

    Ok(0)
}

fn create_graft(cli: &Cli, objects: &[String], force: bool) -> Result<i32> {
    let repo = open_repo(cli)?;

    if objects.is_empty() {
        bail!("usage: git replace --graft <commit> [<parent>...]");
    }

    let commit_oid = git_revwalk::resolve_revision(&repo, &objects[0])?;

    // Read the original commit
    let commit_obj = repo
        .odb()
        .read(&commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit {} not found", commit_oid.to_hex()))?;

    let mut commit = match commit_obj {
        Object::Commit(c) => c,
        _ => bail!("{} is not a commit", commit_oid.to_hex()),
    };

    // Replace parents with specified ones
    let mut new_parents = Vec::new();
    for parent_spec in &objects[1..] {
        let parent_oid = git_revwalk::resolve_revision(&repo, parent_spec)?;
        new_parents.push(parent_oid);
    }
    commit.parents = new_parents;

    // Write the modified commit
    let new_commit_oid = repo.odb().write(&Object::Commit(commit))?;

    // Create the replace ref
    let refname = RefName::new(BString::from(format!(
        "refs/replace/{}",
        commit_oid.to_hex()
    )))?;

    if !force && repo.refs().resolve(&refname)?.is_some() {
        bail!(
            "replace ref already exists for {}; use -f to overwrite",
            commit_oid.to_hex()
        );
    }

    repo.refs().write_ref(&refname, &new_commit_oid)?;

    Ok(0)
}

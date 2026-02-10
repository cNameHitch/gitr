use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_object::{Object, Tag};
use git_ref::{RefName, RefStore};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct TagArgs {
    /// Create an annotated tag
    #[arg(short, long)]
    annotate: bool,

    /// Delete a tag
    #[arg(short, long)]
    delete: bool,

    /// List tags
    #[arg(short, long)]
    list: bool,

    /// Tag message
    #[arg(short, long)]
    message: Option<String>,

    /// Verify tag signature
    #[arg(short = 'v', long)]
    verify: bool,

    /// Force creation even if tag already exists
    #[arg(short, long)]
    force: bool,

    /// Show tag annotation (up to N lines)
    #[arg(short = 'n', num_args = 0..=1, default_missing_value = "1")]
    show_annotation: Option<usize>,

    /// Tag name
    name: Option<String>,

    /// Object to tag (defaults to HEAD)
    object: Option<String>,
}

pub fn run(args: &TagArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // List mode (no name given, or -l flag, or -n flag without name)
    if args.list || (args.name.is_none() && !args.delete) || (args.show_annotation.is_some() && args.name.is_none()) {
        return list_tags(&repo, args.show_annotation, &mut out);
    }

    let name = args.name.as_deref().unwrap();

    if args.delete {
        return delete_tag(&repo, name, &mut out);
    }

    // Create tag
    let target_oid = if let Some(ref spec) = args.object {
        git_revwalk::resolve_revision(&repo, spec)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD does not point to a valid object"))?
    };

    let refname = RefName::new(BString::from(format!("refs/tags/{}", name)))?;

    // Check if tag exists
    if !args.force && repo.refs().resolve(&refname)?.is_some() {
        bail!("fatal: tag '{}' already exists", name);
    }

    if args.annotate || args.message.is_some() {
        // Annotated tag
        let message = args.message.as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing tag message; use -m"))?;

        let tagger = build_signature(&repo)?;

        // Determine target type
        let target_obj = repo.odb().read(&target_oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found"))?;
        let target_type = target_obj.object_type();

        let tag = Tag {
            target: target_oid,
            target_type,
            tag_name: BString::from(name),
            tagger: Some(tagger),
            message: BString::from(format!("{}\n", message)),
            gpgsig: None,
        };

        let tag_oid = repo.odb().write(&Object::Tag(tag))?;
        repo.refs().write_ref(&refname, &tag_oid)?;
    } else {
        // Lightweight tag
        repo.refs().write_ref(&refname, &target_oid)?;
    }

    Ok(0)
}

fn list_tags(repo: &git_repository::Repository, show_annotation: Option<usize>, out: &mut impl Write) -> Result<i32> {
    let refs = repo.refs().iter(Some("refs/tags/"))?;
    for r in refs {
        let r = r?;
        let full = r.name().as_str();
        let short = full.strip_prefix("refs/tags/").unwrap_or(full);

        if let Some(max_lines) = show_annotation {
            // Try to read annotation from tag object or commit subject for lightweight tags
            if let Ok(oid) = r.peel_to_oid(repo.refs()) {
                if let Ok(Some(obj)) = repo.odb().read(&oid) {
                    match obj {
                        Object::Tag(tag) => {
                            let msg = String::from_utf8_lossy(&tag.message);
                            let lines: Vec<&str> = msg.lines().collect();
                            let show_n = max_lines.max(1);
                            let annotation: String = lines.iter().take(show_n).map(|l| l.to_string()).collect::<Vec<_>>().join("\n");
                            writeln!(out, "{:<16}{}", short, annotation)?;
                            continue;
                        }
                        Object::Commit(commit) => {
                            let subject = String::from_utf8_lossy(commit.summary().as_ref());
                            writeln!(out, "{:<16}{}", short, subject)?;
                            continue;
                        }
                        _ => {}
                    }
                }
            }
            // Fallback: no annotation or subject available
            writeln!(out, "{}", short)?;
        } else {
            writeln!(out, "{}", short)?;
        }
    }
    Ok(0)
}

fn delete_tag(repo: &git_repository::Repository, name: &str, out: &mut impl Write) -> Result<i32> {
    let refname = RefName::new(BString::from(format!("refs/tags/{}", name)))?;
    let reference = repo.refs().resolve(&refname)?
        .ok_or_else(|| anyhow::anyhow!("error: tag '{}' not found", name))?;
    let oid = reference.peel_to_oid(repo.refs())?;
    repo.refs().delete_ref(&refname)?;
    writeln!(out, "Deleted tag '{}' (was {})", name, &oid.to_hex()[..7])?;
    Ok(0)
}

pub(crate) fn build_signature(repo: &git_repository::Repository) -> Result<Signature> {
    let name = std::env::var("GIT_COMMITTER_NAME")
        .or_else(|_| {
            repo.config().get_string("user.name")
                .ok()
                .flatten()
                .ok_or(std::env::VarError::NotPresent)
        })
        .unwrap_or_else(|_| "Unknown".to_string());

    let email = std::env::var("GIT_COMMITTER_EMAIL")
        .or_else(|_| {
            repo.config().get_string("user.email")
                .ok()
                .flatten()
                .ok_or(std::env::VarError::NotPresent)
        })
        .unwrap_or_else(|_| "unknown@unknown".to_string());

    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date: GitDate::now(),
    })
}

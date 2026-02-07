use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_ref::{RefStore, Reference};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct ForEachRefArgs {
    /// Format string for output
    #[arg(long, default_value = "%(objectname) %(objecttype)\t%(refname)")]
    format: String,

    /// Sort key (refname, objectname, objecttype, creatordate)
    #[arg(long)]
    sort: Option<String>,

    /// Maximum number of refs to show
    #[arg(long)]
    count: Option<usize>,

    /// Only list refs that contain the given commit
    #[arg(long)]
    contains: Option<String>,

    /// Pattern to filter refs (e.g., refs/heads/)
    #[arg(value_name = "pattern")]
    pattern: Option<String>,
}

pub fn run(args: &ForEachRefArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let refs = repo.refs();
    let odb = repo.odb();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let prefix = args.pattern.as_deref();
    let iter = refs.iter(prefix)?;

    let mut entries: Vec<Reference> = Vec::new();
    for ref_result in iter {
        let reference = ref_result?;
        // Exclude HEAD — for-each-ref only shows refs under refs/
        let name = reference.name().as_str();
        if name == "HEAD" {
            continue;
        }
        if !name.starts_with("refs/") {
            continue;
        }
        entries.push(reference);
    }

    // Sort if requested
    if let Some(ref sort_key) = args.sort {
        match sort_key.as_str() {
            "refname" | "" => {
                entries.sort_by(|a, b| a.name().as_str().cmp(b.name().as_str()));
            }
            "-refname" => {
                entries.sort_by(|a, b| b.name().as_str().cmp(a.name().as_str()));
            }
            _ => {
                entries.sort_by(|a, b| a.name().as_str().cmp(b.name().as_str()));
            }
        }
    } else {
        // Default sort by refname
        entries.sort_by(|a, b| a.name().as_str().cmp(b.name().as_str()));
    }

    // Apply count limit
    if let Some(count) = args.count {
        entries.truncate(count);
    }

    for reference in &entries {
        let oid = match reference {
            Reference::Direct { target, .. } => *target,
            Reference::Symbolic { .. } => {
                match reference.peel_to_oid(refs) {
                    Ok(oid) => oid,
                    Err(_) => continue,
                }
            }
        };

        // Determine the object type
        let obj_type = if let Ok(Some(info)) = odb.read_header(&oid) {
            format!("{}", info.obj_type)
        } else {
            "unknown".to_string()
        };

        let formatted = format_ref(&args.format, reference, &oid, &obj_type, odb)?;
        writeln!(out, "{}", formatted)?;
    }

    Ok(0)
}

fn format_ref(
    format: &str,
    reference: &Reference,
    oid: &git_hash::ObjectId,
    obj_type: &str,
    odb: &git_odb::ObjectDatabase,
) -> Result<String> {
    let mut result = format.to_string();

    let refname = reference.name().as_str();
    let short_name = reference.name().short_name();

    result = result.replace("%(refname)", refname);
    result = result.replace("%(refname:short)", std::str::from_utf8(short_name.as_bytes()).unwrap_or(refname));
    result = result.replace("%(objectname)", &oid.to_hex());
    result = result.replace(
        "%(objectname:short)",
        &oid.to_hex()[..7],
    );
    result = result.replace("%(objecttype)", obj_type);

    // Handle commit-specific fields
    if result.contains("%(author") || result.contains("%(committer") || result.contains("%(subject") || result.contains("%(body") {
        if let Ok(Some(git_object::Object::Commit(commit))) = odb.read(oid) {
            result = result.replace("%(authorname)", std::str::from_utf8(&commit.author.name).unwrap_or(""));
            result = result.replace("%(authoremail)", &format!("<{}>", std::str::from_utf8(&commit.author.email).unwrap_or("")));
            result = result.replace("%(authordate)", &commit.author.date.timestamp.to_string());
            result = result.replace("%(committername)", std::str::from_utf8(&commit.committer.name).unwrap_or(""));
            result = result.replace("%(committeremail)", &format!("<{}>", std::str::from_utf8(&commit.committer.email).unwrap_or("")));
            result = result.replace("%(committerdate)", &commit.committer.date.timestamp.to_string());
            result = result.replace("%(subject)", std::str::from_utf8(commit.summary().as_bytes()).unwrap_or(""));
            if let Some(body) = commit.body() {
                result = result.replace("%(body)", std::str::from_utf8(body.as_bytes()).unwrap_or(""));
            } else {
                result = result.replace("%(body)", "");
            }
        }
    }

    Ok(result)
}

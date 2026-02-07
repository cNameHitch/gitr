use std::io::{self, Read, Write};

use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_object::{Commit, Object};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CommitTreeArgs {
    /// Tree object ID
    tree: String,

    /// Parent commit(s)
    #[arg(short = 'p', num_args = 1)]
    parent: Vec<String>,

    /// Commit message
    #[arg(short = 'm')]
    message: Option<String>,

    /// Read message from file
    #[arg(short = 'F')]
    file: Option<String>,
}

pub fn run(args: &CommitTreeArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let odb = repo.odb();

    let tree_oid = ObjectId::from_hex(&args.tree)?;

    // Verify tree exists
    if !odb.contains(&tree_oid) {
        anyhow::bail!("not a valid object name: {}", args.tree);
    }

    let parents: Vec<ObjectId> = args
        .parent
        .iter()
        .map(|p| ObjectId::from_hex(p))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Get commit message
    let message = if let Some(ref msg) = args.message {
        BString::from(msg.as_str())
    } else if let Some(ref file) = args.file {
        BString::from(std::fs::read(file)?)
    } else {
        // Read from stdin
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        BString::from(buf)
    };

    // Build author/committer from env or config
    let author = get_signature("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", &repo)?;
    let committer = get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", &repo)?;

    let commit = Commit {
        tree: tree_oid,
        parents,
        author,
        committer,
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
        message,
    };

    let obj = Object::Commit(commit);
    let oid = odb.write(&obj)?;

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{}", oid.to_hex())?;

    Ok(0)
}

fn get_signature(
    name_var: &str,
    email_var: &str,
    date_var: &str,
    repo: &git_repository::Repository,
) -> Result<Signature> {
    let name = std::env::var(name_var)
        .ok()
        .or_else(|| {
            repo.config()
                .get_string("user.name")
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let email = std::env::var(email_var)
        .ok()
        .or_else(|| {
            repo.config()
                .get_string("user.email")
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| "unknown@unknown".to_string());

    let date = if let Ok(date_str) = std::env::var(date_var) {
        GitDate::parse_raw(&date_str)?
    } else {
        GitDate::now()
    };

    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date,
    })
}

use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct VarArgs {
    /// Variable name (e.g., GIT_AUTHOR_IDENT, GIT_COMMITTER_IDENT, GIT_EDITOR, GIT_PAGER)
    variable: String,
}

pub fn run(args: &VarArgs, cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match args.variable.as_str() {
        "GIT_AUTHOR_IDENT" => {
            let repo = open_repo(cli)?;
            let sig = build_identity("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", &repo)?;
            writeln!(out, "{}", std::str::from_utf8(&sig.to_bytes()).unwrap_or(""))?;
        }
        "GIT_COMMITTER_IDENT" => {
            let repo = open_repo(cli)?;
            let sig = build_identity("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", &repo)?;
            writeln!(out, "{}", std::str::from_utf8(&sig.to_bytes()).unwrap_or(""))?;
        }
        "GIT_EDITOR" => {
            let editor = std::env::var("GIT_EDITOR")
                .or_else(|_| std::env::var("VISUAL"))
                .or_else(|_| std::env::var("EDITOR"))
                .unwrap_or_else(|_| "vi".to_string());
            writeln!(out, "{}", editor)?;
        }
        "GIT_PAGER" => {
            let pager = std::env::var("GIT_PAGER")
                .or_else(|_| std::env::var("PAGER"))
                .unwrap_or_else(|_| "less".to_string());
            writeln!(out, "{}", pager)?;
        }
        "GIT_DEFAULT_BRANCH" => {
            let repo = open_repo(cli)?;
            let branch = repo.config()
                .get_string("init.defaultBranch")
                .ok()
                .flatten()
                .unwrap_or_else(|| "main".to_string());
            writeln!(out, "{}", branch)?;
        }
        _ => {
            eprintln!("error: unknown variable '{}'", args.variable);
            return Ok(1);
        }
    }

    Ok(0)
}

fn build_identity(
    name_var: &str,
    email_var: &str,
    date_var: &str,
    repo: &git_repository::Repository,
) -> Result<Signature> {
    let name = std::env::var(name_var)
        .ok()
        .or_else(|| repo.config().get_string("user.name").ok().flatten())
        .unwrap_or_else(|| "Unknown".to_string());

    let email = std::env::var(email_var)
        .ok()
        .or_else(|| repo.config().get_string("user.email").ok().flatten())
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

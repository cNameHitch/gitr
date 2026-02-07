use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use regex::Regex;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct GrepArgs {
    /// Case-insensitive matching
    #[arg(short = 'i', long)]
    ignore_case: bool,

    /// Show line numbers
    #[arg(short = 'n', long)]
    line_number: bool,

    /// Show only file names
    #[arg(short = 'l', long)]
    files_with_matches: bool,

    /// Show count of matches per file
    #[arg(long)]
    count: bool,

    /// Pattern to search for (can be specified multiple times)
    #[arg(short = 'e')]
    patterns: Vec<String>,

    /// Invert match
    #[arg(short = 'v', long)]
    invert_match: bool,

    /// Search all branches
    #[arg(long)]
    all: bool,

    /// Tree or commit to search in
    #[arg(long)]
    tree: Option<String>,

    /// Pattern (positional)
    pattern: Option<String>,

    /// Pathspecs to limit search
    pathspecs: Vec<String>,
}

pub fn run(args: &GrepArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Build the regex pattern
    let pattern_str = if !args.patterns.is_empty() {
        args.patterns.join("|")
    } else if let Some(ref p) = args.pattern {
        p.clone()
    } else {
        anyhow::bail!("no pattern specified");
    };

    let regex = if args.ignore_case {
        Regex::new(&format!("(?i){}", pattern_str))?
    } else {
        Regex::new(&pattern_str)?
    };

    let tree_oid = if let Some(ref tree_spec) = args.tree {
        let oid = git_revwalk::resolve_revision(&repo, tree_spec)?;
        get_tree_oid(&repo, &oid)?
    } else {
        let head = repo
            .head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;
        get_tree_oid(&repo, &head)?
    };

    let mut found = false;
    grep_tree(&repo, &tree_oid, "", &regex, args, &mut out, &mut found)?;

    Ok(if found { 0 } else { 1 })
}

fn get_tree_oid(
    repo: &git_repository::Repository,
    oid: &ObjectId,
) -> Result<ObjectId> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;
    match obj {
        Object::Commit(c) => Ok(c.tree),
        Object::Tree(_) => Ok(*oid),
        _ => anyhow::bail!("not a commit or tree: {}", oid),
    }
}

fn grep_tree(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    prefix: &str,
    regex: &Regex,
    args: &GrepArgs,
    out: &mut impl Write,
    found: &mut bool,
) -> Result<()> {
    let obj = repo
        .odb()
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found: {}", tree_oid))?;

    let tree = match obj {
        Object::Tree(t) => t,
        _ => return Ok(()),
    };

    for entry in &tree.entries {
        let name = String::from_utf8_lossy(&entry.name);
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        if entry.mode.is_tree() {
            grep_tree(repo, &entry.oid, &path, regex, args, out, found)?;
        } else if entry.mode.is_blob() {
            // Check pathspec filter
            if !args.pathspecs.is_empty() {
                let matches = args.pathspecs.iter().any(|p| path.starts_with(p));
                if !matches {
                    continue;
                }
            }

            grep_blob(repo, &entry.oid, &path, regex, args, out, found)?;
        }
    }

    Ok(())
}

fn grep_blob(
    repo: &git_repository::Repository,
    blob_oid: &ObjectId,
    path: &str,
    regex: &Regex,
    args: &GrepArgs,
    out: &mut impl Write,
    found: &mut bool,
) -> Result<()> {
    let obj = repo.odb().read(blob_oid)?;
    let data = match obj {
        Some(Object::Blob(b)) => b.data,
        _ => return Ok(()),
    };

    // Skip binary files
    if data.contains(&0) {
        return Ok(());
    }

    let content = String::from_utf8_lossy(&data);
    let mut match_count = 0u32;
    let mut file_matched = false;

    for (line_num, line) in content.lines().enumerate() {
        let matches = regex.is_match(line);
        let show = if args.invert_match { !matches } else { matches };

        if show {
            *found = true;
            file_matched = true;
            match_count += 1;

            if args.files_with_matches {
                writeln!(out, "{}", path)?;
                return Ok(());
            }

            if !args.count {
                if args.line_number {
                    writeln!(out, "{}:{}:{}", path, line_num + 1, line)?;
                } else {
                    writeln!(out, "{}:{}", path, line)?;
                }
            }
        }
    }

    if args.count && file_matched {
        writeln!(out, "{}:{}", path, match_count)?;
    }

    Ok(())
}

use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::ByteSlice;
use clap::Args;
use git_hash::ObjectId;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct LsTreeArgs {
    /// Recurse into sub-trees
    #[arg(short = 'r')]
    recurse: bool,

    /// Show only trees (directories)
    #[arg(short = 'd')]
    trees_only: bool,

    /// Show trees when recursing
    #[arg(short = 't')]
    show_trees: bool,

    /// Show only names
    #[arg(long)]
    name_only: bool,

    /// Show only names (alias)
    #[arg(long)]
    name_status: bool,

    /// Show full path names
    #[arg(long)]
    full_name: bool,

    /// Show full tree path
    #[arg(long)]
    full_tree: bool,

    /// NUL line terminator
    #[arg(short = 'z')]
    nul_terminated: bool,

    /// Tree-ish to list
    #[arg(value_name = "tree-ish")]
    tree_ish: String,

    /// Path patterns to filter
    #[arg(value_name = "path")]
    paths: Vec<String>,
}

pub fn run(args: &LsTreeArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let odb = repo.odb();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let name_only = args.name_only || args.name_status;
    let terminator = if args.nul_terminated { '\0' } else { '\n' };

    // Resolve the tree-ish to a tree OID
    let oid = git_revwalk::resolve_revision(&repo, &args.tree_ish)?;

    // Read the object — might be commit (need to get its tree) or tree directly
    let tree_oid = match odb.read(&oid)? {
        Some(git_object::Object::Tree(_)) => oid,
        Some(git_object::Object::Commit(commit)) => commit.tree,
        Some(other) => bail!("not a tree object: {} is a {}", args.tree_ish, other.object_type()),
        None => bail!("not found: {}", args.tree_ish),
    };

    list_tree(odb, &tree_oid, "", args, name_only, terminator, &mut out)?;

    Ok(0)
}

fn list_tree(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &str,
    args: &LsTreeArgs,
    name_only: bool,
    terminator: char,
    out: &mut impl Write,
) -> Result<()> {
    let tree = match odb.read(tree_oid)? {
        Some(git_object::Object::Tree(t)) => t,
        _ => bail!("not a tree: {}", tree_oid.to_hex()),
    };

    for entry in tree.iter() {
        let name = entry.name.to_str_lossy();
        let full_path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        // Filter by paths if given
        if !args.paths.is_empty() {
            let matches = args.paths.iter().any(|p| {
                full_path.starts_with(p.as_str()) || p.starts_with(&full_path)
            });
            if !matches {
                continue;
            }
        }

        let is_tree = entry.mode.is_tree();

        if is_tree && args.recurse {
            // When recursing, optionally show the tree entry itself
            if args.show_trees {
                print_entry(entry, &full_path, name_only, terminator, out)?;
            }
            list_tree(odb, &entry.oid, &full_path, args, name_only, terminator, out)?;
        } else if args.trees_only && !is_tree {
            continue;
        } else {
            print_entry(entry, &full_path, name_only, terminator, out)?;
        }
    }

    Ok(())
}

fn print_entry(
    entry: &git_object::TreeEntry,
    full_path: &str,
    name_only: bool,
    terminator: char,
    out: &mut impl Write,
) -> Result<()> {
    if name_only {
        write!(out, "{}{}", full_path, terminator)?;
    } else {
        let type_name = if entry.mode.is_tree() {
            "tree"
        } else if entry.mode.is_gitlink() {
            "commit"
        } else {
            "blob"
        };
        write!(
            out,
            "{:06o} {} {}\t{}{}",
            entry.mode.raw(),
            type_name,
            entry.oid.to_hex(),
            full_path,
            terminator,
        )?;
    }
    Ok(())
}

use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat, DiffResult, FileDiff, FileStatus};
use git_hash::ObjectId;
use git_object::{Object, TreeEntry};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DiffTreeArgs {
    /// Recurse into subtrees
    #[arg(short = 'r')]
    recursive: bool,

    /// Generate patch output
    #[arg(short = 'p', long = "patch")]
    patch: bool,

    /// Show raw diff output format
    #[arg(long)]
    raw: bool,

    /// Show only names of changed files
    #[arg(long)]
    name_only: bool,

    /// Show names and status of changed files
    #[arg(long)]
    name_status: bool,

    /// Show the initial tree (diff against empty tree)
    #[arg(long)]
    root: bool,

    /// First tree-ish to compare
    #[arg(value_name = "tree-ish")]
    tree_ish_a: String,

    /// Second tree-ish to compare (if omitted and --root is set, compares against empty tree)
    #[arg(value_name = "tree-ish")]
    tree_ish_b: Option<String>,
}

pub fn run(args: &DiffTreeArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let odb = repo.odb();

    // Resolve tree OIDs
    let (tree_a, tree_b) = if let Some(ref b) = args.tree_ish_b {
        let oid_a = resolve_to_tree(&repo, &args.tree_ish_a)?;
        let oid_b = resolve_to_tree(&repo, b)?;
        (Some(oid_a), Some(oid_b))
    } else if args.root {
        // --root: compare empty tree against the given tree
        let oid_a = resolve_to_tree(&repo, &args.tree_ish_a)?;
        (None, Some(oid_a))
    } else {
        // Single tree-ish: show the commit's diff (tree of parent vs tree of commit)
        let commit_oid = git_revwalk::resolve_revision(&repo, &args.tree_ish_a)?;
        let obj = odb
            .read(&commit_oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found: {}", commit_oid))?;
        match obj {
            Object::Commit(c) => {
                if c.parents.is_empty() {
                    // Initial commit: diff empty tree vs this commit's tree
                    (None, Some(c.tree))
                } else {
                    // Diff first parent's tree vs this commit's tree
                    let parent_obj = odb
                        .read(&c.parents[0])?
                        .ok_or_else(|| anyhow::anyhow!("parent not found"))?;
                    let parent_tree = match parent_obj {
                        Object::Commit(pc) => pc.tree,
                        _ => anyhow::bail!("parent is not a commit"),
                    };
                    (Some(parent_tree), Some(c.tree))
                }
            }
            Object::Tree(_) => {
                anyhow::bail!("need two tree-ish arguments, or use --root for a single tree")
            }
            _ => anyhow::bail!("not a commit or tree: {}", commit_oid),
        }
    };

    let diff_opts = DiffOptions {
        output_format: determine_output_format(args),
        ..DiffOptions::default()
    };

    let result = if args.recursive || args.patch {
        // Full recursive diff using the existing tree diff engine
        git_diff::tree::diff_trees(odb, tree_a.as_ref(), tree_b.as_ref(), &diff_opts)?
    } else {
        // Top-level only: compare only immediate children of the trees
        diff_trees_toplevel(odb, tree_a.as_ref(), tree_b.as_ref())?
    };

    let has_changes = !result.is_empty();

    if has_changes {
        let output = format_diff(&result, &diff_opts);
        write!(out, "{}", output)?;
    }

    Ok(if has_changes { 1 } else { 0 })
}

/// Compare only top-level entries of two trees (no recursion into subtrees).
fn diff_trees_toplevel(
    odb: &git_odb::ObjectDatabase,
    old_tree: Option<&ObjectId>,
    new_tree: Option<&ObjectId>,
) -> Result<DiffResult> {
    let old_entries = match old_tree {
        Some(oid) => read_tree(odb, oid)?,
        None => Vec::new(),
    };
    let new_entries = match new_tree {
        Some(oid) => read_tree(odb, oid)?,
        None => Vec::new(),
    };

    let mut files = Vec::new();
    let mut oi = 0;
    let mut ni = 0;

    while oi < old_entries.len() || ni < new_entries.len() {
        match (old_entries.get(oi), new_entries.get(ni)) {
            (Some(old_entry), Some(new_entry)) => {
                let cmp = TreeEntry::cmp_entries(old_entry, new_entry);
                match cmp {
                    std::cmp::Ordering::Less => {
                        // Only in old -> deleted
                        files.push(make_deleted_entry(old_entry, &BString::from("")));
                        oi += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        // Only in new -> added
                        files.push(make_added_entry(new_entry, &BString::from("")));
                        ni += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        if old_entry.oid != new_entry.oid || old_entry.mode != new_entry.mode {
                            files.push(make_modified_entry(
                                old_entry,
                                new_entry,
                                &BString::from(""),
                            ));
                        }
                        oi += 1;
                        ni += 1;
                    }
                }
            }
            (Some(old_entry), None) => {
                files.push(make_deleted_entry(old_entry, &BString::from("")));
                oi += 1;
            }
            (None, Some(new_entry)) => {
                files.push(make_added_entry(new_entry, &BString::from("")));
                ni += 1;
            }
            (None, None) => break,
        }
    }

    Ok(DiffResult { files })
}

fn make_deleted_entry(entry: &TreeEntry, prefix: &BString) -> FileDiff {
    let path = full_path(prefix, &entry.name);
    FileDiff {
        status: FileStatus::Deleted,
        old_path: Some(path),
        new_path: None,
        old_mode: Some(entry.mode),
        new_mode: None,
        old_oid: Some(entry.oid),
        new_oid: None,
        hunks: Vec::new(),
        is_binary: false,
        similarity: None,
    }
}

fn make_added_entry(entry: &TreeEntry, prefix: &BString) -> FileDiff {
    let path = full_path(prefix, &entry.name);
    FileDiff {
        status: FileStatus::Added,
        old_path: None,
        new_path: Some(path),
        old_mode: None,
        new_mode: Some(entry.mode),
        old_oid: None,
        new_oid: Some(entry.oid),
        hunks: Vec::new(),
        is_binary: false,
        similarity: None,
    }
}

fn make_modified_entry(
    old_entry: &TreeEntry,
    new_entry: &TreeEntry,
    prefix: &BString,
) -> FileDiff {
    let path = full_path(prefix, &old_entry.name);
    let status = if old_entry.mode != new_entry.mode
        && !(old_entry.mode.is_blob() && new_entry.mode.is_blob())
    {
        FileStatus::TypeChanged
    } else {
        FileStatus::Modified
    };
    FileDiff {
        status,
        old_path: Some(path.clone()),
        new_path: Some(path),
        old_mode: Some(old_entry.mode),
        new_mode: Some(new_entry.mode),
        old_oid: Some(old_entry.oid),
        new_oid: Some(new_entry.oid),
        hunks: Vec::new(),
        is_binary: false,
        similarity: None,
    }
}

fn full_path(prefix: &BString, name: &BString) -> BString {
    if prefix.is_empty() {
        name.clone()
    } else {
        let mut p = prefix.clone();
        p.push(b'/');
        p.extend_from_slice(name);
        p
    }
}

fn read_tree(odb: &git_odb::ObjectDatabase, oid: &ObjectId) -> Result<Vec<TreeEntry>> {
    let obj = odb
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found: {}", oid))?;
    match obj {
        Object::Tree(t) => Ok(t.entries),
        _ => anyhow::bail!("not a tree: {}", oid),
    }
}

fn resolve_to_tree(repo: &git_repository::Repository, rev: &str) -> Result<ObjectId> {
    let oid = git_revwalk::resolve_revision(repo, rev)?;
    let obj = repo
        .odb()
        .read(&oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;
    match obj {
        Object::Commit(c) => Ok(c.tree),
        Object::Tree(_) => Ok(oid),
        _ => anyhow::bail!("not a commit or tree: {}", oid),
    }
}

fn determine_output_format(args: &DiffTreeArgs) -> DiffOutputFormat {
    if args.patch {
        DiffOutputFormat::Unified
    } else if args.name_only {
        DiffOutputFormat::NameOnly
    } else if args.name_status {
        DiffOutputFormat::NameStatus
    } else {
        // Default for diff-tree is raw format
        DiffOutputFormat::Raw
    }
}

use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_hash::ObjectId;
use git_object::{Commit, Object};
use git_revwalk::{format_builtin, format_commit, BuiltinFormat, FormatOptions};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct ShowArgs {
    /// Format (oneline, short, medium, full, fuller, raw)
    #[arg(long)]
    format: Option<String>,

    /// Show diffstat
    #[arg(long)]
    stat: bool,

    /// Show name-only
    #[arg(long)]
    name_only: bool,

    /// Show name-status
    #[arg(long)]
    name_status: bool,

    /// Don't show diff
    #[arg(short = 's', long)]
    no_patch: bool,

    /// Object to show (defaults to HEAD)
    #[arg(default_value = "HEAD")]
    object: String,
}

pub fn run(args: &ShowArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Handle tree:path syntax (e.g., HEAD:file.txt)
    if args.object.contains(':') {
        return show_tree_path(&repo, &args.object, &mut out);
    }

    let oid = git_revwalk::resolve_revision(&repo, &args.object)?;
    let obj = repo
        .odb()
        .read(&oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;

    match obj {
        Object::Commit(commit) => show_commit(&repo, &commit, &oid, args, &mut out)?,
        Object::Tag(tag) => show_tag(&repo, &tag, &oid, &mut out)?,
        Object::Tree(tree) => show_tree(&tree, &oid, &mut out)?,
        Object::Blob(blob) => show_blob(&blob, &mut out)?,
    }

    Ok(0)
}

fn show_commit(
    repo: &git_repository::Repository,
    commit: &Commit,
    oid: &ObjectId,
    args: &ShowArgs,
    out: &mut impl Write,
) -> Result<()> {
    let format_options = FormatOptions { abbrev_len: 40, ..FormatOptions::default() };

    // Determine format
    let fmt_str = args.format.as_deref();
    let (preset, custom_format) = match fmt_str {
        Some("oneline") => (BuiltinFormat::Oneline, None),
        Some("short") => (BuiltinFormat::Short, None),
        Some("full") => (BuiltinFormat::Full, None),
        Some("fuller") => (BuiltinFormat::Fuller, None),
        Some("raw") => (BuiltinFormat::Raw, None),
        Some("medium") | None => (BuiltinFormat::Medium, None),
        Some(custom) => {
            let fmt = if let Some(stripped) = custom.strip_prefix("format:") {
                stripped
            } else if let Some(stripped) = custom.strip_prefix("tformat:") {
                stripped
            } else {
                custom
            };
            (BuiltinFormat::Medium, Some(fmt.to_string()))
        }
    };

    if let Some(ref fmt) = custom_format {
        let formatted = format_commit(commit, oid, fmt, &format_options);
        write!(out, "{}", formatted)?;
        writeln!(out)?;
    } else {
        // For medium/full/fuller: add merge header if merge commit
        if commit.parents.len() > 1
            && matches!(
                preset,
                BuiltinFormat::Medium | BuiltinFormat::Full | BuiltinFormat::Fuller
            )
        {
            // The builtin format starts with "commit <oid>\n".
            // We need to inject "Merge: <parent1> <parent2>" after that line.
            let formatted = format_builtin(commit, oid, preset, &format_options);
            let mut lines = formatted.splitn(2, '\n');
            if let Some(first_line) = lines.next() {
                writeln!(out, "{}", first_line)?;
                // Insert Merge: header
                let short_parents: Vec<String> = commit
                    .parents
                    .iter()
                    .map(|p| p.to_hex()[..7].to_string())
                    .collect();
                writeln!(out, "Merge: {}", short_parents.join(" "))?;
                if let Some(rest) = lines.next() {
                    write!(out, "{}", rest)?;
                }
            }
        } else {
            let formatted = format_builtin(commit, oid, preset, &format_options);
            write!(out, "{}", formatted)?;
        }

        if preset == BuiltinFormat::Oneline {
            writeln!(out)?;
        }
    }

    // Show diff unless --no-patch
    if !args.no_patch && custom_format.is_none() {
        let parent_tree = if let Some(parent_oid) = commit.first_parent() {
            let parent_obj = repo.odb().read(parent_oid)?;
            match parent_obj {
                Some(Object::Commit(pc)) => Some(pc.tree),
                _ => None,
            }
        } else {
            None
        };

        let mut diff_opts = DiffOptions::default();
        if args.stat {
            diff_opts.output_format = DiffOutputFormat::Stat;
        } else if args.name_only {
            diff_opts.output_format = DiffOutputFormat::NameOnly;
        } else if args.name_status {
            diff_opts.output_format = DiffOutputFormat::NameStatus;
        }

        let result = git_diff::tree::diff_trees(
            repo.odb(),
            parent_tree.as_ref(),
            Some(&commit.tree),
            &diff_opts,
        )?;

        if !result.is_empty() {
            let output = format_diff(&result, &diff_opts);
            write!(out, "\n{}", output)?;
        }
    }

    Ok(())
}

fn show_tag(
    repo: &git_repository::Repository,
    tag: &git_object::Tag,
    _oid: &ObjectId,
    out: &mut impl Write,
) -> Result<()> {
    writeln!(
        out,
        "tag {}",
        String::from_utf8_lossy(&tag.tag_name)
    )?;
    if let Some(ref tagger) = tag.tagger {
        writeln!(
            out,
            "Tagger: {} <{}>",
            String::from_utf8_lossy(&tagger.name),
            String::from_utf8_lossy(&tagger.email)
        )?;
        writeln!(
            out,
            "Date:   {}",
            tagger.date.format(&git_utils::date::DateFormat::Default)
        )?;
    }
    writeln!(out)?;
    for line in tag.message.lines() {
        writeln!(out, "{}", String::from_utf8_lossy(line))?;
    }
    writeln!(out)?;

    // Also show the tagged object
    let target_obj = repo
        .odb()
        .read(&tag.target)?
        .ok_or_else(|| anyhow::anyhow!("tagged object not found: {}", tag.target))?;

    if let Object::Commit(commit) = target_obj {
        let format_options = FormatOptions::default();
        let formatted =
            format_builtin(&commit, &tag.target, BuiltinFormat::Medium, &format_options);
        write!(out, "{}", formatted)?;
    }

    Ok(())
}

fn show_tree(tree: &git_object::Tree, _oid: &ObjectId, out: &mut impl Write) -> Result<()> {
    for entry in &tree.entries {
        writeln!(
            out,
            "{:06o} {} {}\t{}",
            entry.mode.raw(),
            if entry.mode.is_tree() { "tree" } else { "blob" },
            entry.oid.to_hex(),
            String::from_utf8_lossy(&entry.name)
        )?;
    }
    Ok(())
}

fn show_blob(blob: &git_object::Blob, out: &mut impl Write) -> Result<()> {
    out.write_all(&blob.data)?;
    Ok(())
}

fn show_tree_path(
    repo: &git_repository::Repository,
    spec: &str,
    out: &mut impl Write,
) -> Result<i32> {
    let (rev_part, path_part) = spec.split_once(':').unwrap();

    let rev = if rev_part.is_empty() { "HEAD" } else { rev_part };
    let commit_oid = git_revwalk::resolve_revision(repo, rev)?;

    let commit_obj = repo
        .odb()
        .read(&commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", commit_oid))?;

    let tree_oid = match commit_obj {
        Object::Commit(c) => c.tree,
        _ => anyhow::bail!("not a commit: {}", commit_oid),
    };

    // Walk the tree path
    let path_oid = resolve_tree_path(repo, &tree_oid, path_part)?;

    let obj = repo
        .odb()
        .read(&path_oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", path_oid))?;

    match obj {
        Object::Blob(blob) => {
            out.write_all(&blob.data)?;
        }
        Object::Tree(tree) => {
            show_tree(&tree, &path_oid, out)?;
        }
        _ => {
            anyhow::bail!("unexpected object type at path '{}'", path_part);
        }
    }

    Ok(0)
}

fn resolve_tree_path(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    path: &str,
) -> Result<ObjectId> {
    if path.is_empty() {
        return Ok(*tree_oid);
    }

    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut current_oid = *tree_oid;

    for component in &components {
        let obj = repo
            .odb()
            .read(&current_oid)?
            .ok_or_else(|| anyhow::anyhow!("tree not found: {}", current_oid))?;

        let tree = match obj {
            Object::Tree(t) => t,
            _ => anyhow::bail!("not a tree: {}", current_oid),
        };

        let entry = tree
            .entries
            .iter()
            .find(|e| e.name.as_bstr() == component.as_bytes().as_bstr())
            .ok_or_else(|| anyhow::anyhow!("path '{}' not found in tree", component))?;

        current_oid = entry.oid;
    }

    Ok(current_oid)
}

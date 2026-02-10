use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_hash::ObjectId;
use git_object::{Commit, Object};
use git_revwalk::{
    format_builtin, format_builtin_with_decorations, format_commit_with_decorations,
    BuiltinFormat, FormatOptions,
};
use git_utils::color::{ColorConfig, ColorSlot};

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

    /// When to show colored output (auto, always, never)
    #[arg(long, value_name = "when")]
    color: Option<String>,

    /// Show ref decorations on commits
    #[arg(long)]
    decorate: bool,

    /// Suppress diff output (same as --no-patch)
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Object to show (defaults to HEAD)
    #[arg(default_value = "HEAD")]
    object: String,
}

pub fn run(args: &ShowArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Determine color settings
    let color_config = load_color_config(&repo);
    let cli_color = args.color.as_deref().map(git_utils::color::parse_color_mode);
    let effective = color_config.effective_mode("show", cli_color);
    let color_on = git_utils::color::use_color(effective, io::stdout().is_terminal());

    // Build decoration map if --decorate is set
    let decorations = if args.decorate {
        Some(super::log::build_decoration_map_for_show(&repo)?)
    } else {
        None
    };

    // Handle tree:path syntax (e.g., HEAD:file.txt)
    if args.object.contains(':') {
        return show_tree_path(&repo, &args.object, &mut out);
    }

    let oid = git_revwalk::resolve_revision(&repo, &args.object)?;
    let obj = repo
        .odb()
        .read(&oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;

    // Capture output into a buffer, then colorize if needed
    let mut buf = Vec::new();
    match obj {
        Object::Commit(commit) => {
            show_commit(&repo, &commit, &oid, args, decorations.as_ref(), &mut buf)?;
        }
        Object::Tag(tag) => show_tag(&repo, &tag, &oid, &mut buf)?,
        Object::Tree(tree) => show_tree(&tree, &oid, &mut buf)?,
        Object::Blob(blob) => show_blob(&blob, &mut buf)?,
    }

    if color_on {
        let text = String::from_utf8_lossy(&buf);
        let mut in_diff = false;
        let lines: Vec<&str> = text.split('\n').collect();
        // split('\n') on "a\nb\n" gives ["a", "b", ""], so skip trailing empty
        let count = if lines.last() == Some(&"") {
            lines.len() - 1
        } else {
            lines.len()
        };
        for line in &lines[..count] {
            let colored = colorize_show_line(line, &color_config, &mut in_diff);
            writeln!(out, "{}", colored)?;
        }
    } else {
        out.write_all(&buf)?;
    }

    Ok(0)
}

fn show_commit(
    repo: &git_repository::Repository,
    commit: &Commit,
    oid: &ObjectId,
    args: &ShowArgs,
    decorations: Option<&std::collections::HashMap<ObjectId, Vec<String>>>,
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
        let formatted =
            format_commit_with_decorations(commit, oid, fmt, &format_options, decorations);
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
            let formatted = format_builtin_with_decorations(
                commit,
                oid,
                preset,
                &format_options,
                decorations,
            );
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
            let formatted = format_builtin_with_decorations(
                commit,
                oid,
                preset,
                &format_options,
                decorations,
            );
            write!(out, "{}", formatted)?;
        }

        if preset == BuiltinFormat::Oneline {
            writeln!(out)?;
        }
    }

    // Show diff unless --no-patch or --quiet
    let suppress_diff = args.no_patch || args.quiet;
    if !suppress_diff && custom_format.is_none() {
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

/// Load color configuration from the repository config (best-effort).
fn load_color_config(repo: &git_repository::Repository) -> ColorConfig {
    let config = repo.config();
    ColorConfig::from_config(|key| config.get_string(key).ok().flatten())
}

/// Colorize a single line of `show` output (commit header + diff).
fn colorize_show_line(line: &str, cc: &ColorConfig, in_diff: &mut bool) -> String {
    let reset = cc.get_color(ColorSlot::Reset);

    if line.starts_with("commit ") && !*in_diff {
        let hash_color = "\x1b[33m"; // yellow, matching C git
        return format!("{}{}{}", hash_color, line, reset);
    }

    if line.starts_with("diff --git") {
        *in_diff = true;
        return format!("{}{}{}", cc.get_color(ColorSlot::DiffMetaInfo), line, reset);
    }

    if *in_diff {
        if line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("index ")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("new file")
            || line.starts_with("deleted file")
            || line.starts_with("similarity")
            || line.starts_with("rename")
            || line.starts_with("copy")
        {
            return format!("{}{}{}", cc.get_color(ColorSlot::DiffMetaInfo), line, reset);
        }
        if line.starts_with("@@") {
            return format!("{}{}{}", cc.get_color(ColorSlot::DiffFragInfo), line, reset);
        }
        if line.starts_with('-') {
            return format!("{}{}{}", cc.get_color(ColorSlot::DiffOldNormal), line, reset);
        }
        if line.starts_with('+') {
            return format!("{}{}{}", cc.get_color(ColorSlot::DiffNewNormal), line, reset);
        }
    }

    line.to_string()
}

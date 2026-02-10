use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use regex::Regex;
use git_utils::color::{self, ColorConfig, ColorSlot};

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

    /// When to show colored output (auto, always, never)
    #[arg(long, value_name = "when")]
    color: Option<String>,

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

    let cli_color = args.color.as_deref().map(color::parse_color_mode);
    let color_config = load_color_config(cli);
    let effective = color_config.effective_mode("grep", cli_color);
    let color_on = color::use_color(effective, io::stdout().is_terminal());

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
    grep_tree(&repo, &tree_oid, "", &regex, args, color_on, &color_config, &mut out, &mut found)?;

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

#[allow(clippy::too_many_arguments)]
fn grep_tree(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    prefix: &str,
    regex: &Regex,
    args: &GrepArgs,
    color_on: bool,
    cc: &ColorConfig,
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
            grep_tree(repo, &entry.oid, &path, regex, args, color_on, cc, out, found)?;
        } else if entry.mode.is_blob() {
            // Check pathspec filter
            if !args.pathspecs.is_empty() {
                let matches = args.pathspecs.iter().any(|p| path.starts_with(p));
                if !matches {
                    continue;
                }
            }

            grep_blob(repo, &entry.oid, &path, regex, args, color_on, cc, out, found)?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn grep_blob(
    repo: &git_repository::Repository,
    blob_oid: &ObjectId,
    path: &str,
    regex: &Regex,
    args: &GrepArgs,
    color_on: bool,
    cc: &ColorConfig,
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

    let reset = if color_on { cc.get_color(ColorSlot::Reset) } else { "" };
    let filename_color = if color_on { cc.get_color(ColorSlot::GrepFilename) } else { "" };
    let linenum_color = if color_on { cc.get_color(ColorSlot::GrepLineNumber) } else { "" };
    let sep_color = if color_on { cc.get_color(ColorSlot::GrepSeparator) } else { "" };

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
                let colored_content = if color_on && !args.invert_match {
                    highlight_matches(line, regex, cc)
                } else {
                    line.to_string()
                };

                if args.line_number {
                    writeln!(
                        out,
                        "{}{}{}{}:{}{}{}{}{}:{}{}",
                        filename_color, path, reset,
                        sep_color, reset,
                        linenum_color, line_num + 1, reset,
                        sep_color, reset,
                        colored_content,
                    )?;
                } else {
                    writeln!(
                        out,
                        "{}{}{}{}:{}{}",
                        filename_color, path, reset,
                        sep_color, reset,
                        colored_content,
                    )?;
                }
            }
        }
    }

    if args.count && file_matched {
        writeln!(
            out,
            "{}{}{}{}:{}{}",
            filename_color, path, reset,
            sep_color, reset,
            match_count,
        )?;
    }

    Ok(())
}

fn highlight_matches(text: &str, regex: &Regex, cc: &ColorConfig) -> String {
    let match_color = cc.get_color(ColorSlot::GrepMatch);
    let reset = cc.get_color(ColorSlot::Reset);
    regex
        .replace_all(text, |caps: &regex::Captures| {
            format!("{}{}{}", match_color, &caps[0], reset)
        })
        .into_owned()
}

fn load_color_config(cli: &Cli) -> ColorConfig {
    let config = if let Some(ref git_dir) = cli.git_dir {
        git_config::ConfigSet::load(Some(git_dir)).ok()
    } else {
        git_repository::Repository::discover(".")
            .ok()
            .and_then(|repo| git_config::ConfigSet::load(Some(repo.git_dir())).ok())
    };
    match config {
        Some(c) => ColorConfig::from_config(|key| c.get_string(key).ok().flatten()),
        None => ColorConfig::new(),
    }
}

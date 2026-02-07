use std::io::{self, BufRead, Write};

use anyhow::Result;
use bstr::BStr;
use clap::Args;
use git_utils::wildmatch::{WildmatchFlags, WildmatchPattern};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CheckIgnoreArgs {
    /// Be verbose (show matching pattern)
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Show non-matching paths too
    #[arg(short = 'n', long)]
    non_matching: bool,

    /// Read paths from stdin
    #[arg(long)]
    stdin: bool,

    /// NUL line terminator
    #[arg(short = 'z')]
    nul_terminated: bool,

    /// Paths to check
    #[arg(value_name = "pathname")]
    pathnames: Vec<String>,
}

pub fn run(args: &CheckIgnoreArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let terminator = if args.nul_terminated { '\0' } else { '\n' };

    // Load ignore patterns from .gitignore files
    let patterns = load_ignore_patterns(&repo)?;

    let mut paths: Vec<String> = args.pathnames.clone();
    if args.stdin {
        let stdin_handle = io::stdin();
        for line in stdin_handle.lock().lines() {
            let line = line?;
            let line = line.trim().to_string();
            if !line.is_empty() {
                paths.push(line);
            }
        }
    }

    let mut found_ignored = false;

    for path in &paths {
        let bpath = BStr::new(path.as_bytes());
        let mut is_ignored = false;
        let mut matching_pattern: Option<&str> = None;
        let mut matching_source: Option<&str> = None;
        let mut matching_linenum: Option<usize> = None;

        for (source, linenum, pattern, negated) in &patterns {
            let wm = WildmatchPattern::new(BStr::new(pattern.as_bytes()), WildmatchFlags::PATHNAME);
            if wm.matches(bpath) {
                if *negated {
                    is_ignored = false;
                    matching_pattern = None;
                    matching_source = None;
                    matching_linenum = None;
                } else {
                    is_ignored = true;
                    matching_pattern = Some(pattern);
                    matching_source = Some(source);
                    matching_linenum = Some(*linenum);
                }
            }
        }

        if is_ignored {
            found_ignored = true;
            if args.verbose {
                write!(
                    out,
                    "{}:{}:{}\t{}{}",
                    matching_source.unwrap_or(""),
                    matching_linenum.map(|n| n.to_string()).unwrap_or_default(),
                    matching_pattern.unwrap_or(""),
                    path,
                    terminator,
                )?;
            } else {
                write!(out, "{}{}", path, terminator)?;
            }
        } else if args.non_matching {
            if args.verbose {
                write!(out, "::\t{}{}", path, terminator)?;
            } else {
                write!(out, "{}{}", path, terminator)?;
            }
        }
    }

    if found_ignored { Ok(0) } else { Ok(1) }
}

/// Load .gitignore patterns from the repository.
/// Returns (source_file, line_number, pattern, is_negated) tuples.
fn load_ignore_patterns(repo: &git_repository::Repository) -> Result<Vec<(String, usize, String, bool)>> {
    let mut patterns = Vec::new();

    // Load from .gitignore in worktree root
    if let Some(wt) = repo.work_tree() {
        let gitignore = wt.join(".gitignore");
        if gitignore.exists() {
            let content = std::fs::read_to_string(&gitignore)?;
            let source = ".gitignore".to_string();
            for (i, line) in content.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (pattern, negated) = if let Some(rest) = line.strip_prefix('!') {
                    (rest.to_string(), true)
                } else {
                    (line.to_string(), false)
                };
                patterns.push((source.clone(), i + 1, pattern, negated));
            }
        }
    }

    // Load from .git/info/exclude
    let exclude = repo.git_dir().join("info").join("exclude");
    if exclude.exists() {
        let content = std::fs::read_to_string(&exclude)?;
        let source = ".git/info/exclude".to_string();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (pattern, negated) = if let Some(rest) = line.strip_prefix('!') {
                (rest.to_string(), true)
            } else {
                (line.to_string(), false)
            };
            patterns.push((source.clone(), i + 1, pattern, negated));
        }
    }

    Ok(patterns)
}

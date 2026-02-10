use std::io::{self, BufRead, Write};

use anyhow::Result;
use clap::Args;

use crate::Cli;

#[derive(Args)]
pub struct StripspaceArgs {
    /// Remove lines starting with #
    #[arg(short = 's', long)]
    strip_comments: bool,

    /// Prefix each line with "# "
    #[arg(short = 'c', long)]
    comment_lines: bool,
}

pub fn run(args: &StripspaceArgs, _cli: &Cli) -> Result<i32> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.comment_lines {
        // Comment-lines mode: prefix each line with "# "
        for line in stdin.lock().lines() {
            let line = line?;
            if line.is_empty() {
                writeln!(out, "#")?;
            } else {
                writeln!(out, "# {}", line)?;
            }
        }
        return Ok(0);
    }

    // Default / strip-comments mode: read all input, then process
    let mut lines: Vec<String> = Vec::new();
    for line in stdin.lock().lines() {
        let line = line?;
        lines.push(line);
    }

    // Strip trailing whitespace from each line
    let mut processed: Vec<String> = lines
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect();

    // If --strip-comments, remove lines starting with '#'
    if args.strip_comments {
        processed.retain(|l| !l.starts_with('#'));
    }

    // Collapse consecutive blank lines into a single blank line
    let mut collapsed: Vec<String> = Vec::new();
    let mut prev_blank = false;
    for line in processed {
        let is_blank = line.is_empty();
        if is_blank {
            if !prev_blank {
                collapsed.push(line);
            }
            prev_blank = true;
        } else {
            collapsed.push(line);
            prev_blank = false;
        }
    }

    // Strip leading blank lines
    while collapsed.first().is_some_and(|l| l.is_empty()) {
        collapsed.remove(0);
    }

    // Strip trailing blank lines
    while collapsed.last().is_some_and(|l| l.is_empty()) {
        collapsed.pop();
    }

    // Output
    for line in &collapsed {
        writeln!(out, "{}", line)?;
    }

    Ok(0)
}

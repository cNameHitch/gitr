use std::io::{self, BufRead, Write};

use anyhow::Result;
use bstr::BStr;
use clap::Args;
use git_utils::wildmatch::{WildmatchFlags, WildmatchPattern};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CheckAttrArgs {
    /// Show all attributes
    #[arg(short = 'a', long)]
    all: bool,

    /// Read paths from stdin
    #[arg(long)]
    stdin: bool,

    /// NUL line terminator
    #[arg(short = 'z')]
    nul_terminated: bool,

    /// Attribute names and paths (attrs first, then -- separator, then paths)
    #[arg(value_name = "attr", trailing_var_arg = true)]
    args: Vec<String>,
}

pub fn run(args: &CheckAttrArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let terminator = if args.nul_terminated { '\0' } else { '\n' };

    // Parse attrs and paths from args
    let (attrs, paths) = parse_attr_args(&args.args);

    let mut all_paths: Vec<String> = paths;
    if args.stdin {
        let stdin_handle = io::stdin();
        for line in stdin_handle.lock().lines() {
            let line = line?;
            let line = line.trim().to_string();
            if !line.is_empty() {
                all_paths.push(line);
            }
        }
    }

    // Load .gitattributes
    let attribute_rules = load_gitattributes(&repo)?;

    for path in &all_paths {
        let bpath = BStr::new(path.as_bytes());

        let attrs_to_check: Vec<&str> = if args.all {
            // Only collect attrs from rules whose pattern matches this path
            let mut matched_attrs = Vec::new();
            for (pattern, rules) in &attribute_rules {
                let wm = WildmatchPattern::new(BStr::new(pattern.as_bytes()), WildmatchFlags::PATHNAME);
                if wm.matches(bpath) {
                    for (a, _) in rules {
                        matched_attrs.push(a.as_str());
                    }
                }
            }
            matched_attrs
        } else {
            attrs.iter().map(|s| s.as_str()).collect()
        };

        // Deduplicate attrs
        let mut unique_attrs: Vec<&str> = Vec::new();
        for a in &attrs_to_check {
            if !unique_attrs.contains(a) {
                unique_attrs.push(a);
            }
        }

        for attr in &unique_attrs {
            let value = get_attr_value(path, attr, &attribute_rules);
            write!(
                out,
                "{}: {}: {}{}",
                path,
                attr,
                value.as_deref().unwrap_or("unspecified"),
                terminator,
            )?;
        }
    }

    Ok(0)
}

fn parse_attr_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    if let Some(sep_pos) = args.iter().position(|a| a == "--") {
        let attrs = args[..sep_pos].to_vec();
        let paths = args[sep_pos + 1..].to_vec();
        (attrs, paths)
    } else if args.len() >= 2 {
        // Last arg is the path, rest are attrs
        let attrs = args[..args.len() - 1].to_vec();
        let paths = vec![args.last().unwrap().clone()];
        (attrs, paths)
    } else {
        (Vec::new(), args.to_vec())
    }
}

type AttrRule = (String, Vec<(String, String)>);

fn load_gitattributes(
    repo: &git_repository::Repository,
) -> Result<Vec<AttrRule>> {
    let mut rules = Vec::new();

    if let Some(wt) = repo.work_tree() {
        let gitattributes = wt.join(".gitattributes");
        if gitattributes.exists() {
            let content = std::fs::read_to_string(&gitattributes)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let pattern = parts[0].to_string();
                    let attrs: Vec<(String, String)> = parts[1..]
                        .iter()
                        .map(|a| {
                            if let Some((key, val)) = a.split_once('=') {
                                (key.to_string(), val.to_string())
                            } else if let Some(key) = a.strip_prefix('-') {
                                (key.to_string(), "unset".to_string())
                            } else {
                                (a.to_string(), "set".to_string())
                            }
                        })
                        .collect();
                    rules.push((pattern, attrs));
                }
            }
        }
    }

    Ok(rules)
}

fn get_attr_value(
    path: &str,
    attr: &str,
    rules: &[AttrRule],
) -> Option<String> {
    let bpath = BStr::new(path.as_bytes());

    // Check rules in reverse order (last match wins)
    for (pattern, attrs) in rules.iter().rev() {
        let wm = WildmatchPattern::new(BStr::new(pattern.as_bytes()), WildmatchFlags::PATHNAME);
        if wm.matches(bpath) {
            for (name, value) in attrs {
                if name == attr {
                    return Some(value.clone());
                }
            }
        }
    }

    None
}

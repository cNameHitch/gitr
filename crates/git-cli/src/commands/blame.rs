use std::collections::HashMap;
use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::algorithm::{diff_edits, EditOp};
use git_hash::ObjectId;
use git_object::{Commit, Object};
use git_revwalk::RevWalk;
use git_utils::date::DateFormat;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct BlameArgs {
    /// Show line range (start,end)
    #[arg(short = 'L')]
    line_range: Option<String>,

    /// Detect lines copied from other files
    #[arg(long)]
    detect_copies: bool,

    /// Ignore whitespace changes
    #[arg(short = 'w')]
    ignore_whitespace: bool,

    /// Output in porcelain format
    #[arg(long)]
    porcelain: bool,

    /// Show line numbers
    #[arg(short = 'n', long)]
    show_number: bool,

    /// Show email instead of author name
    #[arg(short = 'e', long)]
    show_email: bool,

    /// Revision to blame from
    #[arg(long, value_name = "rev")]
    rev: Option<String>,

    /// File to blame
    file: String,
}

/// A blame entry: which commit last changed a range of lines.
struct BlameEntry {
    commit: ObjectId,
    original_line: u32,
    final_line: u32,
    num_lines: u32,
}

pub fn run(args: &BlameArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Resolve starting revision
    let start_oid = if let Some(ref rev) = args.rev {
        git_revwalk::resolve_revision(&repo, rev)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD does not point to a valid object"))?
    };

    // Read the file at the starting revision
    let file_lines = read_file_at_rev(&repo, &start_oid, &args.file)?;

    if file_lines.is_empty() {
        return Ok(0);
    }

    // Parse line range
    let (start_line, end_line) = if let Some(ref range) = args.line_range {
        parse_line_range(range, file_lines.len())?
    } else {
        (1, file_lines.len())
    };

    // Run blame algorithm
    let entries = blame_file(&repo, &start_oid, &args.file, &file_lines)?;

    // Cache commit data for output
    let mut commit_cache: HashMap<ObjectId, Commit> = HashMap::new();

    // Compute column widths
    let max_line_num = end_line;
    let line_width = format!("{}", max_line_num).len();

    for entry in &entries {
        if let std::collections::hash_map::Entry::Vacant(e) = commit_cache.entry(entry.commit) {
            if let Some(Object::Commit(c)) = repo.odb().read(&entry.commit)? {
                e.insert(c);
            }
        }
    }

    // Find longest author name for alignment
    let max_author_len = commit_cache
        .values()
        .map(|c| String::from_utf8_lossy(&c.author.name).len())
        .max()
        .unwrap_or(10);

    // Output blame
    for entry in &entries {
        for i in 0..entry.num_lines {
            let line_num = entry.final_line + i;
            if line_num < start_line as u32 || line_num > end_line as u32 {
                continue;
            }

            let line_idx = (line_num - 1) as usize;
            let line_content = file_lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");

            if args.porcelain {
                write_porcelain_entry(
                    &entry.commit,
                    entry.original_line + i,
                    line_num,
                    line_content,
                    commit_cache.get(&entry.commit),
                    &mut out,
                )?;
            } else {
                let hex = entry.commit.to_hex();

                // Check if this is a boundary commit (root commit with no parents)
                let is_boundary = commit_cache
                    .get(&entry.commit)
                    .map(|c| c.parents.is_empty())
                    .unwrap_or(false);
                // Boundary commits: "^" + 7 chars = 8 total
                // Non-boundary:     8 chars       = 8 total
                let (prefix, short_display) = if is_boundary {
                    ("^", &hex[..7.min(hex.len())])
                } else {
                    ("", &hex[..8.min(hex.len())])
                };

                if let Some(commit) = commit_cache.get(&entry.commit) {
                    let author = if args.show_email {
                        format!("<{}>", String::from_utf8_lossy(&commit.author.email))
                    } else {
                        String::from_utf8_lossy(&commit.author.name).to_string()
                    };
                    let date = commit.author.date.format(&DateFormat::Iso);
                    // Show full date+time+tz: "YYYY-MM-DD HH:MM:SS <tz>"
                    let date_display = &date[..date.len().min(25)];

                    write!(
                        out,
                        "{}{} ({:>width$} {} {:>lw$}) ",
                        prefix,
                        short_display,
                        author,
                        date_display,
                        line_num,
                        width = max_author_len,
                        lw = line_width,
                    )?;
                } else {
                    write!(
                        out,
                        "{}{} ({:>lw$}) ",
                        prefix,
                        short_display,
                        line_num,
                        lw = line_width,
                    )?;
                }

                writeln!(out, "{}", line_content)?;
            }
        }
    }

    Ok(0)
}

fn write_porcelain_entry(
    oid: &ObjectId,
    original_line: u32,
    final_line: u32,
    content: &str,
    commit: Option<&Commit>,
    out: &mut impl Write,
) -> Result<()> {
    writeln!(out, "{} {} {} 1", oid.to_hex(), original_line, final_line)?;

    if let Some(c) = commit {
        writeln!(
            out,
            "author {}",
            String::from_utf8_lossy(&c.author.name)
        )?;
        writeln!(
            out,
            "author-mail <{}>",
            String::from_utf8_lossy(&c.author.email)
        )?;
        writeln!(out, "author-time {}", c.author.date.timestamp)?;
        writeln!(
            out,
            "author-tz {}",
            format_tz(c.author.date.tz_offset)
        )?;
        writeln!(
            out,
            "committer {}",
            String::from_utf8_lossy(&c.committer.name)
        )?;
        writeln!(
            out,
            "committer-mail <{}>",
            String::from_utf8_lossy(&c.committer.email)
        )?;
        writeln!(out, "committer-time {}", c.committer.date.timestamp)?;
        writeln!(
            out,
            "committer-tz {}",
            format_tz(c.committer.date.tz_offset)
        )?;
        writeln!(
            out,
            "summary {}",
            String::from_utf8_lossy(c.summary())
        )?;
    }

    writeln!(out, "\t{}", content)?;
    Ok(())
}

fn format_tz(offset_minutes: i32) -> String {
    let sign = if offset_minutes >= 0 { '+' } else { '-' };
    let abs = offset_minutes.unsigned_abs();
    let hours = abs / 60;
    let mins = abs % 60;
    format!("{}{:02}{:02}", sign, hours, mins)
}

/// Blame algorithm: walk backwards through history, attributing lines to commits.
fn blame_file(
    repo: &git_repository::Repository,
    start_oid: &ObjectId,
    path: &str,
    file_lines: &[String],
) -> Result<Vec<BlameEntry>> {
    let total_lines = file_lines.len() as u32;
    if total_lines == 0 {
        return Ok(Vec::new());
    }

    // Track which lines are still unblamed: (line_number, line_content)
    let mut unblamed: Vec<(u32, String)> = file_lines
        .iter()
        .enumerate()
        .map(|(i, l)| ((i + 1) as u32, l.clone()))
        .collect();

    let mut entries: Vec<BlameEntry> = Vec::new();

    // Walk commits
    let mut walker = RevWalk::new(repo)?;
    walker.push(*start_oid)?;

    for oid_result in walker {
        if unblamed.is_empty() {
            break;
        }

        let oid = oid_result?;
        let obj = repo.odb().read_cached(&oid)?;
        let commit = match obj {
            Some(Object::Commit(c)) => c,
            _ => continue,
        };

        // Read file content at this commit
        let current_content = match read_file_at_rev(repo, &oid, path) {
            Ok(lines) => lines,
            Err(_) => continue,
        };

        // Read file content at parent
        let parent_content = if let Some(parent_oid) = commit.first_parent() {
            read_file_at_rev(repo, parent_oid, path).unwrap_or_default()
        } else {
            // Root commit: all remaining lines belong to this commit
            Vec::new()
        };

        // Find lines that changed between parent and this commit using diff-based attribution
        let changed_lines = diff_blame_changed_set(&parent_content, &current_content);

        // Attribute unblamed lines that changed in this commit
        let mut newly_blamed = Vec::new();
        let mut blame_start: Option<u32> = None;
        let mut blame_count = 0u32;

        for (idx, (line_num, _content)) in unblamed.iter().enumerate() {
            let line_idx = (*line_num - 1) as usize;
            if line_idx < current_content.len() && changed_lines.contains(&line_idx) {
                if blame_start.is_none() {
                    blame_start = Some(*line_num);
                    blame_count = 1;
                } else {
                    blame_count += 1;
                }
                newly_blamed.push(idx);
            } else if let Some(start) = blame_start {
                entries.push(BlameEntry {
                    commit: oid,
                    original_line: start,
                    final_line: start,
                    num_lines: blame_count,
                });
                blame_start = None;
                blame_count = 0;
            }
        }

        if let Some(start) = blame_start {
            entries.push(BlameEntry {
                commit: oid,
                original_line: start,
                final_line: start,
                num_lines: blame_count,
            });
        }

        // Remove blamed lines from unblamed (in reverse order to maintain indices)
        for idx in newly_blamed.iter().rev() {
            unblamed.remove(*idx);
        }

        // If this is a root commit, blame all remaining lines
        if commit.parents.is_empty() {
            if !unblamed.is_empty() {
                // Group consecutive lines
                let mut groups: Vec<(u32, u32)> = Vec::new();
                for (line_num, _) in &unblamed {
                    if let Some(last) = groups.last_mut() {
                        if *line_num == last.0 + last.1 {
                            last.1 += 1;
                            continue;
                        }
                    }
                    groups.push((*line_num, 1));
                }

                for (start, count) in groups {
                    entries.push(BlameEntry {
                        commit: oid,
                        original_line: start,
                        final_line: start,
                        num_lines: count,
                    });
                }
                unblamed.clear();
            }
            break;
        }
    }

    // Sort entries by final_line
    entries.sort_by_key(|e| e.final_line);
    Ok(entries)
}

/// Compute which line indices in `current` changed compared to `parent`
/// using the Myers diff algorithm for accurate line-level attribution.
fn diff_blame_changed_set(
    parent: &[String],
    current: &[String],
) -> std::collections::HashSet<usize> {
    if parent.is_empty() {
        // Root commit: all lines are new
        return (0..current.len()).collect();
    }

    // Join lines back with newlines for diff_edits (which splits internally)
    let old_bytes = parent.join("\n");
    let new_bytes = current.join("\n");
    let old_bytes = if old_bytes.is_empty() {
        Vec::new()
    } else {
        format!("{}\n", old_bytes).into_bytes()
    };
    let new_bytes = if new_bytes.is_empty() {
        Vec::new()
    } else {
        format!("{}\n", new_bytes).into_bytes()
    };

    let edits = diff_edits(&old_bytes, &new_bytes, git_diff::DiffAlgorithm::Myers);

    // All lines start as changed; mark equal ones as unchanged
    let mut changed: std::collections::HashSet<usize> = (0..current.len()).collect();

    for edit in &edits {
        if edit.op == EditOp::Equal && edit.new_index < current.len() {
            changed.remove(&edit.new_index);
        }
    }

    changed
}

/// Read a file's content at a specific revision.
fn read_file_at_rev(
    repo: &git_repository::Repository,
    commit_oid: &ObjectId,
    path: &str,
) -> Result<Vec<String>> {
    let obj = repo
        .odb()
        .read_cached(commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit not found: {}", commit_oid))?;

    let tree_oid = match obj {
        Object::Commit(c) => c.tree,
        _ => anyhow::bail!("not a commit: {}", commit_oid),
    };

    let blob_oid = resolve_path_in_tree(repo, &tree_oid, path)?;

    let blob_obj = repo
        .odb()
        .read_cached(&blob_oid)?
        .ok_or_else(|| anyhow::anyhow!("blob not found: {}", blob_oid))?;

    match blob_obj {
        Object::Blob(blob) => {
            let content = String::from_utf8_lossy(&blob.data);
            Ok(content.lines().map(|l| l.to_string()).collect())
        }
        _ => anyhow::bail!("not a blob: {}", blob_oid),
    }
}

fn resolve_path_in_tree(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    path: &str,
) -> Result<ObjectId> {
    let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = *tree_oid;

    for component in &components {
        let obj = repo
            .odb()
            .read_cached(&current)?
            .ok_or_else(|| anyhow::anyhow!("tree not found: {}", current))?;

        let tree = match obj {
            Object::Tree(t) => t,
            _ => anyhow::bail!("not a tree: {}", current),
        };

        let entry = tree
            .entries
            .iter()
            .find(|e| e.name.as_bstr() == component.as_bytes().as_bstr())
            .ok_or_else(|| anyhow::anyhow!("path '{}' not found", component))?;

        current = entry.oid;
    }

    Ok(current)
}

fn parse_line_range(range: &str, total: usize) -> Result<(usize, usize)> {
    let parts: Vec<&str> = range.split(',').collect();
    match parts.len() {
        1 => {
            let start: usize = parts[0].parse()?;
            Ok((start, total))
        }
        2 => {
            let start: usize = parts[0].parse()?;
            let end: usize = parts[1].parse()?;
            Ok((start, end.min(total)))
        }
        _ => anyhow::bail!("invalid line range: {}", range),
    }
}

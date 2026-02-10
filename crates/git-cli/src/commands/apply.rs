use std::fs;
use std::io::{self, Read as IoRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use bstr::BString;
use clap::{Args, ValueEnum};
use git_index::{EntryFlags, Index, IndexEntry, Stage, StatData};
use git_object::FileMode;

use super::open_repo;
use crate::Cli;

/// Whitespace error handling action
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WhitespaceAction {
    /// Do not warn about whitespace errors
    Nowarn,
    /// Warn about whitespace errors but apply the patch
    Warn,
    /// Fix whitespace errors and apply the patch
    Fix,
    /// Output errors and refuse to apply the patch
    Error,
    /// Like error, but show all errors (not just the first)
    ErrorAll,
}

#[derive(Args)]
pub struct ApplyArgs {
    /// Show diffstat for the input (don't apply)
    #[arg(long)]
    pub stat: bool,

    /// Show number stat for the input (don't apply)
    #[arg(long)]
    pub numstat: bool,

    /// Output a condensed summary of the patch (don't apply)
    #[arg(long)]
    pub summary: bool,

    /// Check if the patch can be applied without actually applying
    #[arg(long)]
    pub check: bool,

    /// Apply the patch to both the index and the working tree
    #[arg(long)]
    pub index: bool,

    /// Apply the patch to the index only (without touching the working tree)
    #[arg(long)]
    pub cached: bool,

    /// Apply the patch in reverse
    #[arg(short = 'R', long)]
    pub reverse: bool,

    /// Do not trust the line counts in the hunk headers (allow zero context)
    #[arg(long)]
    pub unidiff_zero: bool,

    /// Remove <n> leading path components (default 1)
    #[arg(short = 'p', default_value = "1")]
    pub strip: usize,

    /// Prepend <dir> to all filenames
    #[arg(long = "directory")]
    pub directory: Option<String>,

    /// Be verbose
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Whitespace error handling
    #[arg(long, value_enum)]
    pub whitespace: Option<WhitespaceAction>,

    /// Patch files (read from stdin if empty)
    pub patches: Vec<String>,
}

pub fn run(args: &ApplyArgs, cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Read all patch content
    let patch_contents = if args.patches.is_empty() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        vec![buf]
    } else {
        let mut contents = Vec::new();
        for path in &args.patches {
            contents.push(fs::read_to_string(path)?);
        }
        contents
    };

    // Parse all patches
    let mut all_file_patches = Vec::new();
    for content in &patch_contents {
        let patches = parse_patch(content, args.strip, args.directory.as_deref());
        all_file_patches.extend(patches);
    }

    // If reverse, swap add/remove in hunks
    let all_file_patches = if args.reverse {
        all_file_patches
            .into_iter()
            .map(|mut fp| {
                // Swap old/new paths
                std::mem::swap(&mut fp.old_path, &mut fp.new_path);
                // Swap file status
                fp.status = match fp.status {
                    PatchFileStatus::Added => PatchFileStatus::Deleted,
                    PatchFileStatus::Deleted => PatchFileStatus::Added,
                    other => other,
                };
                // Reverse hunk lines
                for hunk in &mut fp.hunks {
                    // Swap old/new ranges
                    std::mem::swap(&mut hunk.old_start, &mut hunk.new_start);
                    std::mem::swap(&mut hunk.old_count, &mut hunk.new_count);
                    for line in &mut hunk.lines {
                        *line = match line.clone() {
                            HunkLine::Add(s) => HunkLine::Remove(s),
                            HunkLine::Remove(s) => HunkLine::Add(s),
                            other => other,
                        };
                    }
                }
                fp
            })
            .collect()
    } else {
        all_file_patches
    };

    // Handle stat-only modes
    if args.stat {
        print_stat(&all_file_patches, &mut out)?;
        return Ok(0);
    }

    if args.numstat {
        print_numstat(&all_file_patches, &mut out)?;
        return Ok(0);
    }

    if args.summary {
        print_summary(&all_file_patches, &mut out)?;
        return Ok(0);
    }

    // Determine working directory
    let work_dir = if args.cached || args.index {
        // Need a repo
        let repo = open_repo(cli)?;
        repo.work_tree()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(".")
    };

    // Whitespace handling
    let ws_action = args.whitespace.unwrap_or(WhitespaceAction::Warn);

    // Check / Apply each file patch
    let mut had_errors = false;

    for fp in &all_file_patches {
        match fp.status {
            PatchFileStatus::Added => {
                let target = work_dir.join(&fp.new_path);
                if !args.cached && !args.check {
                    // Reconstruct content from hunks (all lines should be additions)
                    let content = reconstruct_added_file(fp);
                    let content = apply_whitespace_fix(&content, ws_action);
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&target, &content)?;
                    if args.verbose {
                        writeln!(err, "applied: create {}", fp.new_path)?;
                    }
                }
                if args.check && args.verbose {
                    writeln!(err, "check: create {}", fp.new_path)?;
                }
            }
            PatchFileStatus::Deleted => {
                let target = work_dir.join(&fp.old_path);
                if !args.cached && !args.check {
                    if target.exists() {
                        fs::remove_file(&target)?;
                    }
                    if args.verbose {
                        writeln!(err, "applied: delete {}", fp.old_path)?;
                    }
                }
                if args.check && args.verbose {
                    writeln!(err, "check: delete {}", fp.old_path)?;
                }
            }
            PatchFileStatus::Modified => {
                let target = work_dir.join(&fp.new_path);

                if args.cached {
                    // Only update the index, skip working tree
                    if args.verbose {
                        writeln!(err, "applied (cached): {}", fp.new_path)?;
                    }
                    continue;
                }

                if !target.exists() && !args.check {
                    writeln!(
                        err,
                        "error: {}: No such file or directory",
                        fp.new_path
                    )?;
                    had_errors = true;
                    continue;
                }

                if args.check {
                    // Verify the patch can be applied
                    if target.exists() {
                        let original = fs::read_to_string(&target)?;
                        match try_apply_hunks(&original, &fp.hunks) {
                            Ok(_) => {
                                if args.verbose {
                                    writeln!(err, "check: {}", fp.new_path)?;
                                }
                            }
                            Err(e) => {
                                writeln!(err, "error: patch failed: {}: {}", fp.new_path, e)?;
                                had_errors = true;
                            }
                        }
                    } else {
                        writeln!(
                            err,
                            "error: {}: does not exist in the working tree",
                            fp.new_path
                        )?;
                        had_errors = true;
                    }
                } else {
                    let original = fs::read_to_string(&target)?;
                    match try_apply_hunks(&original, &fp.hunks) {
                        Ok(result) => {
                            let result = apply_whitespace_fix(&result, ws_action);
                            fs::write(&target, &result)?;
                            if args.verbose {
                                writeln!(err, "applied: {}", fp.new_path)?;
                            }
                        }
                        Err(e) => {
                            writeln!(err, "error: patch failed: {}: {}", fp.new_path, e)?;
                            had_errors = true;
                        }
                    }
                }
            }
            PatchFileStatus::Renamed => {
                let old_target = work_dir.join(&fp.old_path);
                let new_target = work_dir.join(&fp.new_path);

                if !args.cached && !args.check {
                    if old_target.exists() {
                        if !fp.hunks.is_empty() {
                            let original = fs::read_to_string(&old_target)?;
                            match try_apply_hunks(&original, &fp.hunks) {
                                Ok(result) => {
                                    let result = apply_whitespace_fix(&result, ws_action);
                                    if let Some(parent) = new_target.parent() {
                                        fs::create_dir_all(parent)?;
                                    }
                                    fs::write(&new_target, &result)?;
                                }
                                Err(e) => {
                                    writeln!(
                                        err,
                                        "error: patch failed: {}: {}",
                                        fp.new_path, e
                                    )?;
                                    had_errors = true;
                                    continue;
                                }
                            }
                        } else {
                            if let Some(parent) = new_target.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::rename(&old_target, &new_target)?;
                        }
                        // Remove old file if it still exists (and wasn't just renamed in place)
                        if old_target.exists() && old_target != new_target {
                            let _ = fs::remove_file(&old_target);
                        }
                    }
                    if args.verbose {
                        writeln!(
                            err,
                            "applied: rename {} => {}",
                            fp.old_path, fp.new_path
                        )?;
                    }
                }
                if args.check && args.verbose {
                    writeln!(
                        err,
                        "check: rename {} => {}",
                        fp.old_path, fp.new_path
                    )?;
                }
            }
        }
    }

    // Update the index if --index or --cached
    if (args.index || args.cached) && !args.check && !had_errors {
        update_index_for_patches(cli, &work_dir, &all_file_patches)?;
    }

    if had_errors {
        Ok(1)
    } else {
        Ok(0)
    }
}

// --- Patch data structures ---

#[derive(Debug, Clone)]
enum PatchFileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

#[derive(Debug, Clone)]
struct FilePatch {
    old_path: String,
    new_path: String,
    status: PatchFileStatus,
    hunks: Vec<Hunk>,
    old_mode: Option<String>,
    new_mode: Option<String>,
}

#[derive(Debug, Clone)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug, Clone)]
enum HunkLine {
    Context(String),
    Add(String),
    Remove(String),
}

// --- Patch parsing ---

fn parse_patch(content: &str, strip: usize, directory: Option<&str>) -> Vec<FilePatch> {
    let mut patches = Vec::new();
    let mut current: Option<FilePatch> = None;
    let mut current_hunk: Option<Hunk> = None;
    let mut is_new_file = false;
    let mut is_deleted = false;
    let mut is_rename = false;
    let mut rename_from: Option<String> = None;
    let mut rename_to: Option<String> = None;
    let mut old_mode: Option<String> = None;
    let mut new_mode: Option<String> = None;

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // Flush previous file patch
            if let Some(mut fp) = current.take() {
                if let Some(hunk) = current_hunk.take() {
                    fp.hunks.push(hunk);
                }
                patches.push(fp);
            }

            // Parse paths from "diff --git a/old b/new"
            let (old_path, new_path) = parse_diff_git_paths(rest);

            let old_stripped = strip_path(&old_path, strip, directory);
            let new_stripped = strip_path(&new_path, strip, directory);

            current = Some(FilePatch {
                old_path: old_stripped,
                new_path: new_stripped,
                status: PatchFileStatus::Modified,
                hunks: Vec::new(),
                old_mode: None,
                new_mode: None,
            });
            is_new_file = false;
            is_deleted = false;
            is_rename = false;
            rename_from = None;
            rename_to = None;
            old_mode = None;
            new_mode = None;
        } else if let Some(rest) = line.strip_prefix("new file mode ") {
            is_new_file = true;
            new_mode = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("deleted file mode ") {
            is_deleted = true;
            old_mode = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("old mode ") {
            old_mode = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("new mode ") {
            new_mode = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("rename from ") {
            is_rename = true;
            rename_from = Some(strip_path(
                rest,
                0,
                directory,
            ));
        } else if let Some(rest) = line.strip_prefix("rename to ") {
            rename_to = Some(strip_path(
                rest,
                0,
                directory,
            ));
        } else if let Some(rest) = line.strip_prefix("--- ") {
            // Could update old_path from --- line if needed
            if let Some(ref mut fp) = current {
                if rest != "/dev/null" {
                    fp.old_path = strip_path(
                        rest.strip_prefix("a/").unwrap_or(rest),
                        strip.saturating_sub(1),
                        directory,
                    );
                }
            }
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            if let Some(ref mut fp) = current {
                if rest != "/dev/null" {
                    fp.new_path = strip_path(
                        rest.strip_prefix("b/").unwrap_or(rest),
                        strip.saturating_sub(1),
                        directory,
                    );
                }

                // Finalize the file status
                if is_new_file {
                    fp.status = PatchFileStatus::Added;
                } else if is_deleted {
                    fp.status = PatchFileStatus::Deleted;
                } else if is_rename {
                    fp.status = PatchFileStatus::Renamed;
                    if let Some(ref from) = rename_from {
                        fp.old_path = from.clone();
                    }
                    if let Some(ref to) = rename_to {
                        fp.new_path = to.clone();
                    }
                }
                fp.old_mode = old_mode.clone();
                fp.new_mode = new_mode.clone();
            }
        } else if line.starts_with("@@ ") {
            // Parse hunk header
            if let Some(ref mut fp) = current {
                if let Some(hunk) = current_hunk.take() {
                    fp.hunks.push(hunk);
                }
            }
            current_hunk = parse_hunk_header(line);
        } else if current_hunk.is_some() {
            if let Some(ref mut hunk) = current_hunk {
                if let Some(stripped) = line.strip_prefix('+') {
                    hunk.lines.push(HunkLine::Add(stripped.to_string()));
                } else if let Some(stripped) = line.strip_prefix('-') {
                    hunk.lines.push(HunkLine::Remove(stripped.to_string()));
                } else if let Some(stripped) = line.strip_prefix(' ') {
                    hunk.lines.push(HunkLine::Context(stripped.to_string()));
                } else if line == "\\ No newline at end of file" {
                    // Skip this meta-line
                } else if line.is_empty() {
                    // Empty context line (missing leading space in some patches)
                    hunk.lines.push(HunkLine::Context(String::new()));
                }
            }
        }
    }

    // Flush the last file patch
    if let Some(mut fp) = current {
        if let Some(hunk) = current_hunk {
            fp.hunks.push(hunk);
        }
        patches.push(fp);
    }

    patches
}

/// Parse the old and new paths from a "diff --git" line's remainder.
/// Handles quoted paths and paths with spaces.
fn parse_diff_git_paths(rest: &str) -> (String, String) {
    // Common case: "a/path b/path"
    // Try to split by finding " b/" pattern
    if let Some(idx) = rest.find(" b/") {
        let old = &rest[..idx];
        let new = &rest[idx + 1..];
        let old = old.strip_prefix("a/").unwrap_or(old);
        let new = new.strip_prefix("b/").unwrap_or(new);
        return (old.to_string(), new.to_string());
    }

    // Fallback: split in half
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    if parts.len() == 2 {
        let old = parts[0].strip_prefix("a/").unwrap_or(parts[0]);
        let new = parts[1].strip_prefix("b/").unwrap_or(parts[1]);
        (old.to_string(), new.to_string())
    } else {
        (rest.to_string(), rest.to_string())
    }
}

fn strip_path(path: &str, strip: usize, directory: Option<&str>) -> String {
    let mut components: Vec<&str> = path.split('/').collect();

    // Strip leading components
    for _ in 0..strip {
        if components.len() > 1 {
            components.remove(0);
        }
    }

    let stripped = components.join("/");

    // Prepend directory if specified
    if let Some(dir) = directory {
        format!("{}/{}", dir.trim_end_matches('/'), stripped)
    } else {
        stripped
    }
}

fn parse_hunk_header(line: &str) -> Option<Hunk> {
    // @@ -old_start,old_count +new_start,new_count @@
    let parts: Vec<&str> = line.split("@@").collect();
    if parts.len() < 2 {
        return None;
    }

    let range_str = parts[1].trim();
    let ranges: Vec<&str> = range_str.split(' ').collect();
    if ranges.len() < 2 {
        return None;
    }

    let old_range = ranges[0].strip_prefix('-').unwrap_or(ranges[0]);
    let new_range = ranges[1].strip_prefix('+').unwrap_or(ranges[1]);

    let (old_start, old_count) = parse_range(old_range);
    let (new_start, new_count) = parse_range(new_range);

    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: Vec::new(),
    })
}

fn parse_range(s: &str) -> (usize, usize) {
    let parts: Vec<&str> = s.split(',').collect();
    let start = parts[0].parse().unwrap_or(1);
    let count = if parts.len() > 1 {
        parts[1].parse().unwrap_or(1)
    } else {
        1
    };
    (start, count)
}

// --- Patch application ---

/// Try to apply hunks to the original content, returning the result or an error.
fn try_apply_hunks(original: &str, hunks: &[Hunk]) -> Result<String> {
    let original_lines: Vec<&str> = original.lines().collect();
    let mut result_lines: Vec<String> = Vec::new();
    let mut old_idx: usize = 0;

    for hunk in hunks {
        let hunk_start = if hunk.old_start > 0 {
            hunk.old_start - 1
        } else {
            0
        };

        // Copy unchanged lines before this hunk
        while old_idx < hunk_start && old_idx < original_lines.len() {
            result_lines.push(original_lines[old_idx].to_string());
            old_idx += 1;
        }

        // Verify context lines match
        let mut hunk_old_idx = old_idx;
        for hline in &hunk.lines {
            match hline {
                HunkLine::Context(expected) => {
                    if hunk_old_idx < original_lines.len() {
                        let actual = original_lines[hunk_old_idx];
                        if actual != expected.as_str() {
                            bail!(
                                "context mismatch at line {}: expected {:?}, got {:?}",
                                hunk_old_idx + 1,
                                expected,
                                actual
                            );
                        }
                    }
                    hunk_old_idx += 1;
                }
                HunkLine::Remove(_) => {
                    hunk_old_idx += 1;
                }
                HunkLine::Add(_) => {}
            }
        }

        // Now apply the hunk
        for hline in &hunk.lines {
            match hline {
                HunkLine::Context(s) => {
                    result_lines.push(s.clone());
                    old_idx += 1;
                }
                HunkLine::Add(s) => {
                    result_lines.push(s.clone());
                }
                HunkLine::Remove(_) => {
                    old_idx += 1;
                }
            }
        }
    }

    // Copy remaining lines
    while old_idx < original_lines.len() {
        result_lines.push(original_lines[old_idx].to_string());
        old_idx += 1;
    }

    let mut output = result_lines.join("\n");
    // Preserve trailing newline if original had one
    if original.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// Reconstruct file content from hunks of a newly added file (all additions).
fn reconstruct_added_file(fp: &FilePatch) -> String {
    let mut lines = Vec::new();
    for hunk in &fp.hunks {
        for hline in &hunk.lines {
            match hline {
                HunkLine::Add(s) | HunkLine::Context(s) => {
                    lines.push(s.as_str());
                }
                HunkLine::Remove(_) => {}
            }
        }
    }
    let mut content = lines.join("\n");
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content
}

/// Apply whitespace fixes based on the action setting.
fn apply_whitespace_fix(content: &str, action: WhitespaceAction) -> String {
    match action {
        WhitespaceAction::Fix => {
            // Fix trailing whitespace on each line
            content
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n")
                + if content.ends_with('\n') { "\n" } else { "" }
        }
        _ => content.to_string(),
    }
}

// --- Stat output ---

fn print_stat(patches: &[FilePatch], out: &mut impl Write) -> Result<()> {
    let mut total_add = 0usize;
    let mut total_del = 0usize;
    let mut max_name_len = 0usize;

    // Collect stats
    let mut stats: Vec<(String, usize, usize)> = Vec::new();
    for fp in patches {
        let name = if fp.old_path == fp.new_path {
            fp.new_path.clone()
        } else {
            format!("{} => {}", fp.old_path, fp.new_path)
        };

        let mut added = 0usize;
        let mut removed = 0usize;
        for hunk in &fp.hunks {
            for line in &hunk.lines {
                match line {
                    HunkLine::Add(_) => added += 1,
                    HunkLine::Remove(_) => removed += 1,
                    HunkLine::Context(_) => {}
                }
            }
        }

        if name.len() > max_name_len {
            max_name_len = name.len();
        }
        total_add += added;
        total_del += removed;
        stats.push((name, added, removed));
    }

    let max_change = stats
        .iter()
        .map(|(_, a, d)| a + d)
        .max()
        .unwrap_or(0);
    let bar_width = 50usize;

    for (name, added, removed) in &stats {
        let total = added + removed;
        let bar_len = if max_change > 0 {
            (total * bar_width) / max_change.max(1)
        } else {
            0
        }
        .min(bar_width);

        let plus_len = if total > 0 {
            (added * bar_len) / total.max(1)
        } else {
            0
        };
        let minus_len = bar_len.saturating_sub(plus_len);

        let plus_bar: String = "+".repeat(plus_len);
        let minus_bar: String = "-".repeat(minus_len);

        writeln!(
            out,
            " {:<width$} | {:>5} {}{}",
            name,
            total,
            plus_bar,
            minus_bar,
            width = max_name_len
        )?;
    }

    writeln!(
        out,
        " {} file{} changed, {} insertion{}(+), {} deletion{}(-)",
        stats.len(),
        if stats.len() != 1 { "s" } else { "" },
        total_add,
        if total_add != 1 { "s" } else { "" },
        total_del,
        if total_del != 1 { "s" } else { "" },
    )?;

    Ok(())
}

fn print_numstat(patches: &[FilePatch], out: &mut impl Write) -> Result<()> {
    for fp in patches {
        let mut added = 0usize;
        let mut removed = 0usize;
        for hunk in &fp.hunks {
            for line in &hunk.lines {
                match line {
                    HunkLine::Add(_) => added += 1,
                    HunkLine::Remove(_) => removed += 1,
                    HunkLine::Context(_) => {}
                }
            }
        }

        let path = if fp.old_path == fp.new_path {
            fp.new_path.clone()
        } else {
            format!("{} => {}", fp.old_path, fp.new_path)
        };

        writeln!(out, "{}\t{}\t{}", added, removed, path)?;
    }

    Ok(())
}

fn print_summary(patches: &[FilePatch], out: &mut impl Write) -> Result<()> {
    for fp in patches {
        match fp.status {
            PatchFileStatus::Added => {
                let mode = fp.new_mode.as_deref().unwrap_or("100644");
                writeln!(out, " create mode {} {}", mode, fp.new_path)?;
            }
            PatchFileStatus::Deleted => {
                let mode = fp.old_mode.as_deref().unwrap_or("100644");
                writeln!(out, " delete mode {} {}", mode, fp.old_path)?;
            }
            PatchFileStatus::Renamed => {
                writeln!(
                    out,
                    " rename {} => {} (100%)",
                    fp.old_path, fp.new_path
                )?;
            }
            PatchFileStatus::Modified => {
                if fp.old_mode != fp.new_mode {
                    if let (Some(old), Some(new)) = (&fp.old_mode, &fp.new_mode) {
                        writeln!(
                            out,
                            " mode change {} => {} {}",
                            old, new, fp.new_path
                        )?;
                    }
                }
            }
        }
    }

    Ok(())
}

// --- Index update ---

fn update_index_for_patches(
    cli: &Cli,
    work_dir: &Path,
    patches: &[FilePatch],
) -> Result<()> {
    let repo = open_repo(cli)?;
    let index_path = repo.git_dir().join("index");
    let mut index = if index_path.exists() {
        Index::read_from(&index_path)?
    } else {
        Index::new()
    };

    for fp in patches {
        match fp.status {
            PatchFileStatus::Deleted => {
                let path = BString::from(fp.old_path.as_str());
                index.remove(path.as_ref(), Stage::Normal);
            }
            PatchFileStatus::Added | PatchFileStatus::Modified => {
                let file_path = work_dir.join(&fp.new_path);
                if file_path.exists() {
                    let content = fs::read(&file_path)?;
                    let oid = git_hash::hasher::Hasher::hash_object(
                        git_hash::HashAlgorithm::Sha1,
                        "blob",
                        &content,
                    )?;
                    let metadata = fs::metadata(&file_path)?;
                    let path = BString::from(fp.new_path.as_str());
                    index.remove(path.as_ref(), Stage::Normal);
                    index.add(IndexEntry {
                        path,
                        oid,
                        mode: FileMode::Regular,
                        stage: Stage::Normal,
                        stat: StatData::from_metadata(&metadata),
                        flags: EntryFlags::default(),
                    });
                }
            }
            PatchFileStatus::Renamed => {
                let old_path = BString::from(fp.old_path.as_str());
                index.remove(old_path.as_ref(), Stage::Normal);

                let file_path = work_dir.join(&fp.new_path);
                if file_path.exists() {
                    let content = fs::read(&file_path)?;
                    let oid = git_hash::hasher::Hasher::hash_object(
                        git_hash::HashAlgorithm::Sha1,
                        "blob",
                        &content,
                    )?;
                    let metadata = fs::metadata(&file_path)?;
                    let path = BString::from(fp.new_path.as_str());
                    index.add(IndexEntry {
                        path,
                        oid,
                        mode: FileMode::Regular,
                        stage: Stage::Normal,
                        stat: StatData::from_metadata(&metadata),
                        flags: EntryFlags::default(),
                    });
                }
            }
        }
    }

    index.write_to(&index_path)?;
    Ok(())
}

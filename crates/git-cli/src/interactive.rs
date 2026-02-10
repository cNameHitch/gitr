//! Interactive hunk selection for patch mode (`-p`/`--patch`).
//!
//! Provides an `InteractiveHunkSelector` that presents each diff hunk to the
//! user and lets them accept, reject, split, or edit individual hunks.
//! Input is read from `/dev/tty` so the selector works even when stdin is piped.

use std::io::{self, BufRead, BufReader, Write};

use bstr::ByteSlice;
use git_diff::{DiffLine, DiffResult, FileDiff, Hunk};

/// Decision for a single hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HunkDecision {
    /// Include this hunk.
    Accept,
    /// Skip this hunk.
    Reject,
}

/// Interactive hunk selector matching git's patch mode UI.
///
/// Opens `/dev/tty` for user input so it works even when stdin is piped
/// (e.g. `echo y | gitr add -p`).
pub struct InteractiveHunkSelector {
    /// Reader for TTY input.
    tty_reader: BufReader<std::fs::File>,
}

impl InteractiveHunkSelector {
    /// Create a new selector by opening `/dev/tty`.
    pub fn new() -> io::Result<Self> {
        let tty = std::fs::File::open("/dev/tty")?;
        Ok(Self {
            tty_reader: BufReader::new(tty),
        })
    }

    /// Present each hunk in `diff_result` for interactive selection.
    ///
    /// Returns a new `DiffResult` containing only the accepted hunks.
    /// Files with no accepted hunks are omitted.
    pub fn select_hunks(&mut self, diff_result: &DiffResult) -> io::Result<DiffResult> {
        let stderr = io::stderr();
        let mut err = stderr.lock();

        let mut selected_files = Vec::new();

        for file_diff in &diff_result.files {
            if file_diff.is_binary {
                continue;
            }
            if file_diff.hunks.is_empty() {
                continue;
            }

            let path = file_diff.path().to_str_lossy();
            let total_hunks = file_diff.hunks.len();

            // Flatten hunks (split may add more)
            let mut hunks: Vec<Hunk> = file_diff.hunks.clone();
            let mut decisions: Vec<HunkDecision> = Vec::new();
            let mut i = 0;
            let mut quit = false;
            let mut accept_all = false;
            let mut done = false;

            while i < hunks.len() {
                if quit || done {
                    break;
                }

                if accept_all {
                    decisions.push(HunkDecision::Accept);
                    i += 1;
                    continue;
                }

                // Show hunk header and content
                write_hunk_display(&mut err, &hunks[i], &path)?;

                // Determine if this hunk can be split
                let can_split = can_split_hunk(&hunks[i]);
                let hunk_number = i + 1;
                let hunk_total = hunks.len();

                loop {
                    // Prompt
                    if can_split {
                        write!(
                            err,
                            "({}/{}) Stage this hunk [y,n,q,a,d,s,e,?]? ",
                            hunk_number, hunk_total
                        )?;
                    } else {
                        write!(
                            err,
                            "({}/{}) Stage this hunk [y,n,q,a,d,e,?]? ",
                            hunk_number, hunk_total
                        )?;
                    }
                    err.flush()?;

                    let mut input = String::new();
                    if self.tty_reader.read_line(&mut input)? == 0 {
                        // EOF on tty — treat as quit
                        quit = true;
                        break;
                    }
                    let cmd = input.trim();

                    match cmd {
                        "y" => {
                            decisions.push(HunkDecision::Accept);
                            i += 1;
                            break;
                        }
                        "n" => {
                            decisions.push(HunkDecision::Reject);
                            i += 1;
                            break;
                        }
                        "q" => {
                            decisions.push(HunkDecision::Reject);
                            quit = true;
                            break;
                        }
                        "a" => {
                            decisions.push(HunkDecision::Accept);
                            accept_all = true;
                            i += 1;
                            break;
                        }
                        "d" => {
                            done = true;
                            break;
                        }
                        "s" if can_split => {
                            // Split the current hunk and replace it in the list
                            let sub_hunks = split_hunk(&hunks[i]);
                            if sub_hunks.len() > 1 {
                                hunks.splice(i..=i, sub_hunks);
                                // Re-display: don't increment i, loop again
                                write_hunk_display(&mut err, &hunks[i], &path)?;
                            } else {
                                writeln!(err, "Sorry, cannot split this hunk")?;
                            }
                            // Continue the inner prompt loop with the (possibly new) hunk
                            continue;
                        }
                        "s" => {
                            writeln!(err, "Sorry, cannot split this hunk")?;
                            continue;
                        }
                        "e" => {
                            // Edit hunk: for now, accept as-is with a note
                            writeln!(err, "Sorry, manual hunk editing is not yet supported")?;
                            continue;
                        }
                        _ => {
                            print_help(&mut err, can_split)?;
                            continue;
                        }
                    }
                }
            }

            // Fill remaining hunks as rejected if we quit or finished early
            while decisions.len() < hunks.len() {
                decisions.push(HunkDecision::Reject);
            }

            // Collect accepted hunks
            let accepted_hunks: Vec<Hunk> = hunks
                .into_iter()
                .zip(decisions.iter())
                .filter(|(_, d)| **d == HunkDecision::Accept)
                .map(|(h, _)| h)
                .collect();

            if !accepted_hunks.is_empty() {
                selected_files.push(FileDiff {
                    status: file_diff.status,
                    old_path: file_diff.old_path.clone(),
                    new_path: file_diff.new_path.clone(),
                    old_mode: file_diff.old_mode,
                    new_mode: file_diff.new_mode,
                    old_oid: file_diff.old_oid,
                    new_oid: file_diff.new_oid,
                    hunks: accepted_hunks,
                    is_binary: file_diff.is_binary,
                    similarity: file_diff.similarity,
                });
            }

            if quit {
                break;
            }

            // Show how many hunks were staged for this file
            let staged = decisions
                .iter()
                .filter(|d| **d == HunkDecision::Accept)
                .count();
            if staged > 0 {
                writeln!(
                    err,
                    "# {} hunk(s) selected out of {} for {}",
                    staged, total_hunks, path
                )?;
            }
        }

        Ok(DiffResult {
            files: selected_files,
        })
    }
}

/// Display a single hunk on stderr with its @@ header and diff lines.
fn write_hunk_display(out: &mut impl Write, hunk: &Hunk, path: &str) -> io::Result<()> {
    // @@ header
    let old_range = if hunk.old_count == 1 {
        format!("{}", hunk.old_start)
    } else {
        format!("{},{}", hunk.old_start, hunk.old_count)
    };
    let new_range = if hunk.new_count == 1 {
        format!("{}", hunk.new_start)
    } else {
        format!("{},{}", hunk.new_start, hunk.new_count)
    };
    write!(out, "@@ -{} +{} @@", old_range, new_range)?;
    if let Some(ref header) = hunk.header {
        write!(out, " {}", header.to_str_lossy())?;
    }
    writeln!(out, " {}", path)?;

    // Lines
    for line in &hunk.lines {
        match line {
            DiffLine::Context(content) => {
                write!(out, " ")?;
                out.write_all(content)?;
                if !content.ends_with(b"\n") {
                    writeln!(out)?;
                }
            }
            DiffLine::Addition(content) => {
                write!(out, "+")?;
                out.write_all(content)?;
                if !content.ends_with(b"\n") {
                    writeln!(out)?;
                }
            }
            DiffLine::Deletion(content) => {
                write!(out, "-")?;
                out.write_all(content)?;
                if !content.ends_with(b"\n") {
                    writeln!(out)?;
                }
            }
        }
    }
    Ok(())
}

/// Check if a hunk can be split (has multiple contiguous change regions
/// separated by context lines).
fn can_split_hunk(hunk: &Hunk) -> bool {
    // A hunk can be split if it has at least two separate change regions
    // (groups of additions/deletions separated by context lines).
    let mut in_change = false;
    let mut change_regions = 0u32;

    for line in &hunk.lines {
        match line {
            DiffLine::Context(_) => {
                if in_change {
                    in_change = false;
                }
            }
            DiffLine::Addition(_) | DiffLine::Deletion(_) => {
                if !in_change {
                    change_regions += 1;
                    in_change = true;
                }
            }
        }
    }

    change_regions >= 2
}

/// Split a hunk into multiple sub-hunks, one per contiguous change region.
///
/// Each sub-hunk includes surrounding context lines (up to 3 lines).
pub fn split_hunk(hunk: &Hunk) -> Vec<Hunk> {
    let context_size = 3usize;

    // Identify change regions: each region is a range of line indices
    // that contains additions/deletions.
    let mut regions: Vec<(usize, usize)> = Vec::new(); // (start, end) inclusive
    let mut i = 0;
    while i < hunk.lines.len() {
        match &hunk.lines[i] {
            DiffLine::Addition(_) | DiffLine::Deletion(_) => {
                let start = i;
                while i < hunk.lines.len()
                    && !matches!(&hunk.lines[i], DiffLine::Context(_))
                {
                    i += 1;
                }
                regions.push((start, i - 1));
            }
            DiffLine::Context(_) => {
                i += 1;
            }
        }
    }

    if regions.len() <= 1 {
        return vec![hunk.clone()];
    }

    let mut sub_hunks = Vec::new();

    // Track our position in old and new line numbering
    let _old_line = hunk.old_start;
    let _new_line = hunk.new_start;
    // Precompute line offsets: for each line index, what old_line and new_line
    // values correspond.
    let mut old_lines_at: Vec<u32> = Vec::with_capacity(hunk.lines.len());
    let mut new_lines_at: Vec<u32> = Vec::with_capacity(hunk.lines.len());
    {
        let mut ol = hunk.old_start;
        let mut nl = hunk.new_start;
        for line in &hunk.lines {
            old_lines_at.push(ol);
            new_lines_at.push(nl);
            match line {
                DiffLine::Context(_) => {
                    ol += 1;
                    nl += 1;
                }
                DiffLine::Addition(_) => {
                    nl += 1;
                }
                DiffLine::Deletion(_) => {
                    ol += 1;
                }
            }
        }
    }

    for (ri, &(region_start, region_end)) in regions.iter().enumerate() {
        let mut lines = Vec::new();

        // Leading context: up to `context_size` lines before the change region
        let ctx_start = if region_start >= context_size {
            // But don't overlap with the previous region's trailing context
            if ri > 0 {
                let prev_end = regions[ri - 1].1;
                (prev_end + 1).max(region_start.saturating_sub(context_size))
            } else {
                region_start.saturating_sub(context_size)
            }
        } else {
            0
        };

        // Only add context lines from before the change region
        for j in ctx_start..region_start {
            if matches!(&hunk.lines[j], DiffLine::Context(_)) {
                lines.push(hunk.lines[j].clone());
            }
        }

        // The change lines themselves
        for j in region_start..=region_end {
            lines.push(hunk.lines[j].clone());
        }

        // Trailing context: up to `context_size` lines after the change region
        let ctx_end = if ri + 1 < regions.len() {
            let next_start = regions[ri + 1].0;
            (region_end + 1 + context_size).min(next_start)
        } else {
            (region_end + 1 + context_size).min(hunk.lines.len())
        };

        for j in (region_end + 1)..ctx_end {
            if matches!(&hunk.lines[j], DiffLine::Context(_)) {
                lines.push(hunk.lines[j].clone());
            }
        }

        // Compute old_start and new_start for this sub-hunk
        let _sub_old_start = old_lines_at[ctx_start.max(region_start.min(ctx_start))];
        let _sub_new_start = new_lines_at[ctx_start.max(region_start.min(ctx_start))];

        // If we have leading context, use the start of the leading context
        let effective_start = if ctx_start < region_start {
            ctx_start
        } else {
            region_start
        };
        let sub_old_start = old_lines_at[effective_start];
        let sub_new_start = new_lines_at[effective_start];

        // Count old and new lines in the sub-hunk
        let mut sub_old_count = 0u32;
        let mut sub_new_count = 0u32;
        for line in &lines {
            match line {
                DiffLine::Context(_) => {
                    sub_old_count += 1;
                    sub_new_count += 1;
                }
                DiffLine::Addition(_) => {
                    sub_new_count += 1;
                }
                DiffLine::Deletion(_) => {
                    sub_old_count += 1;
                }
            }
        }

        sub_hunks.push(Hunk {
            old_start: sub_old_start,
            old_count: sub_old_count,
            new_start: sub_new_start,
            new_count: sub_new_count,
            header: hunk.header.clone(),
            lines,
        });
    }

    sub_hunks
}

/// Print the help message for patch mode.
fn print_help(out: &mut impl Write, can_split: bool) -> io::Result<()> {
    writeln!(out, "y - stage this hunk")?;
    writeln!(out, "n - do not stage this hunk")?;
    writeln!(out, "q - quit; do not stage this hunk or any remaining hunks")?;
    writeln!(out, "a - stage this hunk and all later hunks in the file")?;
    writeln!(out, "d - do not stage this hunk or any later hunks in the file")?;
    if can_split {
        writeln!(out, "s - split the current hunk into smaller hunks")?;
    }
    writeln!(out, "e - manually edit the current hunk (not yet supported)")?;
    writeln!(out, "? - print help")?;
    Ok(())
}

/// Apply selected hunks to produce patched content.
///
/// Given the original file content and the selected hunks, produces the new
/// file content with only the selected changes applied.
pub fn apply_hunks_to_content(old_content: &[u8], hunks: &[Hunk]) -> Vec<u8> {
    if hunks.is_empty() {
        return old_content.to_vec();
    }

    let old_lines: Vec<&[u8]> = split_lines_bytes(old_content);
    let mut result: Vec<u8> = Vec::with_capacity(old_content.len());
    let mut old_idx: usize = 0; // 0-based index into old_lines

    for hunk in hunks {
        let hunk_start = (hunk.old_start as usize).saturating_sub(1); // Convert 1-based to 0-based

        // Copy unchanged lines before this hunk
        while old_idx < hunk_start && old_idx < old_lines.len() {
            result.extend_from_slice(old_lines[old_idx]);
            old_idx += 1;
        }

        // Apply hunk lines
        for line in &hunk.lines {
            match line {
                DiffLine::Context(_) => {
                    // Keep the original line
                    if old_idx < old_lines.len() {
                        result.extend_from_slice(old_lines[old_idx]);
                        old_idx += 1;
                    }
                }
                DiffLine::Deletion(_) => {
                    // Skip the old line
                    if old_idx < old_lines.len() {
                        old_idx += 1;
                    }
                }
                DiffLine::Addition(content) => {
                    // Add the new line
                    result.extend_from_slice(content);
                    if !content.ends_with(b"\n") {
                        result.push(b'\n');
                    }
                }
            }
        }
    }

    // Copy remaining lines after the last hunk
    while old_idx < old_lines.len() {
        result.extend_from_slice(old_lines[old_idx]);
        old_idx += 1;
    }

    result
}

/// Apply selected hunks in reverse to produce the reverted content.
///
/// Given the new content and selected hunks, reverses the hunks: additions
/// become deletions and vice versa. This is used by `reset -p` and
/// `checkout -p` / `restore -p`.
pub fn reverse_apply_hunks_to_content(new_content: &[u8], hunks: &[Hunk]) -> Vec<u8> {
    if hunks.is_empty() {
        return new_content.to_vec();
    }

    let new_lines: Vec<&[u8]> = split_lines_bytes(new_content);
    let mut result: Vec<u8> = Vec::with_capacity(new_content.len());
    let mut new_idx: usize = 0; // 0-based index into new_lines

    for hunk in hunks {
        let hunk_start = (hunk.new_start as usize).saturating_sub(1); // new_start is 1-based

        // Copy unchanged lines before this hunk
        while new_idx < hunk_start && new_idx < new_lines.len() {
            result.extend_from_slice(new_lines[new_idx]);
            new_idx += 1;
        }

        // Reverse-apply hunk lines
        for line in &hunk.lines {
            match line {
                DiffLine::Context(_) => {
                    if new_idx < new_lines.len() {
                        result.extend_from_slice(new_lines[new_idx]);
                        new_idx += 1;
                    }
                }
                DiffLine::Addition(_) => {
                    // In reverse, an addition becomes a deletion: skip this line in new
                    if new_idx < new_lines.len() {
                        new_idx += 1;
                    }
                }
                DiffLine::Deletion(content) => {
                    // In reverse, a deletion becomes an addition: insert this line
                    result.extend_from_slice(content);
                    if !content.ends_with(b"\n") {
                        result.push(b'\n');
                    }
                }
            }
        }
    }

    // Copy remaining lines
    while new_idx < new_lines.len() {
        result.extend_from_slice(new_lines[new_idx]);
        new_idx += 1;
    }

    result
}

/// Split byte content into lines, keeping line endings attached.
fn split_lines_bytes(data: &[u8]) -> Vec<&[u8]> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (i, &byte) in data.iter().enumerate() {
        if byte == b'\n' {
            lines.push(&data[start..=i]);
            start = i + 1;
        }
    }
    // Trailing content without newline
    if start < data.len() {
        lines.push(&data[start..]);
    }
    lines
}

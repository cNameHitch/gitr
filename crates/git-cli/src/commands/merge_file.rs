use std::fs;
use std::io::{self, Write};

use anyhow::Result;
use clap::{Args, ValueEnum};

use crate::Cli;

/// Conflict resolution strategy
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConflictStyle {
    /// Take the current (ours) side of conflicts
    Ours,
    /// Take the other (theirs) side of conflicts
    Theirs,
    /// Include both sides of conflicts (union merge)
    Union,
}

#[derive(Args)]
pub struct MergeFileArgs {
    /// Send results to standard output instead of overwriting <current>
    #[arg(short = 'p', long)]
    pub stdout: bool,

    /// Show conflicts in diff3 style (includes base version)
    #[arg(long)]
    pub diff3: bool,

    /// Label for file versions (up to 3: current, base, other)
    #[arg(short = 'L', number_of_values = 1, action = clap::ArgAction::Append)]
    pub label: Vec<String>,

    /// Suppress warnings
    #[arg(short, long)]
    pub quiet: bool,

    /// Conflict resolution strategy: ours, theirs, or union
    #[arg(long, value_enum, name = "strategy")]
    pub merge_strategy: Option<ConflictStyle>,

    /// Shorthand for --merge-strategy=ours
    #[arg(long, conflicts_with_all = ["theirs", "union", "strategy"])]
    pub ours: bool,

    /// Shorthand for --merge-strategy=theirs
    #[arg(long, conflicts_with_all = ["ours", "union", "strategy"])]
    pub theirs: bool,

    /// Shorthand for --merge-strategy=union
    #[arg(long, conflicts_with_all = ["ours", "theirs", "strategy"])]
    pub union: bool,

    /// Path to the current version of the file
    pub current: String,

    /// Path to the base (common ancestor) version of the file
    pub base: String,

    /// Path to the other version of the file
    pub other: String,
}

impl MergeFileArgs {
    fn conflict_style(&self) -> Option<ConflictStyle> {
        if self.ours {
            Some(ConflictStyle::Ours)
        } else if self.theirs {
            Some(ConflictStyle::Theirs)
        } else if self.union {
            Some(ConflictStyle::Union)
        } else {
            self.merge_strategy
        }
    }
}

pub fn run(args: &MergeFileArgs, _cli: &Cli) -> Result<i32> {
    let current_content = fs::read_to_string(&args.current)?;
    let base_content = fs::read_to_string(&args.base)?;
    let other_content = fs::read_to_string(&args.other)?;

    let current_label = args
        .label
        .first()
        .map(|s| s.as_str())
        .unwrap_or(&args.current);
    let base_label = args
        .label
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or(&args.base);
    let other_label = args
        .label
        .get(2)
        .map(|s| s.as_str())
        .unwrap_or(&args.other);

    let conflict_style = args.conflict_style();

    let (merged, had_conflicts) = three_way_merge(
        &base_content,
        &current_content,
        &other_content,
        current_label,
        base_label,
        other_label,
        args.diff3,
        conflict_style,
    );

    if args.stdout {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        out.write_all(merged.as_bytes())?;
    } else {
        fs::write(&args.current, &merged)?;
    }

    if had_conflicts && !args.quiet {
        let stderr = io::stderr();
        let mut err = stderr.lock();
        writeln!(err, "warning: conflicts found in {}", args.current)?;
    }

    if had_conflicts {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Perform a three-way merge of text content.
///
/// Returns (merged_text, had_conflicts).
#[allow(clippy::too_many_arguments)]
fn three_way_merge(
    base: &str,
    current: &str,
    other: &str,
    current_label: &str,
    base_label: &str,
    other_label: &str,
    diff3: bool,
    conflict_style: Option<ConflictStyle>,
) -> (String, bool) {
    let base_lines: Vec<&str> = split_lines(base);
    let current_lines: Vec<&str> = split_lines(current);
    let other_lines: Vec<&str> = split_lines(other);

    // Compute diffs from base to current and base to other using LCS
    let current_hunks = diff_lines(&base_lines, &current_lines);
    let other_hunks = diff_lines(&base_lines, &other_lines);

    // Walk through the base, applying non-overlapping changes, and detecting conflicts
    let mut result = String::new();
    let mut had_conflicts = false;
    let mut base_idx: usize = 0;

    let mut ci = 0; // index into current_hunks
    let mut oi = 0; // index into other_hunks

    while ci < current_hunks.len() || oi < other_hunks.len() {
        let c_hunk = current_hunks.get(ci);
        let o_hunk = other_hunks.get(oi);

        match (c_hunk, o_hunk) {
            (Some(ch), Some(oh)) => {
                if ch.base_end <= oh.base_start {
                    // Current hunk comes first, no overlap
                    append_lines(&mut result, &base_lines, base_idx, ch.base_start);
                    append_strs(&mut result, &ch.replacement);
                    base_idx = ch.base_end;
                    ci += 1;
                } else if oh.base_end <= ch.base_start {
                    // Other hunk comes first, no overlap
                    append_lines(&mut result, &base_lines, base_idx, oh.base_start);
                    append_strs(&mut result, &oh.replacement);
                    base_idx = oh.base_end;
                    oi += 1;
                } else {
                    // Overlapping hunks -- potential conflict
                    // Find the combined region in base
                    let overlap_start = ch.base_start.min(oh.base_start);
                    let overlap_end = ch.base_end.max(oh.base_end);

                    // Output any base lines before the overlap region
                    append_lines(&mut result, &base_lines, base_idx, overlap_start);

                    // Check if both sides made the same change
                    if ch.replacement == oh.replacement {
                        // Identical changes: no conflict
                        append_strs(&mut result, &ch.replacement);
                    } else {
                        // Real conflict
                        match conflict_style {
                            Some(ConflictStyle::Ours) => {
                                append_strs(&mut result, &ch.replacement);
                            }
                            Some(ConflictStyle::Theirs) => {
                                append_strs(&mut result, &oh.replacement);
                            }
                            Some(ConflictStyle::Union) => {
                                append_strs(&mut result, &ch.replacement);
                                append_strs(&mut result, &oh.replacement);
                            }
                            None => {
                                had_conflicts = true;
                                result.push_str(&format!("<<<<<<< {}\n", current_label));
                                append_strs(&mut result, &ch.replacement);
                                if diff3 {
                                    result.push_str(&format!("||||||| {}\n", base_label));
                                    append_lines(&mut result, &base_lines, overlap_start, overlap_end);
                                }
                                result.push_str("=======\n");
                                append_strs(&mut result, &oh.replacement);
                                result.push_str(&format!(">>>>>>> {}\n", other_label));
                            }
                        }
                    }

                    base_idx = overlap_end;

                    // Advance both hunk iterators past the overlap region
                    ci += 1;
                    oi += 1;

                    // Also advance any additional hunks that fall within the overlap
                    while ci < current_hunks.len()
                        && current_hunks[ci].base_start < overlap_end
                    {
                        ci += 1;
                    }
                    while oi < other_hunks.len()
                        && other_hunks[oi].base_start < overlap_end
                    {
                        oi += 1;
                    }
                }
            }
            (Some(ch), None) => {
                append_lines(&mut result, &base_lines, base_idx, ch.base_start);
                append_strs(&mut result, &ch.replacement);
                base_idx = ch.base_end;
                ci += 1;
            }
            (None, Some(oh)) => {
                append_lines(&mut result, &base_lines, base_idx, oh.base_start);
                append_strs(&mut result, &oh.replacement);
                base_idx = oh.base_end;
                oi += 1;
            }
            (None, None) => break,
        }
    }

    // Append any remaining base lines
    append_lines(&mut result, &base_lines, base_idx, base_lines.len());

    (result, had_conflicts)
}

/// A hunk representing a changed region from the base.
#[derive(Debug)]
struct MergeHunk {
    /// Start index in base (inclusive)
    base_start: usize,
    /// End index in base (exclusive)
    base_end: usize,
    /// The replacement lines (from the derived version)
    replacement: Vec<String>,
}

/// Split text into lines, preserving line endings.
fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines().collect()
}

/// Compute the longest common subsequence table for two slices of lines.
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let m = a.len();
    let n = b.len();
    let mut table = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = table[i - 1][j].max(table[i][j - 1]);
            }
        }
    }

    table
}

/// Compute the diff hunks between base and derived using LCS.
fn diff_lines(base: &[&str], derived: &[&str]) -> Vec<MergeHunk> {
    let table = lcs_table(base, derived);
    let mut hunks = Vec::new();

    // Walk back through the LCS table to find changed regions
    let mut i = base.len();
    let mut j = derived.len();

    // Collect edit operations in reverse
    #[derive(Debug)]
    #[allow(dead_code)]
    enum Op {
        Equal,
        Delete(usize),      // index in base
        Insert(usize),      // index in derived
    }

    let mut ops = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && base[i - 1] == derived[j - 1] {
            ops.push(Op::Equal);
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || table[i][j - 1] >= table[i - 1][j]) {
            ops.push(Op::Insert(j - 1));
            j -= 1;
        } else {
            ops.push(Op::Delete(i - 1));
            i -= 1;
        }
    }

    ops.reverse();

    // Group consecutive non-Equal operations into hunks
    let mut idx = 0;
    let mut base_pos = 0;
    let mut derived_pos = 0;

    while idx < ops.len() {
        match ops[idx] {
            Op::Equal => {
                base_pos += 1;
                derived_pos += 1;
                idx += 1;
            }
            _ => {
                // Start of a changed region
                let hunk_base_start = base_pos;
                let hunk_derived_start = derived_pos;

                while idx < ops.len() && !matches!(ops[idx], Op::Equal) {
                    match ops[idx] {
                        Op::Delete(_) => {
                            base_pos += 1;
                        }
                        Op::Insert(_) => {
                            derived_pos += 1;
                        }
                        Op::Equal => unreachable!(),
                    }
                    idx += 1;
                }

                let replacement: Vec<String> = derived[hunk_derived_start..derived_pos]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                hunks.push(MergeHunk {
                    base_start: hunk_base_start,
                    base_end: base_pos,
                    replacement,
                });
            }
        }
    }

    hunks
}

/// Append lines from a slice to the result string.
fn append_lines(result: &mut String, lines: &[&str], from: usize, to: usize) {
    for line in lines.iter().skip(from).take(to.saturating_sub(from)) {
        result.push_str(line);
        result.push('\n');
    }
}

/// Append replacement strings to the result.
fn append_strs(result: &mut String, lines: &[String]) {
    for line in lines {
        result.push_str(line);
        result.push('\n');
    }
}

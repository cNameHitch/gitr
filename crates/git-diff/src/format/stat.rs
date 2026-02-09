//! --stat, --shortstat, and --numstat output formats.

use bstr::ByteSlice;

use crate::{DiffOptions, DiffResult, FileStatus};

/// Format a DiffResult as `--stat` output.
pub fn format_stat(result: &DiffResult, options: &DiffOptions) -> String {
    if result.is_empty() {
        return String::new();
    }

    let stat_width = options.stat_width.unwrap_or(80);
    let mut out = String::new();

    // Calculate max filename width
    let max_name_width = result
        .files
        .iter()
        .map(|f| display_path(f).len())
        .max()
        .unwrap_or(0);

    // Graph width is stat_width minus name and padding
    let graph_width = stat_width
        .saturating_sub(max_name_width)
        .saturating_sub(12); // " | NNN " + some padding

    let max_changes = result
        .files
        .iter()
        .map(|f| f.insertions() + f.deletions())
        .max()
        .unwrap_or(1)
        .max(1);

    // Calculate the width needed for the count field
    let count_width = format!("{}", max_changes).len();

    for file in &result.files {
        let path = display_path(file);
        let ins = file.insertions();
        let del = file.deletions();
        let total = ins + del;

        if file.is_binary {
            out.push_str(&format!(
                " {:<width$} | Bin\n",
                path,
                width = max_name_width
            ));
        } else {
            // Scale the graph
            let scaled_ins = if max_changes > graph_width {
                (ins * graph_width) / max_changes
            } else {
                ins
            };
            let scaled_del = if max_changes > graph_width {
                (del * graph_width) / max_changes
            } else {
                del
            };

            let graph: String = std::iter::repeat('+')
                .take(scaled_ins)
                .chain(std::iter::repeat('-').take(scaled_del))
                .collect();

            out.push_str(&format!(
                " {:<name_width$} | {:>count_width$} {}\n",
                path,
                total,
                graph,
                name_width = max_name_width,
                count_width = count_width
            ));
        }
    }

    // Summary line
    out.push_str(&format_summary_line(result));

    out
}

/// Format a DiffResult as `--shortstat` output.
pub fn format_short_stat(result: &DiffResult) -> String {
    if result.is_empty() {
        return String::new();
    }
    format_summary_line(result)
}

/// Format a DiffResult as `--numstat` output.
pub fn format_numstat(result: &DiffResult) -> String {
    let mut out = String::new();
    for file in &result.files {
        let path = display_path(file);
        if file.is_binary {
            out.push_str(&format!("-\t-\t{}\n", path));
        } else {
            out.push_str(&format!("{}\t{}\t{}\n", file.insertions(), file.deletions(), path));
        }
    }
    out
}

/// Build the summary line (e.g., "3 files changed, 10 insertions(+), 5 deletions(-)").
fn format_summary_line(result: &DiffResult) -> String {
    let n = result.num_files_changed();
    let ins = result.insertions();
    let del = result.deletions();

    let mut parts = Vec::new();
    parts.push(format!(
        " {} file{} changed",
        n,
        if n == 1 { "" } else { "s" }
    ));
    if ins > 0 {
        parts.push(format!(
            " {} insertion{}(+)",
            ins,
            if ins == 1 { "" } else { "s" }
        ));
    }
    if del > 0 {
        parts.push(format!(
            " {} deletion{}(-)",
            del,
            if del == 1 { "" } else { "s" }
        ));
    }

    format!("{}\n", parts.join(","))
}

/// Get the display path for a file diff.
fn display_path(file: &crate::FileDiff) -> String {
    match file.status {
        FileStatus::Renamed | FileStatus::Copied => {
            let old = file
                .old_path
                .as_ref()
                .map(|p| p.to_str_lossy().into_owned())
                .unwrap_or_default();
            let new = file
                .new_path
                .as_ref()
                .map(|p| p.to_str_lossy().into_owned())
                .unwrap_or_default();
            // Find common prefix/suffix for compact display
            format!("{} => {}", old, new)
        }
        _ => file.path().to_str_lossy().into_owned(),
    }
}

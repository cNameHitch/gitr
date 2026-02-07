//! Combined diff format for merge commits.
//!
//! Shows changes from multiple parents simultaneously.

use crate::DiffResult;

/// Format a combined diff from multiple parent diffs.
///
/// Used for merge commits where each parent has a different diff.
pub fn format_combined(parent_diffs: &[DiffResult]) -> String {
    let mut out = String::new();

    if parent_diffs.is_empty() {
        return out;
    }

    // Collect all paths across all parents
    let mut all_paths: Vec<String> = Vec::new();
    for diff in parent_diffs {
        for file in &diff.files {
            let path = file.path().to_string();
            if !all_paths.contains(&path) {
                all_paths.push(path);
            }
        }
    }

    // For each path, show combined output
    let num_parents = parent_diffs.len();
    for path in &all_paths {
        // Combined diff header uses multiple colons
        let colons = ":".repeat(num_parents);
        out.push_str(&format!("{} combined diff for {}\n", colons, path));
    }

    out
}

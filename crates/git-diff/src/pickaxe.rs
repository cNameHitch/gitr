//! Pickaxe search: -S (string) and -G (regex) filtering.
//!
//! Filters diff output to only show files/hunks matching the search pattern.

use bstr::ByteSlice;
use regex::Regex;

use crate::{DiffLine, DiffResult, FileDiff};

/// Pickaxe search mode.
#[derive(Debug, Clone)]
pub enum PickaxeMode {
    /// -S: match files where the string count changed between old and new
    String(Vec<u8>),
    /// -G: match files where any added/removed line matches the regex
    Regex(Regex),
}

impl PickaxeMode {
    /// Create a string pickaxe (-S).
    pub fn string(pattern: &str) -> Self {
        Self::String(pattern.as_bytes().to_vec())
    }

    /// Create a regex pickaxe (-G).
    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self::Regex(Regex::new(pattern)?))
    }
}

/// Filter a DiffResult by pickaxe criteria.
pub fn filter_by_pickaxe(result: &DiffResult, mode: &PickaxeMode) -> DiffResult {
    let files: Vec<FileDiff> = result
        .files
        .iter()
        .filter(|file| file_matches_pickaxe(file, mode))
        .cloned()
        .collect();
    DiffResult { files }
}

/// Check if a file diff matches the pickaxe criteria.
fn file_matches_pickaxe(file: &FileDiff, mode: &PickaxeMode) -> bool {
    match mode {
        PickaxeMode::String(pattern) => {
            // -S: the number of occurrences of the pattern must differ
            // between the old and new versions of the file
            let mut old_count: usize = 0;
            let mut new_count: usize = 0;

            for hunk in &file.hunks {
                for line in &hunk.lines {
                    match line {
                        DiffLine::Deletion(s) => {
                            old_count += count_occurrences(s.as_bytes(), pattern);
                        }
                        DiffLine::Addition(s) => {
                            new_count += count_occurrences(s.as_bytes(), pattern);
                        }
                        DiffLine::Context(_) => {}
                    }
                }
            }

            old_count != new_count
        }
        PickaxeMode::Regex(re) => {
            // -G: any added or removed line matches the regex
            for hunk in &file.hunks {
                for line in &hunk.lines {
                    match line {
                        DiffLine::Addition(s) | DiffLine::Deletion(s) => {
                            if let Ok(text) = s.to_str() {
                                if re.is_match(text) {
                                    return true;
                                }
                            }
                        }
                        DiffLine::Context(_) => {}
                    }
                }
            }
            false
        }
    }
}

/// Count occurrences of a byte pattern in data.
fn count_occurrences(data: &[u8], pattern: &[u8]) -> usize {
    if pattern.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut start = 0;
    while start + pattern.len() <= data.len() {
        if let Some(pos) = data[start..].find(pattern) {
            count += 1;
            start += pos + pattern.len();
        } else {
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::BString;
    use crate::{Hunk, FileStatus};

    fn make_file_diff(additions: &[&str], deletions: &[&str]) -> FileDiff {
        let mut lines = Vec::new();
        for d in deletions {
            lines.push(DiffLine::Deletion(BString::from(*d)));
        }
        for a in additions {
            lines.push(DiffLine::Addition(BString::from(*a)));
        }
        FileDiff {
            status: FileStatus::Modified,
            old_path: Some(BString::from("file.txt")),
            new_path: Some(BString::from("file.txt")),
            old_mode: None,
            new_mode: None,
            old_oid: None,
            new_oid: None,
            hunks: vec![Hunk {
                old_start: 1,
                old_count: deletions.len() as u32,
                new_start: 1,
                new_count: additions.len() as u32,
                header: None,
                lines,
            }],
            is_binary: false,
            similarity: None,
        }
    }

    #[test]
    fn string_pickaxe_matches_added() {
        let file = make_file_diff(&["hello world"], &[]);
        assert!(file_matches_pickaxe(&file, &PickaxeMode::string("hello")));
    }

    #[test]
    fn string_pickaxe_no_change() {
        // Same pattern in both old and new - no count change
        let file = make_file_diff(&["hello world"], &["hello earth"]);
        assert!(!file_matches_pickaxe(&file, &PickaxeMode::string("hello")));
    }

    #[test]
    fn regex_pickaxe_matches() {
        let file = make_file_diff(&["fn main() {"], &[]);
        let mode = PickaxeMode::regex(r"fn \w+").unwrap();
        assert!(file_matches_pickaxe(&file, &mode));
    }

    #[test]
    fn regex_pickaxe_no_match() {
        let file = make_file_diff(&["hello world"], &[]);
        let mode = PickaxeMode::regex(r"fn \w+").unwrap();
        assert!(!file_matches_pickaxe(&file, &mode));
    }
}

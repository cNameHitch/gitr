//! Word-level diff formatting using `[-removed-]{+added+}` markers.

use bstr::ByteSlice;

use crate::{DiffLine, DiffResult, FileStatus};

/// Format a DiffResult as word-level diff output.
pub fn format_word_diff(result: &DiffResult) -> String {
    let mut out = String::new();

    for file in &result.files {
        let old_path = file
            .old_path
            .as_ref()
            .map(|p| p.to_str_lossy().into_owned())
            .unwrap_or_default();
        let new_path = file
            .new_path
            .as_ref()
            .map(|p| p.to_str_lossy().into_owned())
            .unwrap_or_default();

        // Header
        out.push_str(&format!("diff --git a/{} b/{}\n", old_path, new_path));

        if file.is_binary {
            out.push_str(&format!(
                "Binary files a/{} and b/{} differ\n",
                old_path, new_path
            ));
            continue;
        }

        if file.hunks.is_empty() {
            continue;
        }

        // File headers
        if file.status == FileStatus::Added {
            out.push_str("--- /dev/null\n");
        } else {
            out.push_str(&format!("--- a/{}\n", old_path));
        }

        if file.status == FileStatus::Deleted {
            out.push_str("+++ /dev/null\n");
        } else {
            out.push_str(&format!("+++ b/{}\n", new_path));
        }

        // Hunks
        for hunk in &file.hunks {
            // Hunk header
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
            out.push_str(&format!("@@ -{} +{} @@", old_range, new_range));
            if let Some(ref header) = hunk.header {
                out.push(' ');
                out.push_str(&header.to_str_lossy());
            }
            out.push('\n');

            // Process lines with word-level diff
            let mut i = 0;
            let lines = &hunk.lines;
            while i < lines.len() {
                match &lines[i] {
                    DiffLine::Context(content) => {
                        out.push_str(&content.to_str_lossy());
                        if !content.ends_with(b"\n") {
                            out.push('\n');
                        }
                        i += 1;
                    }
                    DiffLine::Deletion(del_content) => {
                        // Collect consecutive deletions and additions for word diff
                        let mut deletions = vec![del_content.to_str_lossy().into_owned()];
                        let mut j = i + 1;
                        while j < lines.len() {
                            if let DiffLine::Deletion(c) = &lines[j] {
                                deletions.push(c.to_str_lossy().into_owned());
                                j += 1;
                            } else {
                                break;
                            }
                        }
                        let mut additions = Vec::new();
                        while j < lines.len() {
                            if let DiffLine::Addition(c) = &lines[j] {
                                additions.push(c.to_str_lossy().into_owned());
                                j += 1;
                            } else {
                                break;
                            }
                        }

                        let old_text = deletions.join("");
                        let new_text = additions.join("");
                        out.push_str(&word_diff_lines(&old_text, &new_text));
                        i = j;
                    }
                    DiffLine::Addition(add_content) => {
                        // Pure addition (no preceding deletion)
                        let text = add_content.to_str_lossy();
                        out.push_str(&format!("{{+{}+}}", text.trim_end_matches('\n')));
                        out.push('\n');
                        i += 1;
                    }
                }
            }
        }
    }

    out
}

/// Perform word-level diff between old and new text.
fn word_diff_lines(old: &str, new: &str) -> String {
    let old_words = tokenize_words(old);
    let new_words = tokenize_words(new);

    let lcs = lcs_words(&old_words, &new_words);

    let mut result = String::new();
    let mut oi = 0;
    let mut ni = 0;

    for (o_idx, n_idx) in lcs {
        // Emit deletions before this match
        if oi < o_idx {
            let deleted: String = old_words[oi..o_idx].concat();
            result.push_str(&format!("[-{}-]", deleted.trim_end_matches('\n')));
        }
        // Emit additions before this match
        if ni < n_idx {
            let added: String = new_words[ni..n_idx].concat();
            result.push_str(&format!("{{+{}+}}", added.trim_end_matches('\n')));
        }
        // Emit the matched word
        result.push_str(&new_words[n_idx]);
        oi = o_idx + 1;
        ni = n_idx + 1;
    }

    // Remaining deletions
    if oi < old_words.len() {
        let deleted: String = old_words[oi..].concat();
        result.push_str(&format!("[-{}-]", deleted.trim_end_matches('\n')));
    }
    // Remaining additions
    if ni < new_words.len() {
        let added: String = new_words[ni..].concat();
        result.push_str(&format!("{{+{}+}}", added.trim_end_matches('\n')));
    }

    if !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// Tokenize text into words (split on whitespace boundaries, keeping whitespace as tokens).
fn tokenize_words(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_whitespace = false;

    for ch in text.chars() {
        let is_ws = ch.is_whitespace() && ch != '\n';
        if ch == '\n' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push("\n".to_string());
            in_whitespace = false;
        } else if is_ws != in_whitespace && !current.is_empty() {
            tokens.push(current.clone());
            current.clear();
            current.push(ch);
            in_whitespace = is_ws;
        } else {
            if !is_ws && !current.is_empty() {
                // Split on word boundaries (non-alphanumeric)
                let last = current.chars().last().unwrap();
                let both_alnum = last.is_alphanumeric() && ch.is_alphanumeric();
                if !both_alnum && !in_whitespace {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            current.push(ch);
            in_whitespace = is_ws;
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Compute LCS of two word sequences, returning matched index pairs.
fn lcs_words(old: &[String], new: &[String]) -> Vec<(usize, usize)> {
    let m = old.len();
    let n = new.len();

    // DP table
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            if old[i] == new[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Backtrack to find LCS
    let mut result = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < m && j < n {
        if old[i] == new[j] {
            result.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }

    result
}

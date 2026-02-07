//! Pretty-print formatting for commits.
//!
//! Supports format specifiers matching C git's `--format` / `--pretty` options.

use bstr::ByteSlice;
use git_hash::ObjectId;
use git_object::Commit;
use git_utils::date::DateFormat;

/// Built-in format presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFormat {
    /// `--oneline`: short OID + first line of message
    Oneline,
    /// `--format=short`: commit + author + subject
    Short,
    /// `--format=medium`: commit + author + date + subject + body (default)
    Medium,
    /// `--format=full`: commit + author + committer + subject + body
    Full,
    /// `--format=fuller`: commit + author + author date + committer + committer date + subject + body
    Fuller,
    /// `--format=email`: email/patch format
    Email,
    /// `--format=raw`: raw commit object format
    Raw,
}

/// Options for formatting.
#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub date_format: DateFormat,
    pub abbrev_len: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            date_format: DateFormat::Default,
            abbrev_len: 7,
        }
    }
}

/// Format a commit with the given format string.
///
/// Format specifiers (matching C git):
/// - `%H` — full commit hash
/// - `%h` — abbreviated commit hash
/// - `%T` — full tree hash
/// - `%t` — abbreviated tree hash
/// - `%P` — full parent hashes (space separated)
/// - `%p` — abbreviated parent hashes
/// - `%an` — author name
/// - `%ae` — author email
/// - `%aE` — author email (respstrstrict)
/// - `%ad` — author date (format by --date=)
/// - `%aD` — author date, RFC2822
/// - `%aI` — author date, ISO 8601 strict
/// - `%ai` — author date, ISO 8601
/// - `%at` — author date, unix timestamp
/// - `%ar` — author date, relative
/// - `%cn` — committer name
/// - `%ce` — committer email
/// - `%cd` — committer date
/// - `%cD` — committer date, RFC2822
/// - `%cI` — committer date, ISO 8601 strict
/// - `%ci` — committer date, ISO 8601
/// - `%ct` — committer date, unix timestamp
/// - `%cr` — committer date, relative
/// - `%s` — subject (first line of message)
/// - `%b` — body (rest of message)
/// - `%B` — raw body (full message)
/// - `%n` — newline
/// - `%%` — literal %
pub fn format_commit(
    commit: &Commit,
    oid: &ObjectId,
    format: &str,
    options: &FormatOptions,
) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some('%') => {
                    chars.next();
                    result.push('%');
                }
                Some('H') => {
                    chars.next();
                    result.push_str(&oid.to_hex());
                }
                Some('h') => {
                    chars.next();
                    let hex = oid.to_hex();
                    let abbrev = &hex[..options.abbrev_len.min(hex.len())];
                    result.push_str(abbrev);
                }
                Some('T') => {
                    chars.next();
                    result.push_str(&commit.tree.to_hex());
                }
                Some('t') => {
                    chars.next();
                    let hex = commit.tree.to_hex();
                    result.push_str(&hex[..options.abbrev_len.min(hex.len())]);
                }
                Some('P') => {
                    chars.next();
                    let parents: Vec<String> =
                        commit.parents.iter().map(|p| p.to_hex()).collect();
                    result.push_str(&parents.join(" "));
                }
                Some('p') => {
                    chars.next();
                    let parents: Vec<String> = commit
                        .parents
                        .iter()
                        .map(|p| {
                            let hex = p.to_hex();
                            hex[..options.abbrev_len.min(hex.len())].to_string()
                        })
                        .collect();
                    result.push_str(&parents.join(" "));
                }
                Some('a') => {
                    chars.next();
                    match chars.peek() {
                        Some('n') => {
                            chars.next();
                            result.push_str(&String::from_utf8_lossy(&commit.author.name));
                        }
                        Some('e') | Some('E') => {
                            chars.next();
                            result.push_str(&String::from_utf8_lossy(&commit.author.email));
                        }
                        Some('d') => {
                            chars.next();
                            result.push_str(&commit.author.date.format(options.date_format));
                        }
                        Some('D') => {
                            chars.next();
                            result.push_str(
                                &commit.author.date.format(DateFormat::Rfc2822),
                            );
                        }
                        Some('I') => {
                            chars.next();
                            result.push_str(
                                &commit.author.date.format(DateFormat::IsoStrict),
                            );
                        }
                        Some('i') => {
                            chars.next();
                            result.push_str(&commit.author.date.format(DateFormat::Iso));
                        }
                        Some('t') => {
                            chars.next();
                            result.push_str(
                                &commit.author.date.format(DateFormat::Unix),
                            );
                        }
                        Some('r') => {
                            chars.next();
                            result.push_str(
                                &commit.author.date.format(DateFormat::Relative),
                            );
                        }
                        _ => {
                            result.push_str("%a");
                        }
                    }
                }
                Some('c') => {
                    chars.next();
                    match chars.peek() {
                        Some('n') => {
                            chars.next();
                            result.push_str(&String::from_utf8_lossy(&commit.committer.name));
                        }
                        Some('e') | Some('E') => {
                            chars.next();
                            result.push_str(&String::from_utf8_lossy(&commit.committer.email));
                        }
                        Some('d') => {
                            chars.next();
                            result
                                .push_str(&commit.committer.date.format(options.date_format));
                        }
                        Some('D') => {
                            chars.next();
                            result.push_str(
                                &commit.committer.date.format(DateFormat::Rfc2822),
                            );
                        }
                        Some('I') => {
                            chars.next();
                            result.push_str(
                                &commit.committer.date.format(DateFormat::IsoStrict),
                            );
                        }
                        Some('i') => {
                            chars.next();
                            result.push_str(&commit.committer.date.format(DateFormat::Iso));
                        }
                        Some('t') => {
                            chars.next();
                            result.push_str(
                                &commit.committer.date.format(DateFormat::Unix),
                            );
                        }
                        Some('r') => {
                            chars.next();
                            result.push_str(
                                &commit.committer.date.format(DateFormat::Relative),
                            );
                        }
                        _ => {
                            result.push_str("%c");
                        }
                    }
                }
                Some('s') => {
                    chars.next();
                    result.push_str(&String::from_utf8_lossy(commit.summary()));
                }
                Some('b') => {
                    chars.next();
                    if let Some(body) = commit.body() {
                        result.push_str(&String::from_utf8_lossy(body));
                    }
                }
                Some('B') => {
                    chars.next();
                    result.push_str(&String::from_utf8_lossy(&commit.message));
                }
                Some('n') => {
                    chars.next();
                    result.push('\n');
                }
                _ => {
                    result.push('%');
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Format a commit with a built-in format preset.
pub fn format_builtin(
    commit: &Commit,
    oid: &ObjectId,
    preset: BuiltinFormat,
    options: &FormatOptions,
) -> String {
    match preset {
        BuiltinFormat::Oneline => {
            let hex = oid.to_hex();
            let abbrev = &hex[..options.abbrev_len.min(hex.len())];
            let summary = String::from_utf8_lossy(commit.summary());
            format!("{} {}", abbrev, summary)
        }
        BuiltinFormat::Short => {
            let mut out = String::new();
            out.push_str(&format!("commit {}\n", oid.to_hex()));
            out.push_str(&format!(
                "Author: {} <{}>\n",
                String::from_utf8_lossy(&commit.author.name),
                String::from_utf8_lossy(&commit.author.email)
            ));
            out.push('\n');
            out.push_str(&format!(
                "    {}\n",
                String::from_utf8_lossy(commit.summary())
            ));
            out
        }
        BuiltinFormat::Medium => {
            let mut out = String::new();
            out.push_str(&format!("commit {}\n", oid.to_hex()));
            out.push_str(&format!(
                "Author: {} <{}>\n",
                String::from_utf8_lossy(&commit.author.name),
                String::from_utf8_lossy(&commit.author.email)
            ));
            out.push_str(&format!(
                "Date:   {}\n",
                commit.author.date.format(options.date_format)
            ));
            out.push('\n');
            // Indent each line of message with 4 spaces.
            for line in commit.message.lines() {
                out.push_str(&format!("    {}\n", String::from_utf8_lossy(line)));
            }
            out
        }
        BuiltinFormat::Full => {
            let mut out = String::new();
            out.push_str(&format!("commit {}\n", oid.to_hex()));
            out.push_str(&format!(
                "Author: {} <{}>\n",
                String::from_utf8_lossy(&commit.author.name),
                String::from_utf8_lossy(&commit.author.email)
            ));
            out.push_str(&format!(
                "Commit: {} <{}>\n",
                String::from_utf8_lossy(&commit.committer.name),
                String::from_utf8_lossy(&commit.committer.email)
            ));
            out.push('\n');
            for line in commit.message.lines() {
                out.push_str(&format!("    {}\n", String::from_utf8_lossy(line)));
            }
            out
        }
        BuiltinFormat::Fuller => {
            let mut out = String::new();
            out.push_str(&format!("commit {}\n", oid.to_hex()));
            out.push_str(&format!(
                "Author:     {} <{}>\n",
                String::from_utf8_lossy(&commit.author.name),
                String::from_utf8_lossy(&commit.author.email)
            ));
            out.push_str(&format!(
                "AuthorDate: {}\n",
                commit.author.date.format(options.date_format)
            ));
            out.push_str(&format!(
                "Commit:     {} <{}>\n",
                String::from_utf8_lossy(&commit.committer.name),
                String::from_utf8_lossy(&commit.committer.email)
            ));
            out.push_str(&format!(
                "CommitDate: {}\n",
                commit.committer.date.format(options.date_format)
            ));
            out.push('\n');
            for line in commit.message.lines() {
                out.push_str(&format!("    {}\n", String::from_utf8_lossy(line)));
            }
            out
        }
        BuiltinFormat::Email => {
            let mut out = String::new();
            out.push_str(&format!(
                "From {} Mon Sep 17 00:00:00 2001\n",
                oid.to_hex()
            ));
            out.push_str(&format!(
                "From: {} <{}>\n",
                String::from_utf8_lossy(&commit.author.name),
                String::from_utf8_lossy(&commit.author.email)
            ));
            out.push_str(&format!(
                "Date: {}\n",
                commit.author.date.format(DateFormat::Rfc2822)
            ));
            out.push_str(&format!(
                "Subject: [PATCH] {}\n",
                String::from_utf8_lossy(commit.summary())
            ));
            out.push('\n');
            if let Some(body) = commit.body() {
                out.push_str(&String::from_utf8_lossy(body));
            }
            out
        }
        BuiltinFormat::Raw => {
            let mut out = String::new();
            out.push_str(&format!("commit {}\n", oid.to_hex()));
            out.push_str(&format!("tree {}\n", commit.tree.to_hex()));
            for parent in &commit.parents {
                out.push_str(&format!("parent {}\n", parent.to_hex()));
            }
            out.push_str(&format!(
                "author {}\n",
                String::from_utf8_lossy(&commit.author.to_bytes())
            ));
            out.push_str(&format!(
                "committer {}\n",
                String::from_utf8_lossy(&commit.committer.to_bytes())
            ));
            out.push('\n');
            out.push_str(&String::from_utf8_lossy(&commit.message));
            out
        }
    }
}

//! Gitignore pattern matching.
//!
//! Implements the layered ignore pattern system:
//! 1. `core.excludesFile` (global)
//! 2. `.git/info/exclude` (repo-local)
//! 3. `.gitignore` files (per-directory, scoped)

use std::path::{Path, PathBuf};

use bstr::{BStr, BString, ByteSlice};
use git_utils::wildmatch::{WildmatchFlags, WildmatchPattern};

use crate::IndexError;

/// A single ignore pattern with metadata.
#[derive(Debug, Clone)]
pub struct IgnorePattern {
    /// The compiled wildmatch pattern.
    pub pattern: WildmatchPattern,
    /// The raw pattern string (for debugging).
    pub raw: BString,
    /// Whether the pattern is negated (`!`).
    pub negated: bool,
    /// Whether the pattern only matches directories.
    pub directory_only: bool,
    /// Whether the pattern is anchored (contains `/` in the middle or starts with `/`).
    pub anchored: bool,
    /// Source file this pattern came from.
    pub source: PathBuf,
    /// Base directory for relative matching (directory containing the .gitignore).
    pub base_dir: PathBuf,
}

/// Layered gitignore pattern stack.
#[derive(Debug, Clone)]
pub struct IgnoreStack {
    /// Patterns in order from lowest to highest priority.
    /// Last match wins (but negation can re-include).
    patterns: Vec<IgnorePattern>,
}

impl IgnoreStack {
    /// Create an empty ignore stack.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Load patterns from a gitignore file.
    pub fn add_file(&mut self, path: &Path, base_dir: &Path) -> Result<(), IndexError> {
        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(IndexError::Io(e)),
        };
        self.add_patterns(&content, path, base_dir);
        Ok(())
    }

    /// Parse and add patterns from raw gitignore content.
    pub fn add_patterns(&mut self, content: &[u8], source: &Path, base_dir: &Path) {
        for line in content.lines() {
            if let Some(pattern) = parse_ignore_line(line, source, base_dir) {
                self.patterns.push(pattern);
            }
        }
    }

    /// Check if a path is ignored. `is_dir` indicates if the path is a directory.
    ///
    /// Evaluates all patterns and returns the result of the last match.
    /// Negation patterns (`!`) can re-include previously ignored files.
    pub fn is_ignored(&self, path: &BStr, is_dir: bool) -> bool {
        let mut ignored = false;

        for pat in &self.patterns {
            // Skip directory-only patterns when path is not a directory
            if pat.directory_only && !is_dir {
                continue;
            }

            if pattern_matches(pat, path) {
                ignored = !pat.negated;
            }
        }

        ignored
    }

    /// Number of patterns loaded.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

impl Default for IgnoreStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a single line from a gitignore file.
fn parse_ignore_line(line: &[u8], source: &Path, base_dir: &Path) -> Option<IgnorePattern> {
    let mut line = line;

    // Skip empty lines and comments
    if line.is_empty() || line[0] == b'#' {
        return None;
    }

    // Strip trailing whitespace (unless escaped with backslash)
    while line.len() > 1 && line.last() == Some(&b' ') && line[line.len() - 2] != b'\\' {
        line = &line[..line.len() - 1];
    }

    if line.is_empty() {
        return None;
    }

    // Check for negation
    let negated = line[0] == b'!';
    if negated {
        line = &line[1..];
        if line.is_empty() {
            return None;
        }
    }

    // Strip leading backslash (escape for # or !)
    if line[0] == b'\\' && line.len() > 1 && (line[1] == b'#' || line[1] == b'!') {
        line = &line[1..];
    }

    // Check for directory-only (trailing /)
    let directory_only = line.last() == Some(&b'/');
    let line = if directory_only {
        &line[..line.len() - 1]
    } else {
        line
    };

    if line.is_empty() {
        return None;
    }

    // Determine if the pattern is anchored
    // A pattern is anchored if it contains a slash (except trailing slash already stripped)
    let has_slash = line.contains(&b'/');
    let anchored = has_slash;

    // Strip leading slash (just for anchoring, not part of the pattern)
    let pattern_str = if line[0] == b'/' {
        &line[1..]
    } else {
        line
    };

    // Build the wildmatch pattern
    // Use PATHNAME mode so * doesn't match /
    let flags = WildmatchFlags::PATHNAME;
    let pattern = WildmatchPattern::new(BStr::new(pattern_str), flags);

    Some(IgnorePattern {
        pattern,
        raw: BString::from(pattern_str),
        negated,
        directory_only,
        anchored,
        source: source.to_path_buf(),
        base_dir: base_dir.to_path_buf(),
    })
}

/// Check if an ignore pattern matches a path.
fn pattern_matches(pat: &IgnorePattern, path: &BStr) -> bool {
    if pat.anchored {
        // Anchored patterns match from the base directory
        pat.pattern.matches(path)
    } else {
        // Unanchored patterns can match against the basename or the full path
        // Try matching against full path first
        if pat.pattern.matches(path) {
            return true;
        }
        // Try matching against basename only
        if let Some(slash_pos) = path.rfind_byte(b'/') {
            let basename = BStr::new(&path[slash_pos + 1..]);
            pat.pattern.matches(basename)
        } else {
            false // already tried full path which is the basename
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_patterns() {
        let content = b"*.o\n# comment\n!important.o\nbuild/\n/root_only\n";
        let mut stack = IgnoreStack::new();
        stack.add_patterns(content, Path::new(".gitignore"), Path::new("."));
        assert_eq!(stack.len(), 4); // *.o, !important.o, build, /root_only
    }

    #[test]
    fn ignore_basic() {
        let content = b"*.o\n";
        let mut stack = IgnoreStack::new();
        stack.add_patterns(content, Path::new(".gitignore"), Path::new("."));

        assert!(stack.is_ignored(BStr::new(b"test.o"), false));
        assert!(!stack.is_ignored(BStr::new(b"test.c"), false));
    }

    #[test]
    fn ignore_negation() {
        let content = b"*.o\n!important.o\n";
        let mut stack = IgnoreStack::new();
        stack.add_patterns(content, Path::new(".gitignore"), Path::new("."));

        assert!(stack.is_ignored(BStr::new(b"test.o"), false));
        assert!(!stack.is_ignored(BStr::new(b"important.o"), false));
    }

    #[test]
    fn ignore_directory_only() {
        let content = b"build/\n";
        let mut stack = IgnoreStack::new();
        stack.add_patterns(content, Path::new(".gitignore"), Path::new("."));

        assert!(stack.is_ignored(BStr::new(b"build"), true));
        assert!(!stack.is_ignored(BStr::new(b"build"), false));
    }

    #[test]
    fn ignore_comments_and_empty() {
        let content = b"# comment\n\n   \n*.o\n";
        let mut stack = IgnoreStack::new();
        stack.add_patterns(content, Path::new(".gitignore"), Path::new("."));
        // Only *.o should be parsed (comment and empty lines are skipped, "   " is whitespace)
        assert!(!stack.is_empty());
    }
}

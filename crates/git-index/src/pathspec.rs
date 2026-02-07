//! Pathspec matching for file filtering.
//!
//! Supports magic signatures: `:(top)`, `:(exclude)` / `:!`, `:(icase)`,
//! `:(glob)`, `:(literal)`.

use bstr::{BStr, BString, ByteSlice};
use git_utils::wildmatch::{WildmatchFlags, WildmatchPattern};

use crate::IndexError;

/// Parsed pathspec pattern with magic signatures.
#[derive(Debug, Clone)]
pub struct PathspecPattern {
    /// The raw input string.
    pub raw: BString,
    /// The extracted pattern (after magic prefix).
    pub pattern: BString,
    /// Magic flags.
    pub magic: PathspecMagic,
    /// Compiled wildmatch pattern (if glob mode).
    compiled: Option<WildmatchPattern>,
}

/// Magic signature flags for pathspecs.
#[derive(Debug, Clone, Default)]
pub struct PathspecMagic {
    /// `:(top)` — pattern is relative to repo root.
    pub top: bool,
    /// `:(exclude)` or `:!` — exclude matching paths.
    pub exclude: bool,
    /// `:(icase)` — case-insensitive matching.
    pub icase: bool,
    /// `:(glob)` — use glob matching (default for most patterns).
    pub glob: bool,
    /// `:(literal)` — no wildcard expansion.
    pub literal: bool,
}

/// Collection of pathspec patterns for matching.
#[derive(Debug, Clone)]
pub struct Pathspec {
    pub patterns: Vec<PathspecPattern>,
}

impl Pathspec {
    /// Parse pathspec patterns from string slices.
    pub fn parse(patterns: &[&str]) -> Result<Self, IndexError> {
        let mut parsed = Vec::with_capacity(patterns.len());
        for &pat in patterns {
            parsed.push(PathspecPattern::parse(pat)?);
        }
        Ok(Pathspec { patterns: parsed })
    }

    /// Check if a path matches this pathspec set.
    ///
    /// A path matches if it matches any include pattern and no exclude pattern.
    /// If there are no include patterns, all paths are included by default.
    pub fn matches(&self, path: &BStr, is_dir: bool) -> bool {
        if self.patterns.is_empty() {
            return true;
        }

        let has_includes = self.patterns.iter().any(|p| !p.magic.exclude);
        let mut included = !has_includes; // if no includes, include everything

        for pat in &self.patterns {
            if pat.magic.exclude {
                if pat.matches_path(path, is_dir) {
                    return false;
                }
            } else if pat.matches_path(path, is_dir) {
                included = true;
            }
        }

        included
    }
}

impl PathspecPattern {
    /// Parse a single pathspec string.
    pub fn parse(input: &str) -> Result<Self, IndexError> {
        let raw = BString::from(input);
        let mut magic = PathspecMagic::default();
        let pattern_str;

        if input.starts_with(":(") {
            // Long-form magic: :(top,exclude,icase)pattern
            let close = input.find(')').ok_or_else(|| {
                IndexError::InvalidPathspec(format!("unclosed magic in: {input}"))
            })?;
            let magic_str = &input[2..close];
            pattern_str = &input[close + 1..];

            for word in magic_str.split(',') {
                match word {
                    "top" => magic.top = true,
                    "exclude" => magic.exclude = true,
                    "icase" => magic.icase = true,
                    "glob" => magic.glob = true,
                    "literal" => magic.literal = true,
                    "" => {}
                    other => {
                        return Err(IndexError::InvalidPathspec(format!(
                            "unknown magic: {other}"
                        )));
                    }
                }
            }
        } else if input.starts_with(":!") || input.starts_with(":^") {
            // Short-form exclude
            magic.exclude = true;
            pattern_str = &input[2..];
        } else {
            pattern_str = input;
        }

        let pattern = BString::from(pattern_str);

        // Compile wildmatch pattern
        let mut flags = WildmatchFlags::PATHNAME;
        if magic.icase {
            flags |= WildmatchFlags::CASEFOLD;
        }

        let compiled = if magic.literal {
            None
        } else {
            Some(WildmatchPattern::new(BStr::new(pattern_str.as_bytes()), flags))
        };

        Ok(PathspecPattern {
            raw,
            pattern,
            magic,
            compiled,
        })
    }

    /// Check if this pattern matches a path.
    fn matches_path(&self, path: &BStr, _is_dir: bool) -> bool {
        if self.magic.literal {
            // Literal: exact prefix match
            if self.magic.icase {
                path.to_ascii_lowercase()
                    .starts_with(&self.pattern.to_ascii_lowercase())
            } else {
                path.starts_with(self.pattern.as_bytes())
            }
        } else if let Some(ref compiled) = self.compiled {
            // Glob match against full path
            if compiled.matches(path) {
                return true;
            }
            // Also try prefix match: if pattern is "src", match "src/foo.rs"
            if !self.pattern.iter().any(|&b| b == b'*' || b == b'?' || b == b'[') {
                // Pattern has no wildcards — treat as prefix
                let pat_bytes = self.pattern.as_bytes();
                if path.starts_with(pat_bytes) {
                    if path.len() == pat_bytes.len() {
                        return true;
                    }
                    if pat_bytes.last() == Some(&b'/') || path.get(pat_bytes.len()) == Some(&b'/') {
                        return true;
                    }
                }
            }
            false
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let ps = Pathspec::parse(&["src/*.rs"]).unwrap();
        assert_eq!(ps.patterns.len(), 1);
        assert!(!ps.patterns[0].magic.exclude);
    }

    #[test]
    fn parse_magic_exclude() {
        let ps = Pathspec::parse(&[":(exclude)*.test"]).unwrap();
        assert!(ps.patterns[0].magic.exclude);
    }

    #[test]
    fn parse_short_exclude() {
        let ps = Pathspec::parse(&[":!*.test"]).unwrap();
        assert!(ps.patterns[0].magic.exclude);
        assert_eq!(&ps.patterns[0].pattern[..], b"*.test");
    }

    #[test]
    fn parse_magic_top() {
        let ps = Pathspec::parse(&[":(top)README"]).unwrap();
        assert!(ps.patterns[0].magic.top);
    }

    #[test]
    fn match_glob() {
        let ps = Pathspec::parse(&["src/*.rs"]).unwrap();
        assert!(ps.matches(BStr::new(b"src/main.rs"), false));
        assert!(!ps.matches(BStr::new(b"src/sub/main.rs"), false));
        assert!(!ps.matches(BStr::new(b"lib/main.rs"), false));
    }

    #[test]
    fn match_exclude() {
        let ps = Pathspec::parse(&["src/*.rs", ":(exclude)src/*.test.rs"]).unwrap();
        assert!(ps.matches(BStr::new(b"src/main.rs"), false));
        assert!(!ps.matches(BStr::new(b"src/main.test.rs"), false));
    }

    #[test]
    fn match_prefix() {
        let ps = Pathspec::parse(&["src"]).unwrap();
        assert!(ps.matches(BStr::new(b"src/main.rs"), false));
        assert!(ps.matches(BStr::new(b"src/sub/file.rs"), false));
        assert!(!ps.matches(BStr::new(b"lib/main.rs"), false));
    }

    #[test]
    fn empty_pathspec_matches_all() {
        let ps = Pathspec::parse(&[]).unwrap();
        assert!(ps.matches(BStr::new(b"anything"), false));
    }
}

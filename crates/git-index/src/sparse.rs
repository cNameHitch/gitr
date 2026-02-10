//! Sparse checkout support.
//!
//! Implements cone-mode sparse checkout pattern parsing and working tree filtering.

use std::path::Path;
use bstr::{BStr, BString, ByteSlice};

/// Sparse checkout configuration.
#[derive(Debug, Clone)]
pub struct SparseCheckout {
    /// Whether sparse checkout is enabled.
    pub enabled: bool,
    /// Whether cone mode is active (default).
    pub cone_mode: bool,
    /// Include patterns (directories to include in cone mode).
    pub include_patterns: Vec<BString>,
    /// Exclude patterns (directories to exclude).
    pub exclude_patterns: Vec<BString>,
}

impl SparseCheckout {
    pub fn new() -> Self {
        Self {
            enabled: false,
            cone_mode: true,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    /// Load sparse checkout configuration from $GIT_DIR/info/sparse-checkout.
    pub fn from_file(git_dir: &Path) -> std::io::Result<Self> {
        let path = git_dir.join("info").join("sparse-checkout");
        let mut sc = Self::new();

        let content = match std::fs::read(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(sc),
            Err(e) => return Err(e),
        };

        sc.enabled = true;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line[0] == b'#' {
                continue;
            }

            if line[0] == b'!' {
                // Exclude pattern
                sc.exclude_patterns.push(BString::from(&line[1..]));
            } else {
                sc.include_patterns.push(BString::from(line));
            }
        }

        Ok(sc)
    }

    /// Check if a path should be included in the sparse working tree.
    pub fn is_included(&self, path: &BStr) -> bool {
        if !self.enabled {
            return true;
        }

        if self.include_patterns.is_empty() {
            return true;
        }

        // In cone mode, check if path is under any included directory
        if self.cone_mode {
            for pattern in &self.include_patterns {
                let pattern: &[u8] = pattern.as_ref();
                // Root pattern "/*" includes all top-level files
                if pattern == b"/*" {
                    if !path.contains(&b'/') {
                        return true;
                    }
                    continue;
                }

                // Directory pattern: path starts with the pattern
                if path.starts_with(pattern) {
                    return true;
                }
                // Pattern without trailing slash
                let pattern_dir = if pattern.ends_with(b"/") {
                    &pattern[..pattern.len() - 1]
                } else {
                    pattern
                };
                if path.starts_with(pattern_dir)
                    && (path.len() == pattern_dir.len()
                        || path.as_bytes().get(pattern_dir.len()) == Some(&b'/'))
                {
                    return true;
                }
            }
            return false;
        }

        // Non-cone mode: simple pattern matching
        for pattern in &self.include_patterns {
            if git_utils::wildmatch::wildmatch(
                BStr::new(pattern),
                path,
                git_utils::wildmatch::WildmatchFlags::PATHNAME,
            ) {
                return true;
            }
        }
        false
    }

    /// Save sparse checkout patterns to $GIT_DIR/info/sparse-checkout.
    pub fn save(&self, git_dir: &Path) -> std::io::Result<()> {
        let info_dir = git_dir.join("info");
        std::fs::create_dir_all(&info_dir)?;
        let path = info_dir.join("sparse-checkout");

        let mut content = String::new();
        for pattern in &self.include_patterns {
            if let Ok(s) = pattern.to_str() {
                content.push_str(s);
                content.push('\n');
            }
        }
        for pattern in &self.exclude_patterns {
            if let Ok(s) = pattern.to_str() {
                content.push('!');
                content.push_str(s);
                content.push('\n');
            }
        }

        std::fs::write(&path, content.as_bytes())
    }
}

impl Default for SparseCheckout {
    fn default() -> Self {
        Self::new()
    }
}

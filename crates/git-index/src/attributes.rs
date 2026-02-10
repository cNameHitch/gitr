//! Gitattributes processing.
//!
//! Reads `.gitattributes` files and provides attribute lookup for paths.
//! Attributes control line ending conversion, diff drivers, merge strategies, etc.

use std::path::{Path, PathBuf};

use bstr::{BStr, BString, ByteSlice};

use crate::IndexError;

/// A single attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// Attribute is set (e.g., `text`).
    Set,
    /// Attribute is unset (e.g., `-text`).
    Unset,
    /// Attribute has a string value (e.g., `text=auto`).
    Value(BString),
    /// Attribute is unspecified.
    Unspecified,
}

/// A single attribute assignment from a gitattributes file.
#[derive(Debug, Clone)]
pub struct AttributeRule {
    /// The glob pattern for matching paths.
    pub pattern: BString,
    /// Attribute name.
    pub name: BString,
    /// Attribute value.
    pub value: AttributeValue,
    /// Source file.
    pub source: PathBuf,
}

/// Layered attribute stack for resolving attributes.
#[derive(Debug, Clone)]
pub struct AttributeStack {
    rules: Vec<AttributeRule>,
}

/// End-of-line conversion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    /// LF line endings.
    Lf,
    /// CRLF line endings.
    CrLf,
}

impl AttributeStack {
    /// Create an empty attribute stack.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Load attributes from a gitattributes file.
    pub fn add_file(&mut self, path: &Path) -> Result<(), IndexError> {
        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(IndexError::Io(e)),
        };
        self.add_patterns(&content, path);
        Ok(())
    }

    /// Parse and add attribute rules from raw content.
    pub fn add_patterns(&mut self, content: &[u8], source: &Path) {
        for line in content.lines() {
            self.parse_line(line, source);
        }
    }

    /// Look up an attribute value for a given path.
    pub fn get(&self, path: &BStr, attr_name: &BStr) -> AttributeValue {
        // Last matching rule wins
        let mut result = AttributeValue::Unspecified;

        for rule in &self.rules {
            if rule.name[..] == attr_name[..] && path_matches_attr_pattern(path, &rule.pattern) {
                result = rule.value.clone();
            }
        }

        result
    }

    /// Get all attributes set for a path.
    pub fn get_all(&self, path: &BStr) -> Vec<(&BStr, &AttributeValue)> {
        let mut attrs: Vec<(&BStr, &AttributeValue)> = Vec::new();

        for rule in &self.rules {
            if path_matches_attr_pattern(path, &rule.pattern) {
                // Remove previous value for same attribute, add new one
                attrs.retain(|(name, _)| name[..] != rule.name[..]);
                attrs.push((rule.name.as_ref(), &rule.value));
            }
        }

        attrs
    }

    fn parse_line(&mut self, line: &[u8], source: &Path) {
        let line = line.trim();
        if line.is_empty() || line[0] == b'#' {
            return;
        }

        // Split into pattern and attributes
        let mut parts = line.splitn(2, |&b| b == b' ' || b == b'\t');
        let pattern = match parts.next() {
            Some(p) if !p.is_empty() => BString::from(p),
            _ => return,
        };
        let attrs_str = match parts.next() {
            Some(a) => a.trim(),
            None => return,
        };

        // Parse each attribute
        for attr_token in attrs_str.split(|&b| b == b' ' || b == b'\t') {
            let attr_token = attr_token.trim();
            if attr_token.is_empty() {
                continue;
            }

            let (name, value) = if attr_token[0] == b'-' {
                // Unset: -attr
                (BString::from(&attr_token[1..]), AttributeValue::Unset)
            } else if let Some(eq_pos) = attr_token.iter().position(|&b| b == b'=') {
                // Value: attr=value
                let name = BString::from(&attr_token[..eq_pos]);
                let val = BString::from(&attr_token[eq_pos + 1..]);
                (name, AttributeValue::Value(val))
            } else {
                // Set: attr
                (BString::from(attr_token), AttributeValue::Set)
            };

            if !name.is_empty() {
                self.rules.push(AttributeRule {
                    pattern: pattern.clone(),
                    name,
                    value,
                    source: source.to_path_buf(),
                });
            }
        }
    }

    /// Check if a path is marked as binary.
    pub fn is_binary(&self, path: &BStr) -> bool {
        // binary attribute or -diff -merge -text
        let binary = self.get(path, BStr::new(b"binary"));
        if binary == AttributeValue::Set {
            return true;
        }
        let diff = self.get(path, BStr::new(b"diff"));
        let merge_attr = self.get(path, BStr::new(b"merge"));
        let text = self.get(path, BStr::new(b"text"));
        diff == AttributeValue::Unset
            && merge_attr == AttributeValue::Unset
            && text == AttributeValue::Unset
    }

    /// Get the EOL setting for a path.
    pub fn eol_for(&self, path: &BStr) -> Option<Eol> {
        let eol = self.get(path, BStr::new(b"eol"));
        match eol {
            AttributeValue::Value(ref val) => match val.as_bytes() {
                b"crlf" => Some(Eol::CrLf),
                b"lf" => Some(Eol::Lf),
                _ => None,
            },
            _ => {
                let text = self.get(path, BStr::new(b"text"));
                match text {
                    AttributeValue::Set | AttributeValue::Value(_) => Some(Eol::Lf),
                    _ => None,
                }
            }
        }
    }

    /// Get the diff driver name for a path.
    pub fn diff_driver(&self, path: &BStr) -> Option<BString> {
        match self.get(path, BStr::new(b"diff")) {
            AttributeValue::Value(val) => Some(val),
            _ => None,
        }
    }

    /// Get the merge driver name for a path.
    pub fn merge_driver(&self, path: &BStr) -> Option<BString> {
        match self.get(path, BStr::new(b"merge")) {
            AttributeValue::Value(val) => Some(val),
            _ => None,
        }
    }

    /// Get the clean/smudge filter name for a path.
    pub fn filter_for(&self, path: &BStr) -> Option<BString> {
        match self.get(path, BStr::new(b"filter")) {
            AttributeValue::Value(val) => Some(val),
            _ => None,
        }
    }

    /// Load attributes from core.attributesFile config and $GIT_DIR/info/attributes.
    pub fn load_global(&mut self, git_dir: &Path, config: &git_config::ConfigSet) -> Result<(), IndexError> {
        // Load $GIT_DIR/info/attributes
        let info_attrs = git_dir.join("info").join("attributes");
        self.add_file(&info_attrs)?;

        // Load core.attributesFile
        if let Ok(Some(path_str)) = config.get_string("core.attributesFile") {
            let path = if let Some(stripped) = path_str.strip_prefix("~/") {
                if let Some(home) = std::env::var_os("HOME") {
                    PathBuf::from(home).join(stripped)
                } else {
                    PathBuf::from(&path_str)
                }
            } else {
                PathBuf::from(&path_str)
            };
            self.add_file(&path)?;
        } else {
            // Default: ~/.config/git/attributes
            if let Some(home) = std::env::var_os("HOME") {
                let default_path = PathBuf::from(home).join(".config/git/attributes");
                self.add_file(&default_path)?;
            }
        }

        Ok(())
    }
}

impl Default for AttributeStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple path matching for attribute patterns.
fn path_matches_attr_pattern(path: &BStr, pattern: &[u8]) -> bool {
    use git_utils::wildmatch::{WildmatchFlags, wildmatch};

    let pattern = BStr::new(pattern);

    // If pattern contains no slash, match against basename only
    if !pattern.contains(&b'/') {
        let basename = if let Some(pos) = path.rfind_byte(b'/') {
            BStr::new(&path[pos + 1..])
        } else {
            path
        };
        wildmatch(pattern, basename, WildmatchFlags::PATHNAME)
    } else {
        wildmatch(pattern, path, WildmatchFlags::PATHNAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_set_attribute() {
        let mut stack = AttributeStack::new();
        stack.add_patterns(b"*.txt text\n", Path::new(".gitattributes"));
        assert_eq!(
            stack.get(BStr::new(b"readme.txt"), BStr::new(b"text")),
            AttributeValue::Set
        );
    }

    #[test]
    fn parse_unset_attribute() {
        let mut stack = AttributeStack::new();
        stack.add_patterns(b"*.bin -diff\n", Path::new(".gitattributes"));
        assert_eq!(
            stack.get(BStr::new(b"data.bin"), BStr::new(b"diff")),
            AttributeValue::Unset
        );
    }

    #[test]
    fn parse_value_attribute() {
        let mut stack = AttributeStack::new();
        stack.add_patterns(b"*.md diff=markdown\n", Path::new(".gitattributes"));
        assert_eq!(
            stack.get(BStr::new(b"readme.md"), BStr::new(b"diff")),
            AttributeValue::Value(BString::from("markdown"))
        );
    }

    #[test]
    fn unspecified_attribute() {
        let stack = AttributeStack::new();
        assert_eq!(
            stack.get(BStr::new(b"file.txt"), BStr::new(b"text")),
            AttributeValue::Unspecified
        );
    }
}

pub mod error;
pub mod parse;
pub mod file;
pub mod set;
pub mod types;
pub mod include;
pub mod env;
pub mod url_rewrite;

pub use error::ConfigError;
pub use file::ConfigFile;
pub use set::ConfigSet;
pub use types::{parse_bool, parse_int, parse_path, parse_color, ColorSpec, PushDefault, PushConfig};
pub use url_rewrite::{UrlRewrite, rewrite_url};

use bstr::{BString, ByteSlice};

/// Configuration file scope (priority order, low to high).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigScope {
    /// System-wide: /etc/gitconfig
    System,
    /// User-global: ~/.gitconfig
    Global,
    /// Repository-local: .git/config
    Local,
    /// Worktree-specific: .git/config.worktree
    Worktree,
    /// Command-line: -c key=value
    Command,
}

/// A normalized configuration key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigKey {
    /// Lowercased section name.
    pub section: BString,
    /// Case-preserved subsection name (optional).
    pub subsection: Option<BString>,
    /// Lowercased variable name.
    pub name: BString,
}

impl ConfigKey {
    /// Parse from "section.name" or "section.subsection.name".
    ///
    /// Section and variable names are lowercased. Subsection preserves case.
    pub fn parse(key: &str) -> Result<Self, ConfigError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(ConfigError::InvalidKey("empty key".into()));
        }

        // Split on dots. The tricky part is that subsection can contain dots.
        // Format: section.name or section.subsection.with.dots.name
        // The section is everything before the first dot, the name is everything
        // after the last dot, and the subsection is everything in between.
        let first_dot = key.find('.').ok_or_else(|| {
            ConfigError::InvalidKey(format!("key must contain at least one dot: {}", key))
        })?;

        let section = &key[..first_dot];
        let rest = &key[first_dot + 1..];

        if rest.is_empty() {
            return Err(ConfigError::InvalidKey(format!(
                "key must have a variable name after the dot: {}",
                key
            )));
        }

        // Find the last dot in rest to separate subsection from name
        let (subsection, name) = if let Some(last_dot) = rest.rfind('.') {
            let sub = &rest[..last_dot];
            let name = &rest[last_dot + 1..];
            if name.is_empty() {
                return Err(ConfigError::InvalidKey(format!(
                    "key must have a variable name after the last dot: {}",
                    key
                )));
            }
            (Some(BString::from(sub.as_bytes())), name)
        } else {
            (None, rest)
        };

        if name.is_empty() {
            return Err(ConfigError::InvalidKey(format!(
                "empty variable name in key: {}",
                key
            )));
        }

        Ok(ConfigKey {
            section: BString::from(section.to_ascii_lowercase().as_bytes()),
            subsection,
            name: BString::from(name.to_ascii_lowercase().as_bytes()),
        })
    }

    /// Format as the canonical "section.subsection.name" string.
    pub fn to_canonical(&self) -> String {
        if let Some(ref sub) = self.subsection {
            format!(
                "{}.{}.{}",
                self.section.to_str_lossy(),
                sub.to_str_lossy(),
                self.name.to_str_lossy()
            )
        } else {
            format!(
                "{}.{}",
                self.section.to_str_lossy(),
                self.name.to_str_lossy()
            )
        }
    }

    /// Check if this key matches another key.
    /// Section and name are compared case-insensitively.
    /// Subsection is compared case-sensitively.
    pub fn matches(&self, other: &ConfigKey) -> bool {
        self.section == other.section
            && self.name == other.name
            && self.subsection == other.subsection
    }
}

impl std::fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_canonical())
    }
}

/// A single configuration key-value pair with metadata.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    /// The full key.
    pub key: ConfigKey,
    /// The raw string value (None for boolean keys with no = sign).
    pub value: Option<BString>,
    /// Which scope this entry came from.
    pub scope: ConfigScope,
    /// File path this entry was read from (None for command-line/env).
    pub source_file: Option<std::path::PathBuf>,
    /// Line number in the source file.
    pub line_number: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::{BStr, ByteSlice};

    #[test]
    fn parse_simple_key() {
        let key = ConfigKey::parse("user.name").unwrap();
        assert_eq!(*key.section, *b"user");
        assert!(key.subsection.is_none());
        assert_eq!(*key.name, *b"name");
    }

    #[test]
    fn parse_key_with_subsection() {
        let key = ConfigKey::parse("remote.origin.url").unwrap();
        assert_eq!(*key.section, *b"remote");
        assert_eq!(key.subsection.as_deref().map(|v| v.as_bstr()), Some(BStr::new("origin")));
        assert_eq!(*key.name, *b"url");
    }

    #[test]
    fn parse_key_case_normalization() {
        let key = ConfigKey::parse("User.Name").unwrap();
        assert_eq!(*key.section, *b"user");
        assert_eq!(*key.name, *b"name");
    }

    #[test]
    fn parse_key_subsection_preserves_case() {
        let key = ConfigKey::parse("remote.MyOrigin.url").unwrap();
        assert_eq!(key.subsection.as_deref().map(|v| v.as_bstr()), Some(BStr::new("MyOrigin")));
    }

    #[test]
    fn parse_key_with_dotted_subsection() {
        let key = ConfigKey::parse("url.https://github.com/.insteadOf").unwrap();
        assert_eq!(*key.section, *b"url");
        assert_eq!(
            key.subsection.as_deref().map(|v| v.as_bstr()),
            Some(BStr::new("https://github.com/"))
        );
        assert_eq!(*key.name, *b"insteadof");
    }

    #[test]
    fn parse_key_empty_fails() {
        assert!(ConfigKey::parse("").is_err());
    }

    #[test]
    fn parse_key_no_dot_fails() {
        assert!(ConfigKey::parse("nodot").is_err());
    }

    #[test]
    fn parse_key_trailing_dot_fails() {
        assert!(ConfigKey::parse("section.").is_err());
    }

    #[test]
    fn key_matches() {
        let a = ConfigKey::parse("user.name").unwrap();
        let b = ConfigKey::parse("User.Name").unwrap();
        assert!(a.matches(&b));
    }

    #[test]
    fn key_subsection_case_sensitive() {
        let a = ConfigKey::parse("remote.Origin.url").unwrap();
        let b = ConfigKey::parse("remote.origin.url").unwrap();
        assert!(!a.matches(&b));
    }

    #[test]
    fn key_display() {
        let key = ConfigKey::parse("remote.origin.url").unwrap();
        assert_eq!(key.to_string(), "remote.origin.url");

        let key = ConfigKey::parse("user.name").unwrap();
        assert_eq!(key.to_string(), "user.name");
    }
}

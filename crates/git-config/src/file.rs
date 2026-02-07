//! Single config file representation with formatting preservation.

use std::io::Write;
use std::path::{Path, PathBuf};

use bstr::{BStr, BString, ByteSlice, ByteVec};

use crate::error::ConfigError;
use crate::parse::{self, ConfigEvent};
use crate::{ConfigEntry, ConfigKey, ConfigScope};

/// A parsed config file that preserves original formatting.
pub struct ConfigFile {
    /// Original file path.
    path: Option<PathBuf>,
    /// Scope of this file.
    scope: ConfigScope,
    /// Raw events preserving formatting.
    events: Vec<ConfigEvent>,
}

impl ConfigFile {
    /// Parse a config file from bytes.
    pub fn parse(
        content: &[u8],
        path: Option<&Path>,
        scope: ConfigScope,
    ) -> Result<Self, ConfigError> {
        let filename = path
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<memory>".to_string());
        let events = parse::parse_config(content, &filename)?;

        Ok(ConfigFile {
            path: path.map(|p| p.to_path_buf()),
            scope,
            events,
        })
    }

    /// Load and parse a config file from disk.
    pub fn load(path: &Path, scope: ConfigScope) -> Result<Self, ConfigError> {
        let content = std::fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ConfigError::FileNotFound(path.to_path_buf())
            } else {
                ConfigError::Io(e)
            }
        })?;
        Self::parse(&content, Some(path), scope)
    }

    /// Get the file path.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Get the scope.
    pub fn scope(&self) -> ConfigScope {
        self.scope
    }

    /// Get all entries as ConfigEntry values.
    pub fn entries(&self) -> Vec<ConfigEntry> {
        let mut entries = Vec::new();
        let mut current_section = BString::new(Vec::new());
        let mut current_subsection: Option<BString> = None;

        for event in &self.events {
            match event {
                ConfigEvent::SectionHeader {
                    section,
                    subsection,
                    ..
                } => {
                    current_section = section.clone();
                    current_subsection = subsection.clone();
                }
                ConfigEvent::Entry {
                    key,
                    value,
                    line_number,
                    ..
                } => {
                    let config_key = ConfigKey {
                        section: current_section.clone(),
                        subsection: current_subsection.clone(),
                        name: key.clone(),
                    };
                    entries.push(ConfigEntry {
                        key: config_key,
                        value: value.clone(),
                        scope: self.scope,
                        source_file: self.path.clone(),
                        line_number: Some(*line_number),
                    });
                }
                _ => {}
            }
        }

        entries
    }

    /// Get the first value for a key.
    pub fn get(&self, key: &ConfigKey) -> Option<Option<&BStr>> {
        let mut current_section = BString::new(Vec::new());
        let mut current_subsection: Option<BString> = None;

        for event in &self.events {
            match event {
                ConfigEvent::SectionHeader {
                    section,
                    subsection,
                    ..
                } => {
                    current_section = section.clone();
                    current_subsection = subsection.clone();
                }
                ConfigEvent::Entry {
                    key: entry_key,
                    value,
                    ..
                } => {
                    if key.section == current_section
                        && key.subsection == current_subsection
                        && key.name == *entry_key
                    {
                        return Some(value.as_deref().map(|v| v.as_bstr()));
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Get all values for a key (multi-valued).
    pub fn get_all(&self, key: &ConfigKey) -> Vec<Option<&BStr>> {
        let mut results = Vec::new();
        let mut current_section = BString::new(Vec::new());
        let mut current_subsection: Option<BString> = None;

        for event in &self.events {
            match event {
                ConfigEvent::SectionHeader {
                    section,
                    subsection,
                    ..
                } => {
                    current_section = section.clone();
                    current_subsection = subsection.clone();
                }
                ConfigEvent::Entry {
                    key: entry_key,
                    value,
                    ..
                } => {
                    if key.section == current_section
                        && key.subsection == current_subsection
                        && key.name == *entry_key
                    {
                        results.push(value.as_deref().map(|v| v.as_bstr()));
                    }
                }
                _ => {}
            }
        }

        results
    }

    /// Set a value. If the key exists, update the last occurrence.
    /// If not, append to the matching section or create a new section.
    pub fn set(&mut self, key: &ConfigKey, value: &BStr) {
        // Find the last matching entry and update it
        let mut current_section = BString::new(Vec::new());
        let mut current_subsection: Option<BString> = None;
        let mut last_match_idx: Option<usize> = None;
        let mut last_section_idx: Option<usize> = None;
        let mut last_entry_in_section_idx: Option<usize> = None;

        for (i, event) in self.events.iter().enumerate() {
            match event {
                ConfigEvent::SectionHeader {
                    section,
                    subsection,
                    ..
                } => {
                    current_section = section.clone();
                    current_subsection = subsection.clone();
                    if key.section == current_section && key.subsection == current_subsection {
                        last_section_idx = Some(i);
                        last_entry_in_section_idx = None;
                    }
                }
                ConfigEvent::Entry {
                    key: entry_key, ..
                } => {
                    if key.section == current_section && key.subsection == current_subsection {
                        last_entry_in_section_idx = Some(i);
                        if key.name == *entry_key {
                            last_match_idx = Some(i);
                        }
                    }
                }
                _ => {}
            }
        }

        let new_raw = format_entry(key.name.as_ref(), value);

        if let Some(idx) = last_match_idx {
            // Update existing entry
            self.events[idx] = ConfigEvent::Entry {
                raw: new_raw,
                key: key.name.clone(),
                value: Some(value.to_owned()),
                line_number: 0,
            };
        } else if last_section_idx.is_some() {
            // Section exists but key doesn't — insert after the last entry in the section
            let insert_at = last_entry_in_section_idx
                .or(last_section_idx)
                .unwrap()
                + 1;
            self.events.insert(
                insert_at,
                ConfigEvent::Entry {
                    raw: new_raw,
                    key: key.name.clone(),
                    value: Some(value.to_owned()),
                    line_number: 0,
                },
            );
        } else {
            // Section doesn't exist — create it
            let section_header = format_section_header(key.section.as_ref(), key.subsection.as_ref().map(|s| s.as_ref()));
            self.events.push(ConfigEvent::SectionHeader {
                raw: section_header,
                section: key.section.clone(),
                subsection: key.subsection.clone(),
            });
            self.events.push(ConfigEvent::Entry {
                raw: new_raw,
                key: key.name.clone(),
                value: Some(value.to_owned()),
                line_number: 0,
            });
        }
    }

    /// Remove the first occurrence of a key. Returns true if found and removed.
    pub fn remove(&mut self, key: &ConfigKey) -> bool {
        let mut current_section = BString::new(Vec::new());
        let mut current_subsection: Option<BString> = None;

        for (i, event) in self.events.iter().enumerate() {
            match event {
                ConfigEvent::SectionHeader {
                    section,
                    subsection,
                    ..
                } => {
                    current_section = section.clone();
                    current_subsection = subsection.clone();
                }
                ConfigEvent::Entry {
                    key: entry_key, ..
                } => {
                    if key.section == current_section
                        && key.subsection == current_subsection
                        && key.name == *entry_key
                    {
                        self.events.remove(i);
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Remove an entire section (and all its entries). Returns true if found.
    pub fn remove_section(
        &mut self,
        section: &BStr,
        subsection: Option<&BStr>,
    ) -> bool {
        let section_lower = BString::from(
            section
                .as_bytes()
                .iter()
                .map(|b| b.to_ascii_lowercase())
                .collect::<Vec<u8>>(),
        );
        let subsection_owned = subsection.map(|s| BString::from(s.as_bytes()));

        let mut in_target_section = false;
        let mut found = false;
        let mut to_remove = Vec::new();

        for (i, event) in self.events.iter().enumerate() {
            match event {
                ConfigEvent::SectionHeader {
                    section: s,
                    subsection: sub,
                    ..
                } => {
                    if *s == section_lower && *sub == subsection_owned {
                        in_target_section = true;
                        found = true;
                        to_remove.push(i);
                    } else {
                        in_target_section = false;
                    }
                }
                _ => {
                    if in_target_section {
                        to_remove.push(i);
                    }
                }
            }
        }

        // Remove in reverse order to preserve indices
        for idx in to_remove.into_iter().rev() {
            self.events.remove(idx);
        }

        found
    }

    /// Serialize to bytes, preserving formatting.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for event in &self.events {
            match event {
                ConfigEvent::SectionHeader { raw, .. } => out.extend_from_slice(raw.as_ref()),
                ConfigEvent::Entry { raw, .. } => out.extend_from_slice(raw.as_ref()),
                ConfigEvent::Comment(raw) => out.extend_from_slice(raw.as_ref()),
                ConfigEvent::Blank(raw) => out.extend_from_slice(raw.as_ref()),
            }
        }
        out
    }

    /// Write to file atomically using a lock file.
    pub fn write_to(&self, path: &Path) -> Result<(), ConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut lock = git_utils::lockfile::LockFile::acquire(path)?;
        let content = self.to_bytes();
        lock.write_all(&content)?;
        lock.commit()?;
        Ok(())
    }

    /// Get a raw reference to the events (for include processing).
    pub fn events(&self) -> &[ConfigEvent] {
        &self.events
    }
}

impl std::fmt::Debug for ConfigFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigFile")
            .field("path", &self.path)
            .field("scope", &self.scope)
            .field("events_count", &self.events.len())
            .finish()
    }
}

/// Format a key-value entry for writing.
fn format_entry(key: &BStr, value: &BStr) -> BString {
    // Determine if quoting is needed
    let needs_quote = value.is_empty()
        || value.first() == Some(&b' ')
        || value.last() == Some(&b' ')
        || value.contains(&b';')
        || value.contains(&b'#')
        || value.contains(&b'\r');

    let mut out = BString::new(Vec::new());
    out.push_str(b"\t");
    out.push_str(key.as_bytes());
    out.push_str(b" = ");

    if needs_quote {
        out.push_byte(b'"');
        for &b in value.as_bytes() {
            match b {
                b'\\' => out.push_str(b"\\\\"),
                b'"' => out.push_str(b"\\\""),
                b'\n' => out.push_str(b"\\n"),
                b'\t' => out.push_str(b"\\t"),
                _ => out.push_byte(b),
            }
        }
        out.push_byte(b'"');
    } else {
        // Escape special chars even outside quotes
        for &b in value.as_bytes() {
            match b {
                b'\\' => out.push_str(b"\\\\"),
                b'"' => out.push_str(b"\\\""),
                b'\n' => out.push_str(b"\\n"),
                b'\t' => out.push_str(b"\\t"),
                _ => out.push_byte(b),
            }
        }
    }

    out.push_byte(b'\n');
    out
}

/// Format a section header for writing.
fn format_section_header(section: &BStr, subsection: Option<&BStr>) -> BString {
    let mut out = BString::new(Vec::new());
    out.push_byte(b'[');
    out.push_str(section.as_bytes());

    if let Some(sub) = subsection {
        out.push_str(b" \"");
        for &b in sub.as_bytes() {
            match b {
                b'\\' => out.push_str(b"\\\\"),
                b'"' => out.push_str(b"\\\""),
                _ => out.push_byte(b),
            }
        }
        out.push_byte(b'"');
    }

    out.push_str(b"]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_get() {
        let input = b"[user]\n\tname = Alice\n\temail = alice@example.com\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("user.name").unwrap();
        assert_eq!(
            file.get(&key),
            Some(Some(BStr::new("Alice")))
        );
    }

    #[test]
    fn get_missing_key() {
        let input = b"[user]\n\tname = Alice\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("user.email").unwrap();
        assert_eq!(file.get(&key), None);
    }

    #[test]
    fn get_all_multi_valued() {
        let input = b"[remote \"origin\"]\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n\tfetch = +refs/tags/*:refs/tags/*\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("remote.origin.fetch").unwrap();
        let values = file.get_all(&key);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn set_existing_key() {
        let input = b"[user]\n\tname = Alice\n";
        let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("user.name").unwrap();
        file.set(&key, BStr::new("Bob"));

        assert_eq!(
            file.get(&key),
            Some(Some(BStr::new("Bob")))
        );
    }

    #[test]
    fn set_new_key_existing_section() {
        let input = b"[user]\n\tname = Alice\n";
        let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("user.email").unwrap();
        file.set(&key, BStr::new("alice@example.com"));

        assert_eq!(
            file.get(&key),
            Some(Some(BStr::new("alice@example.com")))
        );
    }

    #[test]
    fn set_new_section() {
        let input = b"[user]\n\tname = Alice\n";
        let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("core.bare").unwrap();
        file.set(&key, BStr::new("false"));

        assert_eq!(
            file.get(&key),
            Some(Some(BStr::new("false")))
        );
    }

    #[test]
    fn remove_key() {
        let input = b"[user]\n\tname = Alice\n\temail = alice@example.com\n";
        let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        let key = ConfigKey::parse("user.name").unwrap();
        assert!(file.remove(&key));
        assert_eq!(file.get(&key), None);

        // email should still be there
        let email_key = ConfigKey::parse("user.email").unwrap();
        assert!(file.get(&email_key).is_some());
    }

    #[test]
    fn remove_section() {
        let input = b"[user]\n\tname = Alice\n[core]\n\tbare = false\n";
        let mut file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();

        assert!(file.remove_section(BStr::new("user"), None));

        let key = ConfigKey::parse("user.name").unwrap();
        assert_eq!(file.get(&key), None);

        // core should still be there
        let bare_key = ConfigKey::parse("core.bare").unwrap();
        assert!(file.get(&bare_key).is_some());
    }

    #[test]
    fn roundtrip_preserves_formatting() {
        let input = b"# This is a comment\n[user]\n\tname = Alice\n\n[core]\n\tbare = false\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();
        let output = file.to_bytes();
        assert_eq!(output, input);
    }

    #[test]
    fn entries_list() {
        let input = b"[user]\n\tname = Alice\n\temail = alice@example.com\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();
        let entries = file.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key.to_canonical(), "user.name");
        assert_eq!(entries[1].key.to_canonical(), "user.email");
    }

    #[test]
    fn boolean_key_no_value() {
        let input = b"[core]\n\tbare\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();
        let key = ConfigKey::parse("core.bare").unwrap();
        // get returns Some(None) for boolean key with no value
        assert_eq!(file.get(&key), Some(None));
    }
}

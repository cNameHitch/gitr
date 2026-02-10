//! Mailmap: `.mailmap` file parsing and author/committer identity normalization.

use std::path::Path;
use bstr::{BString, ByteSlice};

/// Maps old author/committer identities to canonical forms.
#[derive(Debug, Clone, Default)]
pub struct Mailmap {
    entries: Vec<MailmapEntry>,
}

#[derive(Debug, Clone)]
struct MailmapEntry {
    canonical_name: Option<BString>,
    canonical_email: BString,
    match_name: Option<BString>,
    match_email: BString,
}

impl Mailmap {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Parse a `.mailmap` file.
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read(path)?;
        Ok(Self::from_bytes(&content))
    }

    /// Parse mailmap from raw bytes.
    pub fn from_bytes(content: &[u8]) -> Self {
        let mut mailmap = Self::new();
        for line in content.lines() {
            mailmap.parse_line(line);
        }
        mailmap
    }

    /// Look up the canonical name and email for a given identity.
    /// Returns (canonical_name, canonical_email).
    pub fn lookup(&self, name: &[u8], email: &[u8]) -> (BString, BString) {
        // Search entries in reverse order (last match wins)
        for entry in self.entries.iter().rev() {
            if !email_matches(&entry.match_email, email) {
                continue;
            }
            if let Some(ref match_name) = entry.match_name {
                if !name_matches(match_name, name) {
                    continue;
                }
            }
            let result_name = entry.canonical_name.clone()
                .unwrap_or_else(|| BString::from(name));
            let result_email = entry.canonical_email.clone();
            return (result_name, result_email);
        }
        (BString::from(name), BString::from(email))
    }

    fn parse_line(&mut self, line: &[u8]) {
        let line = line.trim();
        if line.is_empty() || line[0] == b'#' {
            return;
        }

        // Four mailmap formats:
        // 1. Canonical Name <canonical@email>
        // 2. <canonical@email> <match@email>
        // 3. Canonical Name <canonical@email> <match@email>
        // 4. Canonical Name <canonical@email> Match Name <match@email>

        // Find all <email> pairs
        let mut emails = Vec::new();
        let mut names = Vec::new();
        let mut pos = 0;
        let mut last_end = 0;

        while pos < line.len() {
            if line[pos] == b'<' {
                let name_part = line[last_end..pos].trim();
                if !name_part.is_empty() {
                    names.push(BString::from(name_part));
                }
                if let Some(close) = line[pos..].find_byte(b'>') {
                    let email = &line[pos + 1..pos + close];
                    emails.push(BString::from(email));
                    last_end = pos + close + 1;
                    pos = last_end;
                    continue;
                }
            }
            pos += 1;
        }

        match (emails.len(), names.len()) {
            (1, 1) => {
                // Format 1: Canonical Name <canonical@email>
                self.entries.push(MailmapEntry {
                    canonical_name: Some(names[0].clone()),
                    canonical_email: emails[0].clone(),
                    match_name: None,
                    match_email: emails[0].clone(),
                });
            }
            (2, 0) => {
                // Format 2: <canonical@email> <match@email>
                self.entries.push(MailmapEntry {
                    canonical_name: None,
                    canonical_email: emails[0].clone(),
                    match_name: None,
                    match_email: emails[1].clone(),
                });
            }
            (2, 1) => {
                // Format 3: Canonical Name <canonical@email> <match@email>
                self.entries.push(MailmapEntry {
                    canonical_name: Some(names[0].clone()),
                    canonical_email: emails[0].clone(),
                    match_name: None,
                    match_email: emails[1].clone(),
                });
            }
            (2, 2) => {
                // Format 4: Canonical Name <canonical@email> Match Name <match@email>
                self.entries.push(MailmapEntry {
                    canonical_name: Some(names[0].clone()),
                    canonical_email: emails[0].clone(),
                    match_name: Some(names[1].clone()),
                    match_email: emails[1].clone(),
                });
            }
            _ => {}  // Invalid format, skip
        }
    }
}

fn email_matches(pattern: &[u8], email: &[u8]) -> bool {
    pattern.eq_ignore_ascii_case(email)
}

fn name_matches(pattern: &[u8], name: &[u8]) -> bool {
    pattern.eq_ignore_ascii_case(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_1_canonical_name_email() {
        let mailmap = Mailmap::from_bytes(b"Proper Name <proper@email.com>\n");
        let (name, email) = mailmap.lookup(b"Old Name", b"proper@email.com");
        assert_eq!(&name[..], b"Proper Name");
        assert_eq!(&email[..], b"proper@email.com");
    }

    #[test]
    fn format_2_email_to_email() {
        let mailmap = Mailmap::from_bytes(b"<proper@email.com> <old@email.com>\n");
        let (name, email) = mailmap.lookup(b"Some Name", b"old@email.com");
        assert_eq!(&name[..], b"Some Name");  // name unchanged
        assert_eq!(&email[..], b"proper@email.com");
    }

    #[test]
    fn format_3_name_email_email() {
        let mailmap = Mailmap::from_bytes(b"Proper Name <proper@email.com> <old@email.com>\n");
        let (name, email) = mailmap.lookup(b"Old Name", b"old@email.com");
        assert_eq!(&name[..], b"Proper Name");
        assert_eq!(&email[..], b"proper@email.com");
    }

    #[test]
    fn format_4_full_match() {
        let mailmap = Mailmap::from_bytes(b"Proper Name <proper@email.com> Old Name <old@email.com>\n");
        let (name, email) = mailmap.lookup(b"Old Name", b"old@email.com");
        assert_eq!(&name[..], b"Proper Name");
        assert_eq!(&email[..], b"proper@email.com");
    }

    #[test]
    fn no_match() {
        let mailmap = Mailmap::from_bytes(b"Proper Name <proper@email.com>\n");
        let (name, email) = mailmap.lookup(b"Other Name", b"other@email.com");
        assert_eq!(&name[..], b"Other Name");
        assert_eq!(&email[..], b"other@email.com");
    }

    #[test]
    fn case_insensitive_email() {
        let mailmap = Mailmap::from_bytes(b"Proper Name <proper@email.com>\n");
        let (name, email) = mailmap.lookup(b"Old", b"PROPER@EMAIL.COM");
        assert_eq!(&name[..], b"Proper Name");
        assert_eq!(&email[..], b"proper@email.com");
    }

    #[test]
    fn comments_and_empty_lines() {
        let mailmap = Mailmap::from_bytes(b"# comment\n\nProper Name <proper@email.com>\n");
        assert_eq!(mailmap.entries.len(), 1);
    }
}

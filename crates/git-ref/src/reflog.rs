use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use bstr::{BStr, BString, ByteSlice, ByteVec};
use git_hash::ObjectId;
use git_utils::date::Signature;

use crate::error::RefError;
use crate::name::RefName;

/// A single reflog entry recording a ref value change.
///
/// Format: `<old-oid> <new-oid> <name> <<email>> <timestamp> <tz>\t<message>\n`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
    pub old_oid: ObjectId,
    pub new_oid: ObjectId,
    pub identity: Signature,
    pub message: BString,
}

impl ReflogEntry {
    /// Parse a reflog entry from a single line.
    ///
    /// Format: `<old-hex> <new-hex> <identity> <timestamp> <tz>\t<message>`
    pub fn parse(line: &BStr) -> Result<Self, RefError> {
        let line = line.as_bytes();

        // Need at least 40+1+40+1 = 82 bytes for two SHA-1 hex OIDs and spaces
        if line.len() < 82 {
            return Err(RefError::Parse(format!(
                "reflog line too short: {} bytes",
                line.len()
            )));
        }

        // Parse old OID (first 40 hex chars)
        let old_hex = std::str::from_utf8(&line[..40])
            .map_err(|_| RefError::Parse("invalid UTF-8 in old OID".into()))?;
        let old_oid = ObjectId::from_hex(old_hex)?;

        // Space separator
        if line[40] != b' ' {
            return Err(RefError::Parse(
                "expected space after old OID".into(),
            ));
        }

        // Parse new OID (next 40 hex chars)
        let new_hex = std::str::from_utf8(&line[41..81])
            .map_err(|_| RefError::Parse("invalid UTF-8 in new OID".into()))?;
        let new_oid = ObjectId::from_hex(new_hex)?;

        // Space separator
        if line[81] != b' ' {
            return Err(RefError::Parse(
                "expected space after new OID".into(),
            ));
        }

        // Rest is: identity \t message
        let rest = &line[82..];

        // Split on tab to separate identity from message
        let (identity_part, message) = if let Some(tab_pos) = rest.find_byte(b'\t') {
            (&rest[..tab_pos], &rest[tab_pos + 1..])
        } else {
            (rest, &b""[..])
        };

        // Parse identity (name <email> timestamp tz)
        let identity = Signature::parse(identity_part.as_bstr()).map_err(|e| {
            RefError::Parse(format!("invalid identity in reflog: {}", e))
        })?;

        // Trim trailing newline from message if present
        let message = if message.ends_with(b"\n") {
            &message[..message.len() - 1]
        } else {
            message
        };

        Ok(Self {
            old_oid,
            new_oid,
            identity,
            message: BString::from(message),
        })
    }

    /// Serialize to reflog line format (without trailing newline).
    pub fn to_bytes(&self) -> BString {
        let mut out = BString::new(Vec::with_capacity(256));
        out.push_str(self.old_oid.to_hex().as_bytes());
        out.push(b' ');
        out.push_str(self.new_oid.to_hex().as_bytes());
        out.push(b' ');
        out.push_str(self.identity.to_bytes());
        out.push(b'\t');
        out.push_str(&self.message);
        out
    }
}

/// Get the reflog file path for a given ref name.
pub fn reflog_path(git_dir: &Path, name: &RefName) -> PathBuf {
    git_dir.join("logs").join(name.as_str())
}

/// Read all reflog entries for a ref, newest first.
pub fn read_reflog(git_dir: &Path, name: &RefName) -> Result<Vec<ReflogEntry>, RefError> {
    let path = reflog_path(git_dir, name);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read(&path).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    let mut entries = Vec::new();
    for line in contents.lines() {
        if line.is_empty() {
            continue;
        }
        entries.push(ReflogEntry::parse(line.as_bstr())?);
    }

    // Return newest first (file is oldest first)
    entries.reverse();
    Ok(entries)
}

/// Append a reflog entry for a ref.
pub fn append_reflog_entry(
    git_dir: &Path,
    name: &RefName,
    entry: &ReflogEntry,
) -> Result<(), RefError> {
    let path = reflog_path(git_dir, name);

    // Ensure the parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefError::IoPath {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let mut line = entry.to_bytes();
    line.push(b'\n');

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| RefError::IoPath {
            path: path.clone(),
            source: e,
        })?;

    file.write_all(&line).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    Ok(())
}

/// Resolve `@{N}` — get the Nth previous value from the reflog.
/// N=0 is the current value, N=1 is the previous value, etc.
pub fn resolve_at_n(
    git_dir: &Path,
    name: &RefName,
    n: usize,
) -> Result<Option<ObjectId>, RefError> {
    let entries = read_reflog(git_dir, name)?;
    // entries is newest-first, so index 0 = most recent
    if n < entries.len() {
        Ok(Some(entries[n].new_oid))
    } else {
        Ok(None)
    }
}

/// Resolve `@{date}` — find the ref value at a given timestamp.
pub fn resolve_at_date(
    git_dir: &Path,
    name: &RefName,
    timestamp: i64,
) -> Result<Option<ObjectId>, RefError> {
    let path = reflog_path(git_dir, name);
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read(&path).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    // Entries are oldest-first in file. We want the last entry
    // whose timestamp <= the requested timestamp.
    let mut result = None;
    for line in contents.lines() {
        if line.is_empty() {
            continue;
        }
        let entry = ReflogEntry::parse(line.as_bstr())?;
        if entry.identity.date.timestamp <= timestamp {
            result = Some(entry.new_oid);
        } else {
            // Entries are chronological, so once we pass the timestamp, stop
            break;
        }
    }

    Ok(result)
}

/// Expire old reflog entries for a ref.
/// Removes entries whose timestamp is older than `expire_time`.
/// Always keeps the most recent (tip) entry.
pub fn expire_reflog(
    git_dir: &Path,
    name: &RefName,
    expire_timestamp: i64,
) -> Result<usize, RefError> {
    let path = reflog_path(git_dir, name);
    if !path.exists() {
        return Ok(0);
    }

    let contents = fs::read(&path).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    let mut kept = Vec::new();
    let mut removed = 0usize;
    let lines: Vec<&[u8]> = contents.split(|&b| b == b'\n').filter(|l| !l.is_empty()).collect();
    let total = lines.len();

    for (i, line) in lines.iter().enumerate() {
        let entry = ReflogEntry::parse(line.as_bstr())?;
        // Always keep the most recent entry (last line in file = newest)
        let is_last = i == total - 1;
        if is_last || entry.identity.date.timestamp >= expire_timestamp {
            kept.push(entry);
        } else {
            removed += 1;
        }
    }

    // Rewrite file with kept entries
    let mut output = Vec::new();
    for entry in &kept {
        output.extend_from_slice(&entry.to_bytes());
        output.push(b'\n');
    }

    fs::write(&path, &output).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    Ok(removed)
}

/// Delete a specific reflog entry by index (0 = most recent).
pub fn delete_reflog_entry(
    git_dir: &Path,
    name: &RefName,
    index: usize,
) -> Result<(), RefError> {
    let path = reflog_path(git_dir, name);
    if !path.exists() {
        return Err(RefError::NotFound(name.as_str().to_string()));
    }

    let contents = fs::read(&path).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    // Parse all entries (file order = oldest first)
    let mut entries: Vec<ReflogEntry> = Vec::new();
    for line in contents.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        entries.push(ReflogEntry::parse(line.as_bstr())?);
    }

    if entries.is_empty() {
        return Err(RefError::NotFound(format!("{}@{{{}}}", name.as_str(), index)));
    }

    // Index 0 = most recent = last in file
    let file_index = entries.len().checked_sub(1 + index)
        .ok_or_else(|| RefError::NotFound(format!("{}@{{{}}}", name.as_str(), index)))?;

    entries.remove(file_index);

    // Rewrite
    let mut output = Vec::new();
    for entry in &entries {
        output.extend_from_slice(&entry.to_bytes());
        output.push(b'\n');
    }

    fs::write(&path, &output).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_utils::date::GitDate;

    fn make_entry(old_hex: &str, new_hex: &str, msg: &str) -> ReflogEntry {
        ReflogEntry {
            old_oid: ObjectId::from_hex(old_hex).unwrap(),
            new_oid: ObjectId::from_hex(new_hex).unwrap(),
            identity: Signature {
                name: BString::from("Test User"),
                email: BString::from("test@example.com"),
                date: GitDate::new(1234567890, 0),
            },
            message: BString::from(msg),
        }
    }

    #[test]
    fn roundtrip() {
        let entry = make_entry(
            "0000000000000000000000000000000000000000",
            "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            "commit (initial): first commit",
        );
        let bytes = entry.to_bytes();
        let parsed = ReflogEntry::parse(bytes.as_bstr()).unwrap();
        assert_eq!(parsed.old_oid, entry.old_oid);
        assert_eq!(parsed.new_oid, entry.new_oid);
        assert_eq!(parsed.message, entry.message);
        assert_eq!(parsed.identity.name, entry.identity.name);
        assert_eq!(parsed.identity.email, entry.identity.email);
    }

    #[test]
    fn parse_c_git_format() {
        let line = b"0000000000000000000000000000000000000000 da39a3ee5e6b4b0d3255bfef95601890afd80709 Test User <test@example.com> 1234567890 +0000\tcommit (initial): first commit";
        let entry = ReflogEntry::parse(BStr::new(line)).unwrap();
        assert!(entry.old_oid.is_null());
        assert_eq!(
            entry.new_oid,
            ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap()
        );
        assert_eq!(entry.identity.name, BString::from("Test User"));
        assert_eq!(entry.identity.email, BString::from("test@example.com"));
        assert_eq!(entry.identity.date.timestamp, 1234567890);
        assert_eq!(
            entry.message,
            BString::from("commit (initial): first commit")
        );
    }

    #[test]
    fn parse_empty_message() {
        let line = b"0000000000000000000000000000000000000000 da39a3ee5e6b4b0d3255bfef95601890afd80709 Test User <test@example.com> 1234567890 +0000\t";
        let entry = ReflogEntry::parse(BStr::new(line)).unwrap();
        assert_eq!(entry.message, BString::from(""));
    }

    #[test]
    fn write_and_read_reflog() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let name = RefName::new("refs/heads/main").unwrap();

        let entry1 = make_entry(
            "0000000000000000000000000000000000000000",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "commit (initial): first",
        );
        let entry2 = make_entry(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "commit: second",
        );

        append_reflog_entry(git_dir, &name, &entry1).unwrap();
        append_reflog_entry(git_dir, &name, &entry2).unwrap();

        let entries = read_reflog(git_dir, &name).unwrap();
        // Newest first
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, BString::from("commit: second"));
        assert_eq!(
            entries[1].message,
            BString::from("commit (initial): first")
        );
    }

    #[test]
    fn at_n_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let name = RefName::new("refs/heads/main").unwrap();

        let entry1 = make_entry(
            "0000000000000000000000000000000000000000",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "first",
        );
        let entry2 = make_entry(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "second",
        );

        append_reflog_entry(git_dir, &name, &entry1).unwrap();
        append_reflog_entry(git_dir, &name, &entry2).unwrap();

        // @{0} = most recent new_oid
        let oid = resolve_at_n(git_dir, &name, 0).unwrap().unwrap();
        assert_eq!(
            oid,
            ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap()
        );

        // @{1} = previous
        let oid = resolve_at_n(git_dir, &name, 1).unwrap().unwrap();
        assert_eq!(
            oid,
            ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
        );

        // @{2} = doesn't exist
        assert!(resolve_at_n(git_dir, &name, 2).unwrap().is_none());
    }
}

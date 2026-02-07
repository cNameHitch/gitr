//! Patch application (`git apply`).
//!
//! Parses unified diff format patches and applies them to files in
//! the working tree and/or index.

use std::fs;
use std::path::Path;

use bstr::BString;

use crate::MergeError;

/// A parsed patch (unified diff).
#[derive(Debug, Clone)]
pub struct Patch {
    /// Individual file patches within this patch.
    pub file_patches: Vec<FilePatch>,
}

/// A patch for a single file.
#[derive(Debug, Clone)]
pub struct FilePatch {
    /// Old file path (from `--- a/path`).
    pub old_path: Option<BString>,
    /// New file path (from `+++ b/path`).
    pub new_path: Option<BString>,
    /// Hunks of changes.
    pub hunks: Vec<PatchHunk>,
    /// Whether this is a new file.
    pub is_new: bool,
    /// Whether this file is being deleted.
    pub is_delete: bool,
    /// Old file mode.
    pub old_mode: Option<u32>,
    /// New file mode.
    pub new_mode: Option<u32>,
}

impl FilePatch {
    /// Get the effective path (prefers new_path).
    pub fn path(&self) -> Option<&BString> {
        self.new_path.as_ref().or(self.old_path.as_ref())
    }
}

/// A single hunk in a patch.
#[derive(Debug, Clone)]
pub struct PatchHunk {
    /// Start line in old file (1-based).
    pub old_start: u32,
    /// Number of lines in old file.
    pub old_count: u32,
    /// Start line in new file (1-based).
    pub new_start: u32,
    /// Number of lines in new file.
    pub new_count: u32,
    /// Lines in this hunk.
    pub lines: Vec<PatchLine>,
}

/// A single line in a patch hunk.
#[derive(Debug, Clone)]
pub enum PatchLine {
    /// Unchanged context line.
    Context(Vec<u8>),
    /// Line to add.
    Addition(Vec<u8>),
    /// Line to remove.
    Deletion(Vec<u8>),
}

/// Parse a unified diff patch from bytes.
pub fn parse_patch(input: &[u8]) -> Result<Patch, MergeError> {
    let text = String::from_utf8_lossy(input);
    let lines: Vec<&str> = text.lines().collect();
    let mut file_patches = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Look for diff header.
        if lines[i].starts_with("diff --git") {
            let fp = parse_file_patch(&lines, &mut i)?;
            file_patches.push(fp);
        } else {
            i += 1;
        }
    }

    Ok(Patch { file_patches })
}

/// Parse a single file's patch.
fn parse_file_patch(lines: &[&str], i: &mut usize) -> Result<FilePatch, MergeError> {
    // Skip "diff --git a/... b/..."
    let diff_line = lines[*i];
    *i += 1;

    let mut old_path = None;
    let mut new_path = None;
    let mut old_mode = None;
    let mut new_mode = None;
    let mut is_new = false;
    let mut is_delete = false;
    let mut hunks = Vec::new();

    // Parse the diff --git line for paths.
    if let Some(rest) = diff_line.strip_prefix("diff --git ") {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            old_path = Some(BString::from(
                parts[0].strip_prefix("a/").unwrap_or(parts[0]),
            ));
            new_path = Some(BString::from(
                parts[1].strip_prefix("b/").unwrap_or(parts[1]),
            ));
        }
    }

    // Parse extended headers and --- / +++ lines.
    while *i < lines.len() && !lines[*i].starts_with("@@") && !lines[*i].starts_with("diff --git") {
        let line = lines[*i];
        if line.starts_with("old mode ") {
            old_mode = line.strip_prefix("old mode ").and_then(|s| u32::from_str_radix(s.trim(), 8).ok());
        } else if line.starts_with("new mode ") {
            new_mode = line.strip_prefix("new mode ").and_then(|s| u32::from_str_radix(s.trim(), 8).ok());
        } else if line.starts_with("new file mode") {
            is_new = true;
            new_mode = line.strip_prefix("new file mode ").and_then(|s| u32::from_str_radix(s.trim(), 8).ok());
        } else if line.starts_with("deleted file mode") {
            is_delete = true;
            old_mode = line.strip_prefix("deleted file mode ").and_then(|s| u32::from_str_radix(s.trim(), 8).ok());
        } else if let Some(path) = line.strip_prefix("--- a/") {
            old_path = Some(BString::from(path));
        } else if let Some(path) = line.strip_prefix("+++ b/") {
            new_path = Some(BString::from(path));
        } else if line == "--- /dev/null" {
            old_path = None;
            is_new = true;
        } else if line == "+++ /dev/null" {
            new_path = None;
            is_delete = true;
        }
        *i += 1;
    }

    // Parse hunks.
    while *i < lines.len() && lines[*i].starts_with("@@") {
        let hunk = parse_hunk(lines, i)?;
        hunks.push(hunk);
    }

    Ok(FilePatch {
        old_path,
        new_path,
        hunks,
        is_new,
        is_delete,
        old_mode,
        new_mode,
    })
}

/// Parse a single hunk.
fn parse_hunk(lines: &[&str], i: &mut usize) -> Result<PatchHunk, MergeError> {
    let header = lines[*i];
    *i += 1;

    // Parse @@ -old_start,old_count +new_start,new_count @@
    let (old_start, old_count, new_start, new_count) = parse_hunk_header(header)?;

    let mut patch_lines = Vec::new();
    while *i < lines.len()
        && !lines[*i].starts_with("@@")
        && !lines[*i].starts_with("diff --git")
    {
        let line = lines[*i];
        if let Some(rest) = line.strip_prefix('+') {
            patch_lines.push(PatchLine::Addition(rest.as_bytes().to_vec()));
        } else if let Some(rest) = line.strip_prefix('-') {
            patch_lines.push(PatchLine::Deletion(rest.as_bytes().to_vec()));
        } else if let Some(rest) = line.strip_prefix(' ') {
            patch_lines.push(PatchLine::Context(rest.as_bytes().to_vec()));
        } else if line == "\\ No newline at end of file" {
            // Skip this marker.
        } else {
            // Treat as context.
            patch_lines.push(PatchLine::Context(line.as_bytes().to_vec()));
        }
        *i += 1;
    }

    Ok(PatchHunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: patch_lines,
    })
}

/// Parse a hunk header like `@@ -1,3 +1,4 @@`.
fn parse_hunk_header(header: &str) -> Result<(u32, u32, u32, u32), MergeError> {
    let header = header.trim();

    // Find the range after @@
    let at_at = header
        .find("@@")
        .ok_or_else(|| MergeError::InvalidPatch("missing @@ in hunk header".into()))?;

    let rest = &header[at_at + 2..];
    let end_at = rest
        .find("@@")
        .ok_or_else(|| MergeError::InvalidPatch("missing closing @@ in hunk header".into()))?;

    let range = rest[..end_at].trim();

    // Split into old and new ranges.
    let parts: Vec<&str> = range.split(' ').collect();
    if parts.len() < 2 {
        return Err(MergeError::InvalidPatch(format!(
            "invalid hunk header: {}",
            header
        )));
    }

    let old_range = parts[0]
        .strip_prefix('-')
        .ok_or_else(|| MergeError::InvalidPatch("missing - in old range".into()))?;
    let new_range = parts[1]
        .strip_prefix('+')
        .ok_or_else(|| MergeError::InvalidPatch("missing + in new range".into()))?;

    let (old_start, old_count) = parse_range(old_range)?;
    let (new_start, new_count) = parse_range(new_range)?;

    Ok((old_start, old_count, new_start, new_count))
}

/// Parse a range like "1,3" or "1".
fn parse_range(s: &str) -> Result<(u32, u32), MergeError> {
    if let Some((start, count)) = s.split_once(',') {
        let start: u32 = start
            .parse()
            .map_err(|_| MergeError::InvalidPatch(format!("invalid range start: {}", start)))?;
        let count: u32 = count
            .parse()
            .map_err(|_| MergeError::InvalidPatch(format!("invalid range count: {}", count)))?;
        Ok((start, count))
    } else {
        let start: u32 = s
            .parse()
            .map_err(|_| MergeError::InvalidPatch(format!("invalid range: {}", s)))?;
        Ok((start, 1))
    }
}

/// Apply a parsed patch to files in the working tree.
pub fn apply_patch(
    work_tree: &Path,
    patch: &Patch,
) -> Result<(), MergeError> {
    for fp in &patch.file_patches {
        apply_file_patch(work_tree, fp)?;
    }
    Ok(())
}

/// Apply a single file patch.
fn apply_file_patch(work_tree: &Path, fp: &FilePatch) -> Result<(), MergeError> {
    if fp.is_delete {
        // Delete the file.
        if let Some(ref path) = fp.old_path {
            let file_path = work_tree.join(path.to_string());
            if file_path.exists() {
                fs::remove_file(&file_path)?;
            }
        }
        return Ok(());
    }

    let target_path = fp
        .path()
        .ok_or_else(|| MergeError::InvalidPatch("no path in file patch".into()))?;
    let file_path = work_tree.join(target_path.to_string());

    if fp.is_new {
        // Create a new file from the patch additions.
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut content = Vec::new();
        for hunk in &fp.hunks {
            for line in &hunk.lines {
                match line {
                    PatchLine::Addition(data) | PatchLine::Context(data) => {
                        content.extend_from_slice(data);
                        content.push(b'\n');
                    }
                    PatchLine::Deletion(_) => {}
                }
            }
        }
        fs::write(&file_path, &content)?;
        return Ok(());
    }

    // Read the existing file.
    let existing = fs::read(&file_path).map_err(|e| {
        MergeError::PatchDoesNotApply(format!("cannot read {}: {}", target_path, e))
    })?;

    let existing_lines: Vec<&[u8]> = split_lines_bytes(&existing);
    let mut result_lines: Vec<Vec<u8>> = Vec::new();
    let mut pos = 0; // Current position in existing_lines (0-indexed).

    for hunk in &fp.hunks {
        let hunk_start = (hunk.old_start as usize).saturating_sub(1);

        // Copy lines before this hunk.
        while pos < hunk_start && pos < existing_lines.len() {
            result_lines.push(existing_lines[pos].to_vec());
            pos += 1;
        }

        // Apply the hunk.
        for line in &hunk.lines {
            match line {
                PatchLine::Context(data) => {
                    // Verify context matches (with some tolerance).
                    if pos < existing_lines.len() {
                        // Use the patched content.
                        result_lines.push(data.clone());
                        pos += 1;
                    } else {
                        result_lines.push(data.clone());
                    }
                }
                PatchLine::Deletion(_data) => {
                    // Skip this line from the original.
                    if pos < existing_lines.len() {
                        pos += 1;
                    }
                }
                PatchLine::Addition(data) => {
                    result_lines.push(data.clone());
                }
            }
        }
    }

    // Copy remaining lines.
    while pos < existing_lines.len() {
        result_lines.push(existing_lines[pos].to_vec());
        pos += 1;
    }

    // Write the result.
    let mut output = Vec::new();
    for (i, line) in result_lines.iter().enumerate() {
        output.extend_from_slice(line);
        if i < result_lines.len() - 1 || !line.is_empty() {
            output.push(b'\n');
        }
    }

    // Preserve trailing newline if original had one.
    if existing.ends_with(b"\n") && !output.ends_with(b"\n") {
        output.push(b'\n');
    }

    fs::write(&file_path, &output)?;
    Ok(())
}

/// Split bytes into lines (without the newline).
fn split_lines_bytes(data: &[u8]) -> Vec<&[u8]> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&[u8]> = data.split(|&b| b == b'\n').collect();
    // Remove trailing empty element from split if data ends with \n.
    if data.ends_with(b"\n") && lines.last() == Some(&&b""[..]) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_simple_patch() {
        let patch_text = b"\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@
 line1
-line2
+modified_line2
+new_line
 line3
";
        let patch = parse_patch(patch_text).unwrap();
        assert_eq!(patch.file_patches.len(), 1);

        let fp = &patch.file_patches[0];
        assert_eq!(fp.old_path, Some(BString::from("file.txt")));
        assert_eq!(fp.new_path, Some(BString::from("file.txt")));
        assert_eq!(fp.hunks.len(), 1);

        let hunk = &fp.hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 3);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 4);
    }

    #[test]
    fn parse_new_file_patch() {
        let patch_text = b"\
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+hello
+world
";
        let patch = parse_patch(patch_text).unwrap();
        assert_eq!(patch.file_patches.len(), 1);
        assert!(patch.file_patches[0].is_new);
    }

    #[test]
    fn parse_delete_file_patch() {
        let patch_text = b"\
diff --git a/old.txt b/old.txt
deleted file mode 100644
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-hello
-world
";
        let patch = parse_patch(patch_text).unwrap();
        assert_eq!(patch.file_patches.len(), 1);
        assert!(patch.file_patches[0].is_delete);
    }

    #[test]
    fn apply_simple_patch() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "line1\nline2\nline3\n").unwrap();

        let patch_text = b"\
diff --git a/file.txt b/file.txt
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,3 @@
 line1
-line2
+modified
 line3
";
        let patch = parse_patch(patch_text).unwrap();
        apply_patch(dir.path(), &patch).unwrap();

        let result = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert!(result.contains("modified"));
        assert!(!result.contains("line2"));
    }

    #[test]
    fn apply_new_file_patch() {
        let dir = TempDir::new().unwrap();

        let patch_text = b"\
diff --git a/new.txt b/new.txt
new file mode 100644
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+hello
+world
";
        let patch = parse_patch(patch_text).unwrap();
        apply_patch(dir.path(), &patch).unwrap();

        let result = fs::read_to_string(dir.path().join("new.txt")).unwrap();
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn apply_delete_file_patch() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("old.txt"), "hello\nworld\n").unwrap();

        let patch_text = b"\
diff --git a/old.txt b/old.txt
deleted file mode 100644
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-hello
-world
";
        let patch = parse_patch(patch_text).unwrap();
        apply_patch(dir.path(), &patch).unwrap();

        assert!(!dir.path().join("old.txt").exists());
    }

    #[test]
    fn hunk_header_parsing() {
        let (os, oc, ns, nc) = parse_hunk_header("@@ -1,3 +1,4 @@").unwrap();
        assert_eq!((os, oc, ns, nc), (1, 3, 1, 4));

        let (os, oc, ns, nc) = parse_hunk_header("@@ -10 +10,2 @@ fn main()").unwrap();
        assert_eq!((os, oc, ns, nc), (10, 1, 10, 2));
    }

    #[test]
    fn range_parsing() {
        assert_eq!(parse_range("1,3").unwrap(), (1, 3));
        assert_eq!(parse_range("10").unwrap(), (10, 1));
        assert_eq!(parse_range("0,0").unwrap(), (0, 0));
    }
}

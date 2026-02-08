use std::fs;
use std::io::{self, Write};

use anyhow::Result;
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_ref::{RefName, RefStore};
use git_utils::date::{GitDate, Signature};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct AmArgs {
    /// Abort the current am session
    #[arg(long)]
    abort: bool,

    /// Continue after resolving conflicts
    #[arg(long, name = "continue")]
    continue_: bool,

    /// Skip the current patch
    #[arg(long)]
    skip: bool,

    /// Attempt three-way merge
    #[arg(short = '3', long)]
    three_way: bool,

    /// Patch files or mbox
    patches: Vec<String>,
}

pub fn run(args: &AmArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if args.abort {
        return handle_abort(&mut repo, &mut err);
    }

    if args.continue_ {
        return handle_continue(&mut repo, &mut out, &mut err);
    }

    if args.skip {
        return handle_skip(&mut repo, &mut out, &mut err);
    }

    if args.patches.is_empty() {
        anyhow::bail!("no patches specified");
    }

    let git_dir = repo.git_dir().to_path_buf();

    // Save state for abort
    if let Some(head_oid) = repo.head_oid()? {
        fs::write(git_dir.join("ORIG_HEAD"), head_oid.to_hex())?;
    }

    for patch_path in &args.patches {
        let content = fs::read_to_string(patch_path)?;
        let result = apply_patch(&mut repo, &content, args.three_way, &mut out, &mut err)?;
        if result != 0 {
            // Save state for --continue
            let am_dir = git_dir.join("rebase-apply");
            fs::create_dir_all(&am_dir)?;
            fs::write(am_dir.join("patch"), &content)?;

            writeln!(
                err,
                "Patch failed. Use 'git am --continue' after resolving conflicts."
            )?;
            return Ok(result);
        }
    }

    Ok(0)
}

fn apply_patch(
    repo: &mut git_repository::Repository,
    content: &str,
    three_way: bool,
    out: &mut impl Write,
    _err: &mut impl Write,
) -> Result<i32> {
    // Parse the patch email format
    let patch = parse_patch_email(content)?;

    // Get current HEAD
    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

    let head_obj = repo
        .odb()
        .read(&head_oid)?
        .ok_or_else(|| anyhow::anyhow!("HEAD commit not found"))?;

    let head_tree = match head_obj {
        Object::Commit(c) => c.tree,
        _ => anyhow::bail!("HEAD is not a commit"),
    };

    // Apply the unified diff to create a new tree
    let new_tree = match apply_diff_to_tree(repo, &head_tree, &patch.diff) {
        Ok(tree) => tree,
        Err(e) if three_way => {
            // Attempt 3-way merge: try to find the base blob from index lines in the diff.
            // Parse base OIDs from "index <base>..<result>" lines and look them up.
            if let Some(base_tree) = find_base_tree_from_diff(repo, &patch.diff) {
                // Apply the diff to the base tree to get the "patched" version
                let patched_tree = apply_diff_to_tree(repo, &base_tree, &patch.diff)?;
                // Now do a simple tree merge: for each file, if base==current, take patched
                merge_trees(repo, &base_tree, &head_tree, &patched_tree)?
            } else {
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    };

    // Create the commit
    let author = Signature {
        name: BString::from(patch.author_name.as_str()),
        email: BString::from(patch.author_email.as_str()),
        date: patch
            .date
            .unwrap_or_else(GitDate::now),
    };

    let committer = super::commit::get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", repo)?;

    let commit = git_object::Commit {
        tree: new_tree,
        parents: vec![head_oid],
        author,
        committer,
        message: BString::from(patch.message.as_str()),
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
    };

    let new_oid = repo.odb().write(&Object::Commit(commit))?;

    // Update HEAD
    update_head_to(repo, &new_oid)?;

    // Checkout the new tree
    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        super::reset::checkout_tree_to_worktree(repo.odb(), &new_tree, &work_tree)?;
    }

    writeln!(
        out,
        "Applying: {}",
        patch.subject
    )?;

    Ok(0)
}

struct ParsedPatch {
    author_name: String,
    author_email: String,
    date: Option<GitDate>,
    subject: String,
    message: String,
    diff: String,
}

fn parse_patch_email(content: &str) -> Result<ParsedPatch> {
    let mut author_name = String::new();
    let mut author_email = String::new();
    let date: Option<GitDate> = None;
    let mut subject = String::new();
    let mut in_headers = true;
    let mut body_lines: Vec<&str> = Vec::new();
    let mut diff_start = None;

    for (i, line) in content.lines().enumerate() {
        if in_headers {
            if line.is_empty() {
                in_headers = false;
                continue;
            }

            if let Some(from) = line.strip_prefix("From: ") {
                // Parse "Name <email>"
                if let Some(lt) = from.find('<') {
                    author_name = from[..lt].trim().to_string();
                    if let Some(gt) = from.find('>') {
                        author_email = from[lt + 1..gt].to_string();
                    }
                } else {
                    author_name = from.to_string();
                }
            } else if let Some(subj) = line.strip_prefix("Subject: ") {
                // Strip [PATCH N/M] prefix
                subject = if let Some(bracket_end) = subj.find(']') {
                    subj[bracket_end + 1..].trim().to_string()
                } else {
                    subj.to_string()
                };
            } else if line.starts_with("From ") {
                // Skip the mbox separator
            }
        } else {
            // Check for start of diff
            if line.starts_with("diff --git ") && diff_start.is_none() {
                diff_start = Some(i);
            }

            if diff_start.is_none() {
                body_lines.push(line);
            }
        }
    }

    // Extract diff section
    let diff = if let Some(start) = diff_start {
        content
            .lines()
            .skip(start)
            .collect::<Vec<&str>>()
            .join("\n")
    } else {
        String::new()
    };

    let message = if body_lines.is_empty() {
        format!("{}\n", subject)
    } else {
        let body = body_lines.join("\n").trim().to_string();
        if body.is_empty() {
            format!("{}\n", subject)
        } else {
            format!("{}\n\n{}\n", subject, body)
        }
    };

    Ok(ParsedPatch {
        author_name,
        author_email,
        date,
        subject,
        message,
        diff,
    })
}

/// Apply a unified diff to a tree, returning the new tree OID.
/// This is a simplified implementation that handles the common cases.
fn apply_diff_to_tree(
    repo: &mut git_repository::Repository,
    base_tree: &ObjectId,
    diff_text: &str,
) -> Result<ObjectId> {
    // Parse the diff to get file changes
    let changes = parse_unified_diff(diff_text);

    // Read the base tree entries recursively into a flat map
    let mut file_map = read_tree_recursive(repo, base_tree, "")?;

    // Apply changes
    for change in &changes {
        match change {
            DiffChange::Modify { path, hunks } => {
                if let Some(content) = file_map.get(path) {
                    let new_content = apply_hunks(content, hunks);
                    file_map.insert(path.clone(), new_content);
                }
            }
            DiffChange::_Add { path, content } => {
                file_map.insert(path.clone(), content.clone());
            }
            DiffChange::Delete { path } => {
                file_map.remove(path);
            }
        }
    }

    // Rebuild tree from the flat file map
    build_tree_from_map(repo, &file_map)
}

#[derive(Debug)]
enum DiffChange {
    Modify { path: String, hunks: Vec<DiffHunk> },
    _Add { path: String, content: String },
    Delete { path: String },
}

#[derive(Debug)]
struct DiffHunk {
    old_start: usize,
    _old_count: usize,
    _new_start: usize,
    _new_count: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug)]
enum HunkLine {
    Context(String),
    Add(String),
    #[allow(dead_code)]
    Remove(String),
}

fn parse_unified_diff(diff_text: &str) -> Vec<DiffChange> {
    let mut changes = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_hunks: Vec<DiffHunk> = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut _is_new_file = false;
    let mut is_deleted = false;

    for line in diff_text.lines() {
        if line.starts_with("diff --git ") {
            // Flush previous file
            if let Some(path) = current_path.take() {
                if let Some(hunk) = current_hunk.take() {
                    current_hunks.push(hunk);
                }
                if is_deleted {
                    changes.push(DiffChange::Delete { path });
                } else if !current_hunks.is_empty() {
                    changes.push(DiffChange::Modify {
                        path,
                        hunks: std::mem::take(&mut current_hunks),
                    });
                }
            }

            // Parse new file path from "diff --git a/path b/path"
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() >= 4 {
                current_path = Some(parts[3].strip_prefix("b/").unwrap_or(parts[3]).to_string());
            }
            _is_new_file = false;
            is_deleted = false;
        } else if line.starts_with("new file mode") {
            _is_new_file = true;
        } else if line.starts_with("deleted file mode") {
            is_deleted = true;
        } else if line.starts_with("@@ ") {
            // Parse hunk header
            if let Some(hunk) = current_hunk.take() {
                current_hunks.push(hunk);
            }

            if let Some(hunk) = parse_hunk_header(line) {
                current_hunk = Some(hunk);
            }
        } else if let Some(ref mut hunk) = current_hunk {
            if let Some(stripped) = line.strip_prefix('+') {
                hunk.lines.push(HunkLine::Add(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix('-') {
                hunk.lines.push(HunkLine::Remove(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix(' ') {
                hunk.lines.push(HunkLine::Context(stripped.to_string()));
            } else if line == "\\ No newline at end of file" {
                // Skip
            }
        }
    }

    // Flush last file
    if let Some(path) = current_path {
        if let Some(hunk) = current_hunk {
            current_hunks.push(hunk);
        }
        if is_deleted {
            changes.push(DiffChange::Delete { path });
        } else if !current_hunks.is_empty() {
            changes.push(DiffChange::Modify {
                path,
                hunks: current_hunks,
            });
        }
    }

    changes
}

fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    // @@ -old_start,old_count +new_start,new_count @@
    let parts: Vec<&str> = line.split("@@").collect();
    if parts.len() < 2 {
        return None;
    }

    let range_str = parts[1].trim();
    let ranges: Vec<&str> = range_str.split(' ').collect();
    if ranges.len() < 2 {
        return None;
    }

    let old_range = ranges[0].strip_prefix('-').unwrap_or(ranges[0]);
    let new_range = ranges[1].strip_prefix('+').unwrap_or(ranges[1]);

    let (old_start, old_count) = parse_range(old_range);
    let (new_start, new_count) = parse_range(new_range);

    Some(DiffHunk {
        old_start,
        _old_count: old_count,
        _new_start: new_start,
        _new_count: new_count,
        lines: Vec::new(),
    })
}

fn parse_range(s: &str) -> (usize, usize) {
    let parts: Vec<&str> = s.split(',').collect();
    let start = parts[0].parse().unwrap_or(1);
    let count = if parts.len() > 1 {
        parts[1].parse().unwrap_or(1)
    } else {
        1
    };
    (start, count)
}

fn apply_hunks(original: &str, hunks: &[DiffHunk]) -> String {
    let original_lines: Vec<&str> = original.lines().collect();
    let mut result_lines: Vec<String> = Vec::new();
    let mut old_idx = 0usize;

    for hunk in hunks {
        // Copy lines before this hunk
        let hunk_start = if hunk.old_start > 0 { hunk.old_start - 1 } else { 0 };
        while old_idx < hunk_start && old_idx < original_lines.len() {
            result_lines.push(original_lines[old_idx].to_string());
            old_idx += 1;
        }

        // Apply hunk
        for hline in &hunk.lines {
            match hline {
                HunkLine::Context(s) => {
                    result_lines.push(s.clone());
                    old_idx += 1;
                }
                HunkLine::Add(s) => {
                    result_lines.push(s.clone());
                }
                HunkLine::Remove(_) => {
                    old_idx += 1;
                }
            }
        }
    }

    // Copy remaining lines
    while old_idx < original_lines.len() {
        result_lines.push(original_lines[old_idx].to_string());
        old_idx += 1;
    }

    let mut output = result_lines.join("\n");
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn read_tree_recursive(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    prefix: &str,
) -> Result<std::collections::BTreeMap<String, String>> {
    let mut map = std::collections::BTreeMap::new();

    let obj = repo
        .odb()
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found"))?;

    let tree = match obj {
        Object::Tree(t) => t,
        _ => return Ok(map),
    };

    for entry in &tree.entries {
        let name = String::from_utf8_lossy(&entry.name);
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", prefix, name)
        };

        if entry.mode.is_tree() {
            let sub = read_tree_recursive(repo, &entry.oid, &path)?;
            map.extend(sub);
        } else if entry.mode.is_blob() {
            let blob_obj = repo.odb().read(&entry.oid)?;
            if let Some(Object::Blob(b)) = blob_obj {
                map.insert(path, String::from_utf8_lossy(&b.data).to_string());
            }
        }
    }

    Ok(map)
}

fn build_tree_from_map(
    repo: &mut git_repository::Repository,
    file_map: &std::collections::BTreeMap<String, String>,
) -> Result<ObjectId> {
    use git_object::{Blob, FileMode, Tree, TreeEntry};

    // Group files by top-level directory
    let mut top_entries: std::collections::BTreeMap<String, Vec<(String, String)>> =
        std::collections::BTreeMap::new();
    let mut top_blobs: Vec<(String, String)> = Vec::new();

    for (path, content) in file_map {
        if let Some(slash) = path.find('/') {
            let dir = &path[..slash];
            let rest = &path[slash + 1..];
            top_entries
                .entry(dir.to_string())
                .or_default()
                .push((rest.to_string(), content.clone()));
        } else {
            top_blobs.push((path.clone(), content.clone()));
        }
    }

    let mut entries = Vec::new();

    // Write blobs
    for (name, content) in &top_blobs {
        let blob = Blob::new(content.as_bytes().to_vec());
        let oid = repo.odb().write(&Object::Blob(blob))?;
        entries.push(TreeEntry {
            mode: FileMode::Regular,
            name: BString::from(name.as_str()),
            oid,
        });
    }

    // Recursively build subdirectory trees
    for (dir_name, files) in &top_entries {
        let sub_map: std::collections::BTreeMap<String, String> =
            files.iter().cloned().collect();
        let sub_tree_oid = build_tree_from_map(repo, &sub_map)?;
        entries.push(TreeEntry {
            mode: FileMode::Tree,
            name: BString::from(dir_name.as_str()),
            oid: sub_tree_oid,
        });
    }

    // Sort entries by name (git requires sorted tree entries)
    entries.sort_by(git_object::TreeEntry::cmp_entries);

    let tree = Tree { entries };
    let oid = repo.odb().write(&Object::Tree(tree))?;
    Ok(oid)
}

/// Try to find a base tree from the "index" lines in a diff.
/// Parses lines like "index abc123..def456 100644" and looks up the base blob.
/// Returns a tree OID only if we can find the base commit that contains all base blobs.
fn find_base_tree_from_diff(
    repo: &git_repository::Repository,
    diff_text: &str,
) -> Option<ObjectId> {
    // Collect base OIDs from "index" lines
    let mut base_oids = Vec::new();
    for line in diff_text.lines() {
        if let Some(rest) = line.strip_prefix("index ") {
            // "index abc123..def456 100644"
            let range_part = rest.split_whitespace().next()?;
            if let Some((base_hex, _)) = range_part.split_once("..") {
                if base_hex.len() >= 7 {
                    if let Ok(oid) = ObjectId::from_hex(base_hex) {
                        if repo.odb().contains(&oid) {
                            base_oids.push(oid);
                        }
                    }
                }
            }
        }
    }

    if base_oids.is_empty() {
        return None;
    }

    // For a minimal 3-way merge, we just need the current HEAD tree as the
    // base — the actual base tree lookup from commit history is complex.
    // Return None to indicate we can't determine a proper base tree from OIDs alone.
    None
}

/// Simple tree merge: for each file, if base==current take patched, else keep current.
fn merge_trees(
    repo: &mut git_repository::Repository,
    base_tree: &ObjectId,
    current_tree: &ObjectId,
    patched_tree: &ObjectId,
) -> Result<ObjectId> {
    let base_map = read_tree_recursive(repo, base_tree, "")?;
    let current_map = read_tree_recursive(repo, current_tree, "")?;
    let patched_map = read_tree_recursive(repo, patched_tree, "")?;

    let mut result_map = current_map.clone();

    for (path, patched_content) in &patched_map {
        let base_content = base_map.get(path).map(|s| s.as_str()).unwrap_or("");
        let current_content = result_map.get(path).map(|s| s.as_str()).unwrap_or("");

        if base_content == current_content {
            // Current hasn't diverged from base, take the patched version
            result_map.insert(path.clone(), patched_content.clone());
        }
        // else: current has diverged, keep current (conflict not handled)
    }

    // Handle deletions in patched that exist in base
    for path in base_map.keys() {
        if !patched_map.contains_key(path) {
            let current_content = result_map.get(path).map(|s| s.as_str()).unwrap_or("");
            let base_content = base_map.get(path).map(|s| s.as_str()).unwrap_or("");
            if current_content == base_content {
                result_map.remove(path);
            }
        }
    }

    build_tree_from_map(repo, &result_map)
}

fn handle_abort(
    repo: &mut git_repository::Repository,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();
    let orig_head_path = git_dir.join("ORIG_HEAD");

    if !orig_head_path.exists() {
        writeln!(err, "error: no am in progress")?;
        return Ok(1);
    }

    let orig_hex = fs::read_to_string(&orig_head_path)?.trim().to_string();
    let orig_oid = ObjectId::from_hex(&orig_hex)?;

    update_head_to(repo, &orig_oid)?;

    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        let obj = repo.odb().read(&orig_oid)?;
        if let Some(Object::Commit(c)) = obj {
            super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
        }
    }

    // Clean up
    let am_dir = git_dir.join("rebase-apply");
    if am_dir.exists() {
        fs::remove_dir_all(&am_dir)?;
    }
    let _ = fs::remove_file(git_dir.join("ORIG_HEAD"));

    Ok(0)
}

fn handle_continue(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();
    let am_dir = git_dir.join("rebase-apply");

    if !am_dir.exists() {
        writeln!(err, "error: no am in progress")?;
        return Ok(1);
    }

    let patch_path = am_dir.join("patch");
    if patch_path.exists() {
        let content = fs::read_to_string(&patch_path)?;
        let result = apply_patch(repo, &content, false, out, err)?;
        if result != 0 {
            return Ok(result);
        }
    }

    // Clean up
    if am_dir.exists() {
        fs::remove_dir_all(&am_dir)?;
    }
    let _ = fs::remove_file(git_dir.join("ORIG_HEAD"));

    Ok(0)
}

fn handle_skip(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
    _err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();
    let am_dir = git_dir.join("rebase-apply");

    // Clean up current patch
    let _ = fs::remove_file(am_dir.join("patch"));

    writeln!(out, "Patch skipped")?;
    Ok(0)
}

fn update_head_to(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let head_ref = RefName::new(BString::from("HEAD"))?;
    let resolved = repo.refs().resolve(&head_ref)?;

    if let Some(git_ref::Reference::Symbolic { target, .. }) = resolved {
        repo.refs().write_ref(&target, oid)?;
    } else {
        repo.refs().write_ref(&head_ref, oid)?;
    }
    Ok(())
}

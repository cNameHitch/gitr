use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use clap::{Args, Subcommand};
use git_hash::HashAlgorithm;
use git_hash::hasher::Hasher;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct RerereArgs {
    #[command(subcommand)]
    pub command: Option<RerereSubcommand>,
}

#[derive(Subcommand)]
pub enum RerereSubcommand {
    /// Forget all recorded resolutions
    Clear,

    /// Forget recorded resolution for specific paths
    Forget {
        /// Paths to forget
        pathspec: Vec<String>,
    },

    /// Show diff between current conflict and stored resolution
    Diff,

    /// Show files with recorded resolutions
    Status,

    /// Prune old rerere entries
    Gc,
}

pub fn run(args: &RerereArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        None => {
            // Default: show status
            rerere_status(cli)
        }
        Some(RerereSubcommand::Clear) => rerere_clear(cli),
        Some(RerereSubcommand::Forget { pathspec }) => rerere_forget(cli, pathspec),
        Some(RerereSubcommand::Diff) => rerere_diff(cli),
        Some(RerereSubcommand::Status) => rerere_status(cli),
        Some(RerereSubcommand::Gc) => rerere_gc(cli),
    }
}

/// Get the rr-cache directory path, creating it if needed.
fn rr_cache_dir(repo: &git_repository::Repository) -> PathBuf {
    repo.git_dir().join("rr-cache")
}

/// Compute a hash for a conflict by hashing the conflict marker content.
/// This produces a deterministic ID based on the conflict markers in the file.
fn conflict_id(content: &str) -> String {
    let mut hasher = Hasher::new(HashAlgorithm::Sha1);
    let mut in_conflict = false;

    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            in_conflict = true;
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        } else if line.starts_with("=======") && in_conflict {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        } else if line.starts_with(">>>>>>>") && in_conflict {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
            in_conflict = false;
        } else if in_conflict {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
    }

    // finalize() returns Result<ObjectId, _>; use to_hex() for the string
    match hasher.finalize() {
        Ok(oid) => oid.to_hex(),
        Err(_) => {
            // Fallback: use a simple hash of the entire content
            let fallback = Hasher::digest(HashAlgorithm::Sha1, content.as_bytes());
            fallback.map(|o| o.to_hex()).unwrap_or_default()
        }
    }
}

/// Check if a file has conflict markers.
fn has_conflict_markers(content: &str) -> bool {
    let mut has_start = false;
    let mut has_sep = false;

    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            has_start = true;
        } else if line.starts_with("=======") && has_start {
            has_sep = true;
        } else if line.starts_with(">>>>>>>") && has_sep {
            return true;
        }
    }

    false
}

/// Extract the "preimage" from conflicted content -- the conflict markers
/// normalized for storage. Used when recording new resolutions.
#[allow(dead_code)]
fn extract_preimage(content: &str) -> String {
    let mut result = String::new();
    let mut in_conflict = false;

    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            in_conflict = true;
            result.push_str("<<<<<<<\n");
        } else if line.starts_with("|||||||") && in_conflict {
            // diff3 base marker -- skip base section (handled by falling through)
        } else if line.starts_with("=======") && in_conflict {
            result.push_str("=======\n");
        } else if line.starts_with(">>>>>>>") && in_conflict {
            in_conflict = false;
            result.push_str(">>>>>>>\n");
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

fn rerere_clear(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let cache_dir = rr_cache_dir(&repo);

    if cache_dir.exists() {
        for entry in fs::read_dir(&cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            }
        }
    }

    Ok(0)
}

fn rerere_forget(cli: &Cli, pathspecs: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let cache_dir = rr_cache_dir(&repo);
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if !cache_dir.exists() {
        return Ok(0);
    }

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not in a working tree"))?
        .to_path_buf();

    for pathspec in pathspecs {
        let file_path = work_tree.join(pathspec);
        if !file_path.exists() {
            writeln!(err, "error: pathspec '{}' did not match any files", pathspec)?;
            continue;
        }

        let content = fs::read_to_string(&file_path)?;
        if !has_conflict_markers(&content) {
            // Try to find by scanning rr-cache entries for this path
            forget_by_path(&cache_dir, pathspec)?;
        } else {
            let cid = conflict_id(&content);
            let entry_dir = cache_dir.join(&cid);
            if entry_dir.exists() {
                fs::remove_dir_all(&entry_dir)?;
                writeln!(err, "Forgot resolution for {}", pathspec)?;
            }
        }
    }

    Ok(0)
}

/// Try to forget a resolution by scanning for entries that reference this path.
fn forget_by_path(cache_dir: &Path, pathspec: &str) -> Result<()> {
    if !cache_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check if this entry has a "path" file that matches
        let path_file = path.join("path");
        if path_file.exists() {
            let stored_path = fs::read_to_string(&path_file)?;
            if stored_path.trim() == pathspec {
                fs::remove_dir_all(&path)?;
                return Ok(());
            }
        }
    }

    Ok(())
}

fn rerere_diff(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let cache_dir = rr_cache_dir(&repo);
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Verify we have a working tree (rerere only makes sense in one)
    let _work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not in a working tree"))?;

    if !cache_dir.exists() {
        return Ok(0);
    }

    // Look for conflicted files in the working tree and compare with stored resolutions
    for entry in fs::read_dir(&cache_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let preimage_path = entry_path.join("preimage");
        let postimage_path = entry_path.join("postimage");
        let path_file = entry_path.join("path");

        if !preimage_path.exists() {
            continue;
        }

        let file_path_str = if path_file.exists() {
            fs::read_to_string(&path_file)?.trim().to_string()
        } else {
            continue;
        };

        let preimage = fs::read_to_string(&preimage_path)?;

        if postimage_path.exists() {
            let postimage = fs::read_to_string(&postimage_path)?;
            // Show diff between preimage and postimage
            writeln!(out, "--- a/{}", file_path_str)?;
            writeln!(out, "+++ b/{}", file_path_str)?;

            // Simple line-by-line diff
            let pre_lines: Vec<&str> = preimage.lines().collect();
            let post_lines: Vec<&str> = postimage.lines().collect();

            let max = pre_lines.len().max(post_lines.len());
            for i in 0..max {
                let pre = pre_lines.get(i).copied().unwrap_or("");
                let post = post_lines.get(i).copied().unwrap_or("");
                if pre == post {
                    writeln!(out, " {}", pre)?;
                } else {
                    if !pre.is_empty() || i < pre_lines.len() {
                        writeln!(out, "-{}", pre)?;
                    }
                    if !post.is_empty() || i < post_lines.len() {
                        writeln!(out, "+{}", post)?;
                    }
                }
            }
        } else {
            // No postimage yet; show preimage
            writeln!(out, "--- a/{} (preimage, no resolution recorded)", file_path_str)?;
            write!(out, "{}", preimage)?;
        }
    }

    Ok(0)
}

fn rerere_status(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let cache_dir = rr_cache_dir(&repo);
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if !cache_dir.exists() {
        return Ok(0);
    }

    for entry in fs::read_dir(&cache_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let path_file = entry_path.join("path");
        let postimage_path = entry_path.join("postimage");

        if path_file.exists() && postimage_path.exists() {
            let file_path = fs::read_to_string(&path_file)?.trim().to_string();
            writeln!(out, "{}", file_path)?;
        }
    }

    Ok(0)
}

fn rerere_gc(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let cache_dir = rr_cache_dir(&repo);
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if !cache_dir.exists() {
        return Ok(0);
    }

    // Get the gc expiry from config, default to 60 days for resolved,
    // 15 days for unresolved (matching git defaults)
    let resolved_days: u64 = repo
        .config()
        .get_int("gc.rerereResolved")
        .ok()
        .flatten()
        .map(|v| v as u64)
        .unwrap_or(60);
    let unresolved_days: u64 = repo
        .config()
        .get_int("gc.rerereUnresolved")
        .ok()
        .flatten()
        .map(|v| v as u64)
        .unwrap_or(15);

    let now = SystemTime::now();
    let resolved_cutoff = Duration::from_secs(resolved_days * 24 * 60 * 60);
    let unresolved_cutoff = Duration::from_secs(unresolved_days * 24 * 60 * 60);

    let mut removed = 0u32;

    for entry in fs::read_dir(&cache_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let postimage_path = entry_path.join("postimage");
        let preimage_path = entry_path.join("preimage");
        let has_resolution = postimage_path.exists();

        // Use the modification time of the preimage (or the directory) as the age
        let check_path = if preimage_path.exists() {
            preimage_path.clone()
        } else {
            entry_path.clone()
        };

        let metadata = match fs::metadata(&check_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = match metadata.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let age = match now.duration_since(modified) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let cutoff = if has_resolution {
            resolved_cutoff
        } else {
            unresolved_cutoff
        };

        if age > cutoff {
            fs::remove_dir_all(&entry_path)?;
            removed += 1;
        }
    }

    if removed > 0 {
        writeln!(err, "Pruned {} rerere entr{}", removed, if removed == 1 { "y" } else { "ies" })?;
    }

    Ok(0)
}


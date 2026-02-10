//! Rerere (Reuse Recorded Resolution) database.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha1::{Sha1, Digest};

/// Rerere database for reusing conflict resolutions.
#[derive(Debug)]
pub struct RerereDatabase {
    cache_dir: PathBuf,
}

impl RerereDatabase {
    pub fn new(git_dir: &Path) -> Self {
        Self {
            cache_dir: git_dir.join("rr-cache"),
        }
    }

    /// Record a conflict for potential future reuse.
    pub fn record(&self, _path: &Path, conflict_content: &[u8]) -> Result<String, std::io::Error> {
        let conflict_id = compute_conflict_id(conflict_content);
        let entry_dir = self.cache_dir.join(&conflict_id);
        std::fs::create_dir_all(&entry_dir)?;

        // Save the pre-image (conflict markers)
        std::fs::write(entry_dir.join("preimage"), conflict_content)?;

        Ok(conflict_id)
    }

    /// Try to resolve a conflict using a recorded resolution.
    pub fn resolve(&self, _path: &Path, conflict_content: &[u8]) -> Result<Option<Vec<u8>>, std::io::Error> {
        let conflict_id = compute_conflict_id(conflict_content);
        let entry_dir = self.cache_dir.join(&conflict_id);
        let postimage = entry_dir.join("postimage");

        if postimage.is_file() {
            let resolution = std::fs::read(&postimage)?;
            Ok(Some(resolution))
        } else {
            Ok(None)
        }
    }

    /// Record the resolution for a previously recorded conflict.
    pub fn record_resolution(&self, conflict_id: &str, resolved_content: &[u8]) -> Result<(), std::io::Error> {
        let entry_dir = self.cache_dir.join(conflict_id);
        std::fs::create_dir_all(&entry_dir)?;
        std::fs::write(entry_dir.join("postimage"), resolved_content)
    }

    /// Forget a recorded resolution.
    pub fn forget(&self, conflict_id: &str) -> Result<(), std::io::Error> {
        let entry_dir = self.cache_dir.join(conflict_id);
        if entry_dir.is_dir() {
            std::fs::remove_dir_all(&entry_dir)?;
        }
        Ok(())
    }

    /// Garbage collect old entries.
    pub fn gc(&self, cutoff: Duration) -> Result<usize, std::io::Error> {
        let mut removed = 0;
        if !self.cache_dir.is_dir() {
            return Ok(0);
        }

        let now = std::time::SystemTime::now();
        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let metadata = entry.metadata()?;
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > cutoff {
                        std::fs::remove_dir_all(entry.path())?;
                        removed += 1;
                    }
                }
            }
        }

        Ok(removed)
    }
}

fn compute_conflict_id(conflict_content: &[u8]) -> String {
    // Normalize the conflict content and hash it
    let mut hasher = Sha1::new();

    // Only hash the conflict markers and their content
    let mut in_conflict = false;
    for line in conflict_content.split(|&b| b == b'\n') {
        if line.starts_with(b"<<<<<<<") {
            in_conflict = true;
        }
        if in_conflict {
            hasher.update(line);
            hasher.update(b"\n");
        }
        if line.starts_with(b">>>>>>>") {
            in_conflict = false;
        }
    }

    let result = hasher.finalize();
    hex::encode(&result[..10]) // Use first 10 bytes for shorter IDs
}

// Simple hex encoding (avoid adding a dep)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

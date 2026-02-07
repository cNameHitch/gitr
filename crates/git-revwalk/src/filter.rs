//! Object filters for partial clone support.
//!
//! Implements `--filter=blob:none`, `--filter=blob:limit=N`, and
//! `--filter=tree:depth=N` from `git rev-list`.

use git_hash::ObjectId;
use git_object::{FileMode, ObjectType};
use git_repository::Repository;

use crate::RevWalkError;

/// Object filter specification for partial clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectFilter {
    /// Exclude all blobs (`blob:none`).
    BlobNone,
    /// Exclude blobs larger than the given size in bytes (`blob:limit=N`).
    BlobLimit(u64),
    /// Limit tree traversal depth (`tree:depth=N`).
    TreeDepth(u32),
}

impl ObjectFilter {
    /// Parse a filter specification string.
    ///
    /// Supported formats:
    /// - `blob:none`
    /// - `blob:limit=<size>` where size can have K, M, G suffixes
    /// - `tree:<depth>`
    pub fn parse(spec: &str) -> Result<Self, RevWalkError> {
        if spec == "blob:none" {
            return Ok(Self::BlobNone);
        }

        if let Some(limit_str) = spec.strip_prefix("blob:limit=") {
            let size = parse_size(limit_str).map_err(|e| {
                RevWalkError::InvalidRevision(format!("invalid blob limit: {}", e))
            })?;
            return Ok(Self::BlobLimit(size));
        }

        if let Some(depth_str) = spec.strip_prefix("tree:") {
            let depth: u32 = depth_str
                .parse()
                .map_err(|_| RevWalkError::InvalidRevision(format!("invalid tree depth: {}", depth_str)))?;
            return Ok(Self::TreeDepth(depth));
        }

        Err(RevWalkError::InvalidRevision(format!(
            "unknown filter: {}",
            spec
        )))
    }

    /// Check if an object should be included given this filter.
    pub fn should_include(
        &self,
        repo: &Repository,
        oid: &ObjectId,
        mode: FileMode,
    ) -> Result<bool, RevWalkError> {
        match self {
            Self::BlobNone => {
                // Exclude all blobs.
                Ok(!is_blob_mode(mode))
            }
            Self::BlobLimit(max_size) => {
                if !is_blob_mode(mode) {
                    return Ok(true);
                }
                // Check blob size.
                match repo.odb().read_header(oid)? {
                    Some(info) if info.obj_type == ObjectType::Blob => {
                        Ok((info.size as u64) <= *max_size)
                    }
                    _ => Ok(true),
                }
            }
            Self::TreeDepth(_depth) => {
                // Tree depth filtering is handled at the walk level,
                // not per-object. Always include here.
                Ok(true)
            }
        }
    }
}

fn is_blob_mode(mode: FileMode) -> bool {
    matches!(mode, FileMode::Regular | FileMode::Executable | FileMode::Symlink)
}

/// Parse a size string with optional K, M, G suffix.
fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty size".into());
    }

    let (num_str, multiplier) = if let Some(prefix) = s.strip_suffix('k').or_else(|| s.strip_suffix('K')) {
        (prefix, 1024u64)
    } else if let Some(prefix) = s.strip_suffix('m').or_else(|| s.strip_suffix('M')) {
        (prefix, 1024 * 1024)
    } else if let Some(prefix) = s.strip_suffix('g').or_else(|| s.strip_suffix('G')) {
        (prefix, 1024 * 1024 * 1024)
    } else {
        (s, 1u64)
    };

    let num: u64 = num_str.parse().map_err(|_| format!("invalid number: {}", num_str))?;
    Ok(num * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filter_blob_none() {
        assert_eq!(ObjectFilter::parse("blob:none").unwrap(), ObjectFilter::BlobNone);
    }

    #[test]
    fn parse_filter_blob_limit() {
        assert_eq!(
            ObjectFilter::parse("blob:limit=1024").unwrap(),
            ObjectFilter::BlobLimit(1024)
        );
        assert_eq!(
            ObjectFilter::parse("blob:limit=1k").unwrap(),
            ObjectFilter::BlobLimit(1024)
        );
        assert_eq!(
            ObjectFilter::parse("blob:limit=1M").unwrap(),
            ObjectFilter::BlobLimit(1024 * 1024)
        );
    }

    #[test]
    fn parse_filter_tree_depth() {
        assert_eq!(
            ObjectFilter::parse("tree:0").unwrap(),
            ObjectFilter::TreeDepth(0)
        );
        assert_eq!(
            ObjectFilter::parse("tree:2").unwrap(),
            ObjectFilter::TreeDepth(2)
        );
    }

    #[test]
    fn parse_size_values() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("1K").unwrap(), 1024);
        assert_eq!(parse_size("2M").unwrap(), 2 * 1024 * 1024);
        assert_eq!(parse_size("1G").unwrap(), 1024 * 1024 * 1024);
    }
}

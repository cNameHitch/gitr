//! Index entry types: IndexEntry, StatData, EntryFlags.

use bstr::BString;
use git_hash::ObjectId;
use git_object::FileMode;

use crate::Stage;

/// A single entry in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// File path (relative to repo root).
    pub path: BString,
    /// Object ID of the blob.
    pub oid: ObjectId,
    /// File mode.
    pub mode: FileMode,
    /// Merge stage (0 = normal, 1 = base, 2 = ours, 3 = theirs).
    pub stage: Stage,
    /// Stat data from the file system.
    pub stat: StatData,
    /// Entry flags.
    pub flags: EntryFlags,
}

/// File system stat data cached in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatData {
    pub ctime_secs: u32,
    pub ctime_nsecs: u32,
    pub mtime_secs: u32,
    pub mtime_nsecs: u32,
    pub dev: u32,
    pub ino: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u32,
}

impl StatData {
    /// Create from file system metadata.
    #[cfg(unix)]
    pub fn from_metadata(meta: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            ctime_secs: meta.ctime() as u32,
            ctime_nsecs: meta.ctime_nsec() as u32,
            mtime_secs: meta.mtime() as u32,
            mtime_nsecs: meta.mtime_nsec() as u32,
            dev: meta.dev() as u32,
            ino: meta.ino() as u32,
            uid: meta.uid(),
            gid: meta.gid(),
            size: meta.len() as u32,
        }
    }

    /// Create from file system metadata (non-Unix fallback).
    #[cfg(not(unix))]
    pub fn from_metadata(meta: &std::fs::Metadata) -> Self {
        use std::time::UNIX_EPOCH;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .unwrap_or_default();
        Self {
            ctime_secs: mtime.as_secs() as u32,
            ctime_nsecs: mtime.subsec_nanos(),
            mtime_secs: mtime.as_secs() as u32,
            mtime_nsecs: mtime.subsec_nanos(),
            dev: 0,
            ino: 0,
            uid: 0,
            gid: 0,
            size: meta.len() as u32,
        }
    }

    /// Check if stat data matches file metadata (for change detection).
    /// Uses the same heuristics as C git's `ie_match_stat`.
    pub fn matches(&self, meta: &std::fs::Metadata) -> bool {
        let other = Self::from_metadata(meta);

        // Size mismatch is a definite change
        if self.size != other.size {
            return false;
        }

        // Check mtime (most common change indicator)
        if self.mtime_secs != other.mtime_secs || self.mtime_nsecs != other.mtime_nsecs {
            return false;
        }

        // Check ctime
        if self.ctime_secs != other.ctime_secs || self.ctime_nsecs != other.ctime_nsecs {
            return false;
        }

        // Check inode (detects file replacement on Unix)
        if self.ino != 0 && other.ino != 0 && self.ino != other.ino {
            return false;
        }

        // Check device
        if self.dev != 0 && other.dev != 0 && self.dev != other.dev {
            return false;
        }

        // Check uid/gid
        if self.uid != 0 && other.uid != 0 && self.uid != other.uid {
            return false;
        }
        if self.gid != 0 && other.gid != 0 && self.gid != other.gid {
            return false;
        }

        true
    }
}

/// Entry flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntryFlags {
    /// CE_VALID: assume the entry is unchanged.
    pub assume_valid: bool,
    /// CE_INTENT_TO_ADD: the entry is a placeholder for `git add -N`.
    pub intent_to_add: bool,
    /// CE_SKIP_WORKTREE: the entry should not be checked out.
    pub skip_worktree: bool,
}

impl EntryFlags {
    /// Returns true if any extended flags are set (requiring v3+ format).
    pub fn has_extended(&self) -> bool {
        self.intent_to_add || self.skip_worktree
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_data_default() {
        let stat = StatData::default();
        assert_eq!(stat.size, 0);
        assert_eq!(stat.mtime_secs, 0);
    }

    #[test]
    fn entry_flags_default() {
        let flags = EntryFlags::default();
        assert!(!flags.assume_valid);
        assert!(!flags.intent_to_add);
        assert!(!flags.skip_worktree);
        assert!(!flags.has_extended());
    }

    #[test]
    fn entry_flags_extended() {
        let flags = EntryFlags {
            intent_to_add: true,
            ..Default::default()
        };
        assert!(flags.has_extended());
    }
}

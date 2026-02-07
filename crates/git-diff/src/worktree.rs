//! Working tree diff operations.
//!
//! Provides diff between index and working tree (`git diff`),
//! and between HEAD and index (`git diff --cached`).

use bstr::{BString, ByteSlice};
use git_hash::ObjectId;
use git_index::Stage;
use git_object::FileMode;
use git_repository::Repository;

use crate::algorithm;
use crate::binary::is_binary;
use crate::tree::{diff_trees, read_blob};
use crate::{DiffError, DiffOptions, DiffResult, FileDiff, FileStatus};

/// Compute a blob OID for working tree content.
fn hash_blob(data: &[u8]) -> Option<ObjectId> {
    git_hash::hasher::Hasher::hash_object(git_hash::HashAlgorithm::Sha1, "blob", data).ok()
}

/// Snapshot of an index entry's data needed for diffing.
struct IndexEntrySnapshot {
    path: BString,
    oid: ObjectId,
    mode: FileMode,
    stat: git_index::StatData,
}

/// Diff the index against the working tree (unstaged changes).
///
/// Equivalent to `git diff` (no arguments).
pub fn diff_index_to_worktree(
    repo: &mut Repository,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError> {
    let work_tree = repo
        .work_tree()
        .ok_or(git_repository::RepoError::BareNoWorkTree)?
        .to_path_buf();

    // Collect index entry data upfront to release the mutable borrow on repo.
    let entries: Vec<IndexEntrySnapshot> = {
        let index = repo.index()?;
        index
            .iter()
            .filter(|e| e.stage == Stage::Normal)
            .filter(|e| matches_pathspec(&e.path, options))
            .map(|e| IndexEntrySnapshot {
                path: e.path.clone(),
                oid: e.oid,
                mode: e.mode,
                stat: e.stat,
            })
            .collect()
    };

    let odb = repo.odb();
    let mut files = Vec::new();

    for entry in &entries {
        let fs_path = work_tree.join(entry.path.to_str_lossy().as_ref());

        if !fs_path.exists() {
            // File deleted from working tree but still in index
            let blob_data = read_blob(odb, &entry.oid)?;
            let binary = is_binary(&blob_data);
            let hunks = if binary {
                Vec::new()
            } else {
                algorithm::diff_lines(&blob_data, &[], options.algorithm, options.context_lines)
            };
            files.push(FileDiff {
                status: FileStatus::Deleted,
                old_path: Some(entry.path.clone()),
                new_path: None,
                old_mode: Some(entry.mode),
                new_mode: None,
                old_oid: Some(entry.oid),
                new_oid: None,
                hunks,
                is_binary: binary,
                similarity: None,
            });
            continue;
        }

        let metadata = std::fs::symlink_metadata(&fs_path)?;

        // Use stat comparison first (fast path)
        if entry.stat.matches(&metadata) {
            continue; // No change
        }

        // Stat differs: read the file and compare content
        let worktree_content = std::fs::read(&fs_path)?;
        let blob_data = read_blob(odb, &entry.oid)?;

        if worktree_content == blob_data {
            // Content identical despite stat change (racily clean)
            continue;
        }

        // Determine mode change
        let new_mode = file_mode_from_metadata(&metadata);

        let binary = is_binary(&blob_data) || is_binary(&worktree_content);

        let hunks = if binary {
            Vec::new()
        } else {
            algorithm::diff_lines(&blob_data, &worktree_content, options.algorithm, options.context_lines)
        };

        files.push(FileDiff {
            status: if entry.mode != new_mode && !mode_is_same_type(entry.mode, new_mode) {
                FileStatus::TypeChanged
            } else {
                FileStatus::Modified
            },
            old_path: Some(entry.path.clone()),
            new_path: Some(entry.path.clone()),
            old_mode: Some(entry.mode),
            new_mode: Some(new_mode),
            old_oid: Some(entry.oid),
            new_oid: hash_blob(&worktree_content),
            hunks,
            is_binary: binary,
            similarity: None,
        });
    }

    Ok(DiffResult { files })
}

/// Diff HEAD against the index (staged changes).
///
/// Equivalent to `git diff --cached` or `git diff --staged`.
pub fn diff_head_to_index(
    repo: &mut Repository,
    options: &DiffOptions,
) -> Result<DiffResult, DiffError> {
    let head_tree_oid = resolve_head_tree(repo)?;

    // Load index first (triggers lazy load via &mut self), then use write_tree.
    // We force the index load, drop the returned ref, and re-borrow immutably
    // via the index_path. Since index() caches the result, a second call just
    // returns the cached version. The borrow checker still sees this as a
    // &mut self borrow, so we use the free function + read the index file directly.
    let index_path = repo.git_dir().join("index");
    let index_for_tree = if index_path.exists() {
        git_index::Index::read_from(&index_path)
            .map_err(|e| DiffError::Io(std::io::Error::other(e.to_string())))?
    } else {
        git_index::Index::new()
    };
    let index_tree_oid = index_for_tree
        .write_tree(repo.odb())
        .map_err(|e| DiffError::Io(std::io::Error::other(e.to_string())))?;

    diff_trees(
        repo.odb(),
        head_tree_oid.as_ref(),
        Some(&index_tree_oid),
        options,
    )
}

/// Resolve HEAD to a tree OID. Returns None for an unborn branch.
fn resolve_head_tree(repo: &Repository) -> Result<Option<ObjectId>, DiffError> {
    let head_oid = match repo.head_oid()? {
        Some(oid) => oid,
        None => return Ok(None), // Unborn branch
    };

    // Read the commit to get its tree
    let obj = repo
        .odb()
        .read(&head_oid)
        .map_err(|e| DiffError::ObjectRead {
            oid: head_oid,
            source: e,
        })?
        .ok_or(DiffError::ObjectNotFound(head_oid))?;

    match obj {
        git_object::Object::Commit(commit) => Ok(Some(commit.tree)),
        other => Err(DiffError::UnexpectedObjectType {
            oid: head_oid,
            expected: "commit",
            actual: other.object_type().to_string(),
        }),
    }
}

/// Determine FileMode from filesystem metadata.
fn file_mode_from_metadata(meta: &std::fs::Metadata) -> FileMode {
    if meta.is_symlink() {
        FileMode::Symlink
    } else if meta.is_dir() {
        FileMode::Tree
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 != 0 {
                return FileMode::Executable;
            }
        }
        FileMode::Regular
    }
}

/// Check if two modes represent the same type.
fn mode_is_same_type(a: FileMode, b: FileMode) -> bool {
    a.is_blob() == b.is_blob()
        && a.is_symlink() == b.is_symlink()
        && a.is_gitlink() == b.is_gitlink()
}

/// Check if a path matches the pathspec filter.
fn matches_pathspec(path: &BString, options: &DiffOptions) -> bool {
    match &options.pathspec {
        None => true,
        Some(specs) => specs
            .iter()
            .any(|spec| path.starts_with(spec.as_slice())),
    }
}

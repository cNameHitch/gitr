use std::path::{Path, PathBuf};

use crate::{DiscoveredRepo, RepoError, RepositoryKind};

/// Open a repository from a gitdir: redirect target.
///
/// When `.git` is a file containing `gitdir: <path>`, that path either:
/// - Points to a worktree git dir (e.g., `.git/worktrees/<name>`)
/// - Points to another location (submodule or relocated git dir)
pub fn open_from_gitdir_redirect(
    target_git_dir: &Path,
    work_tree: &Path,
) -> Result<DiscoveredRepo, RepoError> {
    let commondir_file = target_git_dir.join("commondir");

    if commondir_file.is_file() {
        // This is a linked worktree
        let common_dir = resolve_commondir(target_git_dir, &commondir_file)?;
        Ok(DiscoveredRepo {
            git_dir: target_git_dir.to_path_buf(),
            work_tree: Some(work_tree.to_path_buf()),
            common_dir,
            kind: RepositoryKind::LinkedWorktree,
        })
    } else {
        // Not a worktree — just a redirected git dir
        // Verify it looks like a git dir
        if !target_git_dir.join("HEAD").is_file() {
            return Err(RepoError::InvalidGitDir {
                path: target_git_dir.to_path_buf(),
                reason: "gitdir redirect target is not a valid git directory".to_string(),
            });
        }
        Ok(DiscoveredRepo {
            git_dir: target_git_dir.to_path_buf(),
            work_tree: Some(work_tree.to_path_buf()),
            common_dir: target_git_dir.to_path_buf(),
            kind: RepositoryKind::Normal,
        })
    }
}

/// Resolve the commondir file to get the shared directory path.
fn resolve_commondir(git_dir: &Path, commondir_file: &Path) -> Result<PathBuf, RepoError> {
    let content = std::fs::read_to_string(commondir_file).map_err(|e| {
        RepoError::InvalidGitDir {
            path: commondir_file.to_path_buf(),
            reason: format!("cannot read commondir file: {e}"),
        }
    })?;
    let relative = content.trim();
    let resolved = git_dir.join(relative);
    std::fs::canonicalize(&resolved).map_err(|e| RepoError::InvalidGitDir {
        path: resolved,
        reason: format!("cannot resolve commondir: {e}"),
    })
}

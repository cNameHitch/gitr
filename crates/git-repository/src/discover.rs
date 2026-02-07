use std::path::{Path, PathBuf};

use crate::env::EnvOverrides;
use crate::worktree;
use crate::{DiscoveredRepo, RepoError, RepositoryKind};

/// Discover a git repository by walking up from `start`.
///
/// Follows C git's `setup_git_directory()` algorithm:
/// 1. Check `$GIT_DIR` → use directly if set
/// 2. Walk up from `start`:
///    a. Check for `.git/` directory
///    b. Check for `.git` file (gitdir: redirect)
///    c. Check if the dir itself is a bare repo (has HEAD, objects/, refs/)
///    d. Check against `$GIT_CEILING_DIRECTORIES`
///    e. Go to parent, repeat
pub fn discover_git_dir(start: &Path) -> Result<DiscoveredRepo, RepoError> {
    let env_overrides = EnvOverrides::from_env();
    discover_git_dir_with_env(start, &env_overrides)
}

/// Discover with explicit environment overrides (for testing).
pub fn discover_git_dir_with_env(
    start: &Path,
    env: &EnvOverrides,
) -> Result<DiscoveredRepo, RepoError> {
    // If GIT_DIR is set, use it directly
    if let Some(ref git_dir) = env.git_dir {
        let git_dir = if git_dir.is_absolute() {
            git_dir.clone()
        } else {
            start.join(git_dir)
        };
        return open_git_dir(&git_dir);
    }

    let start = std::fs::canonicalize(start)
        .map_err(|_| RepoError::NotFound(start.to_path_buf()))?;

    // Canonicalize ceiling directories for comparison
    let ceilings: Vec<PathBuf> = env
        .ceiling_directories
        .iter()
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .collect();

    let mut current = start.clone();
    loop {
        // Check ceiling directories
        if ceilings.contains(&current) {
            return Err(RepoError::NotFound(start));
        }

        let dot_git = current.join(".git");

        if dot_git.is_dir() {
            // Found a .git directory
            let common_dir = resolve_common_dir(&dot_git);
            return Ok(DiscoveredRepo {
                git_dir: dot_git,
                work_tree: Some(current),
                common_dir,
                kind: RepositoryKind::Normal,
            });
        }

        if dot_git.is_file() {
            // .git file — read gitdir: redirect
            let target = parse_gitdir_file(&dot_git)?;
            let target = if target.is_absolute() {
                target
            } else {
                current.join(&target)
            };
            let target = std::fs::canonicalize(&target).map_err(|e| {
                RepoError::InvalidGitDir {
                    path: dot_git.clone(),
                    reason: format!("cannot resolve gitdir target: {e}"),
                }
            })?;
            return worktree::open_from_gitdir_redirect(&target, &current);
        }

        // Check if current directory IS a bare git repo
        if is_git_dir(&current) {
            let common_dir = resolve_common_dir(&current);
            return Ok(DiscoveredRepo {
                git_dir: current,
                work_tree: None,
                common_dir,
                kind: RepositoryKind::Bare,
            });
        }

        // Go to parent
        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => {
                return Err(RepoError::NotFound(start));
            }
        }
    }
}

/// Open a known git directory path directly.
pub fn open_git_dir(git_dir: &Path) -> Result<DiscoveredRepo, RepoError> {
    let git_dir = std::fs::canonicalize(git_dir)
        .map_err(|_| RepoError::NotFound(git_dir.to_path_buf()))?;

    if !is_git_dir(&git_dir) {
        return Err(RepoError::InvalidGitDir {
            path: git_dir,
            reason: "missing HEAD, objects/, or refs/".to_string(),
        });
    }

    let common_dir = resolve_common_dir(&git_dir);

    // If the git dir has a commondir file, it's a linked worktree's git dir
    if git_dir.join("commondir").is_file() {
        // This is a worktree git dir (e.g., .git/worktrees/<name>)
        let gitdir_file = git_dir.join("gitdir");
        let work_tree = if gitdir_file.is_file() {
            let wt = std::fs::read_to_string(&gitdir_file)
                .map_err(|e| RepoError::InvalidGitDir {
                    path: gitdir_file,
                    reason: e.to_string(),
                })?;
            let wt = wt.trim();
            // The gitdir file in worktree git dir points to the .git file in the worktree
            // The worktree is the parent of that .git file
            let wt_path = PathBuf::from(wt);
            wt_path.parent().map(|p| p.to_path_buf())
        } else {
            None
        };
        return Ok(DiscoveredRepo {
            git_dir,
            work_tree,
            common_dir,
            kind: RepositoryKind::LinkedWorktree,
        });
    }

    // Check if the git dir is inside a working tree (i.e., parent has .git = this dir)
    if let Some(parent) = git_dir.parent() {
        if parent.join(".git") == git_dir {
            return Ok(DiscoveredRepo {
                work_tree: Some(parent.to_path_buf()),
                common_dir,
                git_dir,
                kind: RepositoryKind::Normal,
            });
        }
    }

    // Must be a bare repo
    Ok(DiscoveredRepo {
        common_dir: common_dir.clone(),
        git_dir,
        work_tree: None,
        kind: RepositoryKind::Bare,
    })
}

/// Open a git dir when we know the working tree root (e.g., path/.git exists).
pub fn open_git_dir_from_work_tree(work_tree: &Path) -> Result<DiscoveredRepo, RepoError> {
    let dot_git = work_tree.join(".git");

    if dot_git.is_dir() {
        let dot_git = std::fs::canonicalize(&dot_git)
            .map_err(|_| RepoError::NotFound(dot_git.clone()))?;
        let work_tree = std::fs::canonicalize(work_tree)
            .map_err(|_| RepoError::NotFound(work_tree.to_path_buf()))?;
        let common_dir = resolve_common_dir(&dot_git);
        return Ok(DiscoveredRepo {
            git_dir: dot_git,
            work_tree: Some(work_tree),
            common_dir,
            kind: RepositoryKind::Normal,
        });
    }

    if dot_git.is_file() {
        let target = parse_gitdir_file(&dot_git)?;
        let target = if target.is_absolute() {
            target
        } else {
            work_tree.join(&target)
        };
        let target = std::fs::canonicalize(&target).map_err(|e| RepoError::InvalidGitDir {
            path: dot_git,
            reason: format!("cannot resolve gitdir target: {e}"),
        })?;
        let work_tree = std::fs::canonicalize(work_tree)
            .map_err(|_| RepoError::NotFound(work_tree.to_path_buf()))?;
        return worktree::open_from_gitdir_redirect(&target, &work_tree);
    }

    Err(RepoError::NotFound(work_tree.to_path_buf()))
}

/// Check if a directory looks like a git dir (has HEAD, objects/, refs/).
pub fn is_git_dir(path: &Path) -> bool {
    path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir()
}

/// Parse a `.git` file containing `gitdir: <path>`.
pub fn parse_gitdir_file(path: &Path) -> Result<PathBuf, RepoError> {
    let content = std::fs::read_to_string(path).map_err(|e| RepoError::InvalidGitDir {
        path: path.to_path_buf(),
        reason: format!("cannot read .git file: {e}"),
    })?;
    let content = content.trim();
    let target = content.strip_prefix("gitdir: ").ok_or_else(|| {
        RepoError::InvalidGitDir {
            path: path.to_path_buf(),
            reason: format!("expected 'gitdir: <path>', got: {content}"),
        }
    })?;
    Ok(PathBuf::from(target))
}

/// Resolve the common dir for a git directory.
///
/// If the git dir has a `commondir` file, it points to the shared directory.
/// Otherwise, the common dir is the git dir itself.
fn resolve_common_dir(git_dir: &Path) -> PathBuf {
    let commondir_file = git_dir.join("commondir");
    if commondir_file.is_file() {
        if let Ok(content) = std::fs::read_to_string(&commondir_file) {
            let relative = content.trim();
            let resolved = git_dir.join(relative);
            if let Ok(canonical) = std::fs::canonicalize(&resolved) {
                return canonical;
            }
            return resolved;
        }
    }
    git_dir.to_path_buf()
}

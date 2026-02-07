use std::fs;
use std::path::Path;

use crate::{DiscoveredRepo, InitOptions, RepoError, RepositoryKind};

/// Initialize a new git repository.
///
/// Creates the standard `.git/` structure:
/// - HEAD (pointing to default branch)
/// - config
/// - objects/
/// - objects/info/
/// - objects/pack/
/// - refs/
/// - refs/heads/
/// - refs/tags/
/// - hooks/
/// - info/
/// - info/exclude
/// - description
pub fn init_repository(path: &Path, options: &InitOptions) -> Result<DiscoveredRepo, RepoError> {
    let path = if path.is_relative() {
        std::env::current_dir()?.join(path)
    } else {
        path.to_path_buf()
    };

    let (git_dir, work_tree) = if options.bare {
        (path.clone(), None)
    } else {
        (path.join(".git"), Some(path.clone()))
    };

    // If git_dir already exists with HEAD, treat as reinit (safe no-op)
    if git_dir.join("HEAD").is_file() {
        // Re-running init on an existing repo is a safe no-op.
        // We do NOT overwrite existing data.
        return Ok(DiscoveredRepo {
            git_dir: git_dir.clone(),
            work_tree,
            common_dir: git_dir,
            kind: if options.bare {
                RepositoryKind::Bare
            } else {
                RepositoryKind::Normal
            },
        });
    }

    // Create directory structure
    fs::create_dir_all(&git_dir)?;
    fs::create_dir_all(git_dir.join("objects").join("info"))?;
    fs::create_dir_all(git_dir.join("objects").join("pack"))?;
    fs::create_dir_all(git_dir.join("refs").join("heads"))?;
    fs::create_dir_all(git_dir.join("refs").join("tags"))?;
    fs::create_dir_all(git_dir.join("hooks"))?;
    fs::create_dir_all(git_dir.join("info"))?;

    // Determine default branch name
    let default_branch = options
        .default_branch
        .as_deref()
        .unwrap_or("main");

    // Write HEAD
    fs::write(
        git_dir.join("HEAD"),
        format!("ref: refs/heads/{default_branch}\n"),
    )?;

    // Write config
    let config_content = if options.bare {
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = true\n"
    } else {
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n"
    };
    fs::write(git_dir.join("config"), config_content)?;

    // Write description
    fs::write(
        git_dir.join("description"),
        "Unnamed repository; edit this file 'description' to name the repository.\n",
    )?;

    // Write info/exclude
    fs::write(
        git_dir.join("info").join("exclude"),
        "# git ls-files --others --exclude-from=.git/info/exclude\n# Lines that start with '#' are comments.\n",
    )?;

    // Copy template directory if specified
    if let Some(ref template_dir) = options.template_dir {
        if template_dir.is_dir() {
            copy_template(template_dir, &git_dir)?;
        }
    }

    Ok(DiscoveredRepo {
        git_dir: git_dir.clone(),
        work_tree,
        common_dir: git_dir,
        kind: if options.bare {
            RepositoryKind::Bare
        } else {
            RepositoryKind::Normal
        },
    })
}

/// Copy template directory contents into the git dir.
///
/// Files from the template are only copied if they don't already exist in the target.
fn copy_template(template_dir: &Path, git_dir: &Path) -> Result<(), RepoError> {
    copy_dir_recursive(template_dir, git_dir)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), RepoError> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            if !dst_path.exists() {
                fs::create_dir_all(&dst_path)?;
            }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() && !dst_path.exists() {
            // Only copy if target doesn't exist (don't overwrite HEAD, config, etc.)
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

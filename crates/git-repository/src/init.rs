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
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = true\n".to_string()
    } else {
        let mut content = String::from(
            "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n",
        );
        #[cfg(target_os = "macos")]
        {
            content.push_str("\tignorecase = true\n\tprecomposeunicode = true\n");
        }
        content
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

    // Write sample hook files
    write_sample_hooks(&git_dir)?;

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

/// Write standard sample hook files to .git/hooks/.
fn write_sample_hooks(git_dir: &Path) -> Result<(), RepoError> {
    let hooks_dir = git_dir.join("hooks");

    let samples: &[(&str, &str)] = &[
        ("applypatch-msg.sample", "#!/bin/sh\n#\n# An example hook script to check the commit log message taken by\n# applypatch from an e-mail message.\n#\n# The hook should exit with non-zero status after issuing an\n# appropriate message if it wants to stop the commit.\n#\n# To enable this hook, rename this file to \"applypatch-msg\".\n\n. git-sh-setup\ncommitmsg=\"$(git rev-parse --git-path hooks/commit-msg)\"\ntest -x \"$commitmsg\" && exec \"$commitmsg\" ${1+\"$@\"}\n:\n"),
        ("commit-msg.sample", "#!/bin/sh\n#\n# An example hook script to check the commit log message.\n# Called by \"git commit\" with one argument, the name of the file\n# that has the commit message.\n#\n# To enable this hook, rename this file to \"commit-msg\".\n\n# This example catches duplicate Signed-off-by lines.\n\ntest \"\" = \"$(grep '^Signed-off-by: ' \"$1\" |\n\t sort | uniq -c | sed -e '/^[ \t]*1[ \t]/d')\" || {\n\techo >&2 Duplicate Signed-off-by lines.\n\texit 1\n}\n"),
        ("pre-applypatch.sample", "#!/bin/sh\n#\n# An example hook script to verify what is about to be committed\n# by applypatch from an e-mail message.\n#\n# The hook should exit with non-zero status after issuing an\n# appropriate message if it wants to stop the commit.\n#\n# To enable this hook, rename this file to \"pre-applypatch\".\n\n. git-sh-setup\ntest -x \"$GIT_DIR/hooks/pre-commit\" &&\n\texec \"$GIT_DIR/hooks/pre-commit\" ${1+\"$@\"}\n:\n"),
        ("pre-commit.sample", "#!/bin/sh\n#\n# An example hook script to verify what is about to be committed.\n# Called by \"git commit\" with no arguments.\n#\n# To enable this hook, rename this file to \"pre-commit\".\n\nexec git diff-index --check --cached HEAD --\n"),
        ("pre-push.sample", "#!/bin/sh\n#\n# An example hook script to verify what is about to be pushed.\n#\n# Called by \"git push\" after it has checked the remote status, but before\n# anything has been pushed.\n#\n# To enable this hook, rename this file to \"pre-push\".\n\nremote=\"$1\"\nurl=\"$2\"\n\nexit 0\n"),
        ("pre-rebase.sample", "#!/bin/sh\n#\n# Copyright (c) 2006, 2008 Junio C Hamano\n#\n# An example hook script to prepare a packed repository for use over\n# dumb transports.\n#\n# To enable this hook, rename this file to \"pre-rebase\".\n\n:\n"),
        ("prepare-commit-msg.sample", "#!/bin/sh\n#\n# An example hook script to prepare the commit log message.\n# Called by \"git commit\" with the name of the file that has the\n# commit message, followed by the description of the commit\n# message's source.\n#\n# To enable this hook, rename this file to \"prepare-commit-msg\".\n\n:\n"),
        ("post-update.sample", "#!/bin/sh\n#\n# An example hook script to prepare a packed repository for use over\n# dumb transports.\n#\n# To enable this hook, rename this file to \"post-update\".\n\nexec git update-server-info\n"),
        ("fsmonitor-watchman.sample", "#!/usr/bin/perl\n#\n# An example hook script to integrate Watchman.\n# https://facebook.github.io/watchman/\n#\n# To enable this hook, rename this file to \"query-watchman\".\n\ndie \"Not implemented.\\n\";\n"),
        ("push-to-checkout.sample", "#!/bin/sh\n#\n# An example hook script to update a checked-out tree.\n#\n# To enable this hook, rename this file to \"push-to-checkout\".\n\n:\n"),
        ("sendemail-validate.sample", "#!/bin/sh\n#\n# An example hook script to validate patches/emails.\n#\n# To enable this hook, rename this file to \"sendemail-validate\".\n\n:\n"),
    ];

    for (name, content) in samples {
        let path = hooks_dir.join(name);
        if !path.exists() {
            fs::write(&path, content)?;
            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
        }
    }

    Ok(())
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

use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice};
use clap::Args;
use git_hash::ObjectId;
use git_index::{EntryFlags, IndexEntry, Stage, StatData};
use git_object::{Commit, FileMode, Object, ObjectType};
use git_ref::reflog::{append_reflog_entry, ReflogEntry};
use git_ref::{RefName, RefStore, Reference};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CommitArgs {
    /// Use the given message as the commit message (can be specified multiple times)
    #[arg(short = 'm', num_args = 1)]
    message: Vec<String>,

    /// Automatically stage all tracked modified files before committing
    #[arg(short = 'a', long = "all")]
    all: bool,

    /// Replace the tip of the current branch by creating a new commit
    #[arg(long)]
    amend: bool,

    /// Allow creating a commit with no changes from the parent
    #[arg(long)]
    allow_empty: bool,

    /// Open an editor for the commit message
    #[arg(short = 'e', long = "edit")]
    edit: bool,

    /// With --amend, reuse the previous commit's message without editing
    #[arg(long)]
    no_edit: bool,

    /// Override the author (format: "Name <email>")
    #[arg(long, value_name = "author")]
    author: Option<String>,
}

pub fn run(args: &CommitArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    // 1. Check for conflicts
    {
        let index = repo.index()?;
        let conflicts = index.conflicts();
        if !conflicts.is_empty() {
            bail!(
                "cannot commit: you have unmerged paths.\n\
                 fix conflicts and then commit the result."
            );
        }
    }

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("cannot commit in a bare repository"))?
        .to_path_buf();

    // 2. If -a, auto-stage all tracked modified files
    if args.all {
        auto_stage_tracked(&mut repo, &work_tree)?;
    }

    // 2b. Run pre-commit hook
    let git_dir = repo.git_dir().to_path_buf();
    if !run_hook(&git_dir, "pre-commit", &[])? {
        bail!("pre-commit hook failed");
    }

    // 3. Build tree from index via write_tree
    let index_path = repo.git_dir().join("index");
    let index = git_index::Index::read_from(&index_path)?;
    let tree_oid = index.write_tree(repo.odb())?;

    // 4. Get parent commit(s) from HEAD (or none for initial commit)
    let is_unborn = repo.is_unborn()?;
    let mut parents: Vec<ObjectId> = Vec::new();
    let mut prev_commit: Option<Commit> = None;

    if args.amend {
        if is_unborn {
            bail!("cannot amend: no existing commit to amend");
        }
        let head_oid = repo
            .head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD does not point to a valid commit"))?;
        let head_obj = repo
            .odb()
            .read(&head_oid)?
            .ok_or_else(|| anyhow::anyhow!("failed to read HEAD commit object"))?;
        let commit = match head_obj {
            Object::Commit(c) => c,
            _ => bail!("HEAD does not point to a commit object"),
        };
        // For amend, use the original commit's parents
        parents = commit.parents.clone();
        prev_commit = Some(commit);
    } else if !is_unborn {
        if let Some(head_oid) = repo.head_oid()? {
            parents.push(head_oid);
        }
    }

    // Check for empty commits (tree unchanged from parent)
    if !args.allow_empty && !args.amend && !is_unborn {
        if let Some(parent_oid) = parents.first() {
            let parent_obj = repo
                .odb()
                .read(parent_oid)?
                .ok_or_else(|| anyhow::anyhow!("failed to read parent commit"))?;
            if let Object::Commit(parent_commit) = parent_obj {
                if parent_commit.tree == tree_oid {
                    bail!(
                        "nothing to commit, working tree clean\n\
                         (use --allow-empty to override)"
                    );
                }
            }
        }
    }

    if !args.allow_empty && args.amend {
        if let Some(ref pc) = prev_commit {
            // For amend, allow if tree differs from the amended commit's tree
            // or if the message is changing. We allow amend by default since
            // the user explicitly requested it.
            let _ = pc;
        }
    }

    // 5. Build Signature from config or env vars
    let author = if let Some(ref author_str) = args.author {
        parse_author_override(author_str)?
    } else if args.amend {
        // When amending, reuse the original author by default
        if let Some(ref pc) = prev_commit {
            pc.author.clone()
        } else {
            get_signature("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", &repo)?
        }
    } else {
        get_signature("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE", &repo)?
    };
    let committer = get_signature(
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "GIT_COMMITTER_DATE",
        &repo,
    )?;

    // 6/7. Determine commit message
    let mut message = determine_message(args, prev_commit.as_ref())?;

    // Run commit-msg hook (may modify the message)
    let commit_msg_hook = git_dir.join("hooks").join("commit-msg");
    if commit_msg_hook.exists() {
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), message.as_slice())?;
        let tmp_path_str = tmp.path().to_string_lossy().to_string();
        if !run_hook(&git_dir, "commit-msg", &[&tmp_path_str])? {
            bail!("commit-msg hook failed");
        }
        message = BString::from(std::fs::read(tmp.path())?);
    }

    // Ensure message is not empty
    let trimmed = message.trim();
    if trimmed.is_empty() {
        bail!("Aborting commit due to empty commit message.");
    }

    // 8. Create Commit object and write to ODB
    let commit = Commit {
        tree: tree_oid,
        parents,
        author,
        committer,
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
        message,
    };

    let obj = Object::Commit(commit.clone());
    let commit_oid = repo.odb().write(&obj)?;

    // 9. Update HEAD ref
    let old_head_oid = repo.head_oid()?.unwrap_or(ObjectId::NULL_SHA1);
    update_head(&repo, &commit_oid)?;

    // Write reflog entry for HEAD
    {
        let reflog_msg = if is_unborn {
            format!("commit (initial): {}", String::from_utf8_lossy(commit.summary()))
        } else if args.amend {
            format!("commit (amend): {}", String::from_utf8_lossy(commit.summary()))
        } else {
            format!("commit: {}", String::from_utf8_lossy(commit.summary()))
        };
        let entry = ReflogEntry {
            old_oid: old_head_oid,
            new_oid: commit_oid,
            identity: commit.committer.clone(),
            message: BString::from(reflog_msg),
        };
        let head_ref = RefName::new(BString::from("HEAD"))?;
        append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
    }

    // Run post-commit hook (ignore exit code)
    let _ = run_hook(&git_dir, "post-commit", &[]);

    // 10. Print summary
    print_summary(&repo, &commit, &commit_oid, is_unborn, args.amend)?;

    Ok(0)
}

/// Auto-stage all tracked modified files (implements -a flag).
fn auto_stage_tracked(
    repo: &mut git_repository::Repository,
    work_tree: &std::path::Path,
) -> Result<()> {
    // Load the index to get tracked paths
    let _ = repo.index_mut()?;

    // Collect entries that need updating
    let entries_to_update: Vec<(String, bool)> = {
        let index = repo.index()?;
        index
            .iter()
            .filter(|e| e.stage == Stage::Normal)
            .map(|entry| {
                let path_str = entry.path.to_str_lossy().to_string();
                let file_path = work_tree.join(&path_str);
                let file_exists = file_path.exists();
                (path_str, file_exists)
            })
            .collect()
    };

    let mut changed = false;

    for (path_str, file_exists) in &entries_to_update {
        let file_path = work_tree.join(path_str);

        if !file_exists {
            // File was deleted - remove from index
            let bpath = bstr::BStr::new(path_str.as_bytes());
            let index = repo.index_mut()?;
            index.remove(bpath, Stage::Normal);
            changed = true;
            continue;
        }

        // Check if the file has been modified by comparing stat data
        let meta = std::fs::metadata(&file_path)?;
        let needs_update = {
            let index = repo.index()?;
            if let Some(entry) = index.get(bstr::BStr::new(path_str.as_bytes()), Stage::Normal) {
                !entry.stat.matches(&meta)
            } else {
                false
            }
        };

        if needs_update {
            let data = std::fs::read(&file_path)?;
            let oid = repo.odb().write_raw(ObjectType::Blob, &data)?;

            let mode = if is_executable(&meta) {
                FileMode::Executable
            } else {
                FileMode::Regular
            };

            let entry = IndexEntry {
                path: BString::from(path_str.as_str()),
                oid,
                mode,
                stage: Stage::Normal,
                stat: StatData::from_metadata(&meta),
                flags: EntryFlags::default(),
            };

            let index = repo.index_mut()?;
            index.add(entry);
            changed = true;
        }
    }

    if changed {
        repo.write_index()?;
    }

    Ok(())
}

/// Determine the commit message from flags and editor.
fn determine_message(args: &CommitArgs, prev_commit: Option<&Commit>) -> Result<BString> {
    // --no-edit with --amend: reuse previous message
    if args.no_edit && args.amend {
        if let Some(pc) = prev_commit {
            return Ok(pc.message.clone());
        }
        bail!("--no-edit requires --amend with an existing commit");
    }

    // -m messages provided
    if !args.message.is_empty() {
        let combined = args.message.join("\n\n");
        let mut msg = combined;
        // Ensure trailing newline
        if !msg.ends_with('\n') {
            msg.push('\n');
        }

        // If -e is also specified, open editor with the pre-filled message
        if args.edit {
            return launch_editor(Some(&msg));
        }

        return Ok(BString::from(msg));
    }

    // No -m and no --no-edit: launch editor
    let template = if args.amend {
        prev_commit.map(|pc| {
            let msg: &[u8] = pc.message.as_ref();
            String::from_utf8_lossy(msg).to_string()
        })
    } else {
        None
    };

    launch_editor(template.as_deref())
}

/// Launch an editor to compose the commit message.
fn launch_editor(initial_content: Option<&str>) -> Result<BString> {
    let editor = std::env::var("GIT_EDITOR")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    // Create a temporary file for the commit message
    let tmp_dir = std::env::temp_dir();
    let msg_path = tmp_dir.join("COMMIT_EDITMSG");

    // Write initial content or template
    let content = if let Some(initial) = initial_content {
        initial.to_string()
    } else {
        "\n# Enter the commit message for your changes.\n\
         # Lines starting with '#' will be ignored.\n"
            .to_string()
    };
    std::fs::write(&msg_path, &content)?;

    // Launch the editor
    let status = Command::new(&editor)
        .arg(&msg_path)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to launch editor '{}': {}", editor, e))?;

    if !status.success() {
        bail!("editor '{}' exited with non-zero status", editor);
    }

    // Read back the edited message, stripping comment lines
    let raw = std::fs::read_to_string(&msg_path)?;
    let filtered: Vec<&str> = raw
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    let mut message = filtered.join("\n");

    // Ensure trailing newline
    if !message.ends_with('\n') {
        message.push('\n');
    }

    // Clean up
    let _ = std::fs::remove_file(&msg_path);

    Ok(BString::from(message))
}

/// Update HEAD to point to the new commit.
fn update_head(repo: &git_repository::Repository, commit_oid: &ObjectId) -> Result<()> {
    let refs = repo.refs();
    let head_ref = RefName::new("HEAD")?;

    // Determine what HEAD points to
    match refs.resolve(&head_ref)? {
        Some(Reference::Symbolic { target, .. }) => {
            // HEAD is a symbolic ref (e.g., refs/heads/main)
            // Update the target branch ref
            refs.write_ref(&target, commit_oid)?;
        }
        Some(Reference::Direct { .. }) => {
            // Detached HEAD - update HEAD directly
            refs.write_ref(&head_ref, commit_oid)?;
        }
        None => {
            // No HEAD at all (shouldn't happen in a valid repo, but handle gracefully)
            refs.write_ref(&head_ref, commit_oid)?;
        }
    }

    Ok(())
}

/// Print the commit summary.
fn print_summary(
    repo: &git_repository::Repository,
    commit: &Commit,
    oid: &ObjectId,
    is_initial: bool,
    is_amend: bool,
) -> Result<()> {
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let hex = oid.to_hex();
    let short_sha = &hex[..7.min(hex.len())];

    let branch_name = if is_initial {
        match repo.current_branch()? {
            Some(name) => format!("{} (root-commit)", name),
            None => "(root-commit)".to_string(),
        }
    } else {
        match repo.current_branch()? {
            Some(name) => name,
            None => format!("(HEAD detached at {})", short_sha),
        }
    };

    let summary = commit.summary();
    let summary_str = summary.to_str_lossy();

    writeln!(
        err,
        "[{} {}] {}",
        branch_name, short_sha, summary_str
    )?;

    if is_amend {
        writeln!(
            err,
            " Date: {}",
            commit.author.date.format(&git_utils::date::DateFormat::Default)
        )?;
    }

    // Compute diffstat: parent tree vs commit tree
    let parent_tree = commit.first_parent().and_then(|p| {
        repo.odb().read(p).ok().flatten().and_then(|o| match o {
            Object::Commit(c) => Some(c.tree),
            _ => None,
        })
    });
    let diff_opts = git_diff::DiffOptions::default();
    if let Ok(result) = git_diff::tree::diff_trees(
        repo.odb(),
        parent_tree.as_ref(),
        Some(&commit.tree),
        &diff_opts,
    ) {
        let mut insertions = 0usize;
        let mut deletions = 0usize;
        let file_count = result.files.len();
        for file in &result.files {
            for hunk in &file.hunks {
                for line in &hunk.lines {
                    match line {
                        git_diff::DiffLine::Addition(_) => insertions += 1,
                        git_diff::DiffLine::Deletion(_) => deletions += 1,
                        _ => {}
                    }
                }
            }
        }
        let mut parts = Vec::new();
        parts.push(format!(" {} file{} changed", file_count, if file_count != 1 { "s" } else { "" }));
        if insertions > 0 {
            parts.push(format!("{} insertion{}", insertions, if insertions != 1 { "s(+)" } else { "(+)" }));
        }
        if deletions > 0 {
            parts.push(format!("{} deletion{}", deletions, if deletions != 1 { "s(-)" } else { "(-)" }));
        }
        if file_count > 0 {
            writeln!(err, "{}", parts.join(", "))?;
        }
    }

    Ok(())
}

/// Parse --author="Name <email>" override.
fn parse_author_override(author_str: &str) -> Result<Signature> {
    // Expected format: "Name <email>"
    let gt_pos = author_str
        .rfind('>')
        .ok_or_else(|| anyhow::anyhow!("invalid --author format, expected 'Name <email>'"))?;
    let lt_pos = author_str[..gt_pos]
        .rfind('<')
        .ok_or_else(|| anyhow::anyhow!("invalid --author format, expected 'Name <email>'"))?;

    let name = author_str[..lt_pos].trim();
    let email = &author_str[lt_pos + 1..gt_pos];

    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date: GitDate::now(),
    })
}

/// Build a Signature from environment variables or config.
pub(crate) fn get_signature(
    name_var: &str,
    email_var: &str,
    date_var: &str,
    repo: &git_repository::Repository,
) -> Result<Signature> {
    let name = std::env::var(name_var)
        .ok()
        .or_else(|| {
            repo.config()
                .get_string("user.name")
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let email = std::env::var(email_var)
        .ok()
        .or_else(|| {
            repo.config()
                .get_string("user.email")
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| "unknown@unknown".to_string());

    let date = if let Ok(date_str) = std::env::var(date_var) {
        GitDate::parse_raw(&date_str)?
    } else {
        GitDate::now()
    };

    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date,
    })
}

/// Run a hook script from .git/hooks/{name}. Returns Ok(true) if the hook
/// succeeded or didn't exist, Ok(false) if it failed.
fn run_hook(git_dir: &Path, name: &str, args: &[&str]) -> Result<bool> {
    let hook_path = git_dir.join("hooks").join(name);
    if !hook_path.exists() {
        return Ok(true);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&hook_path)?;
        if meta.permissions().mode() & 0o111 == 0 {
            return Ok(true); // not executable, skip
        }
    }

    let status = Command::new(&hook_path)
        .args(args)
        .status()?;

    Ok(status.success())
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    false
}

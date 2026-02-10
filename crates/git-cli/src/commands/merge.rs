use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice, ByteVec};
use clap::Args;
use git_hash::ObjectId;
use git_index::{EntryFlags, Index, IndexEntry, Stage, StatData};
use git_merge::{MergeOptions, ConflictEntry};
use git_object::{Commit, FileMode, Object};
use git_ref::{RefName, RefStore};
use git_ref::reflog::{ReflogEntry, append_reflog_entry};
use git_revwalk::{merge_base_one, resolve_revision};
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct MergeArgs {
    /// Create a merge commit even when fast-forward is possible
    #[arg(long = "no-ff")]
    pub no_ff: bool,

    /// Only allow fast-forward merges (fail otherwise)
    #[arg(long)]
    pub ff_only: bool,

    /// Squash the merge into a single set of changes (don't commit)
    #[arg(long)]
    pub squash: bool,

    /// Abort the current in-progress merge
    #[arg(long)]
    pub abort: bool,

    /// Continue after resolving conflicts
    #[arg(long, name = "continue")]
    pub cont: bool,

    /// Perform the merge but don't create a commit
    #[arg(long = "no-commit")]
    pub no_commit: bool,

    /// Use the auto-generated message without launching an editor
    #[arg(long)]
    pub no_edit: bool,

    /// Merge commit message
    #[arg(short = 'm')]
    pub message: Option<String>,

    /// Branch or commit to merge
    #[arg(required_unless_present_any = ["abort", "continue"])]
    pub commit: Option<String>,
}

pub fn run(args: &MergeArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;

    let stderr = io::stderr();
    let mut err = stderr.lock();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Handle --abort
    if args.abort {
        return handle_abort(&mut repo, &mut err);
    }

    // Handle --continue
    if args.cont {
        return handle_continue(&mut repo, &mut out, &mut err);
    }

    // Check for in-progress merge
    let merge_head_path = repo.git_dir().join("MERGE_HEAD");
    if merge_head_path.exists() {
        bail!("you have not concluded your merge (MERGE_HEAD exists).\nPlease, commit your changes before you merge.\nExiting because of unfinished merge.");
    }

    // Get HEAD oid
    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("cannot merge into an unborn branch"))?;

    // Resolve the merge target
    let commit_spec = args
        .commit
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no commit specified to merge"))?;
    let theirs_oid = resolve_revision(&repo, commit_spec)?;

    // Already up to date?
    if head_oid == theirs_oid {
        writeln!(err, "Already up to date.")?;
        return Ok(0);
    }

    // Find merge base
    let base_oid = merge_base_one(&repo, &head_oid, &theirs_oid)?;

    // Check if theirs is already an ancestor of HEAD (already merged)
    if let Some(ref base) = base_oid {
        if *base == theirs_oid {
            writeln!(err, "Already up to date.")?;
            return Ok(0);
        }
    }

    // Determine target label for messages
    let hex = theirs_oid.to_hex();
    let theirs_label = args
        .commit
        .as_deref()
        .unwrap_or(&hex);

    // Check for fast-forward possibility
    let can_ff = match base_oid {
        Some(ref base) => *base == head_oid,
        None => false,
    };

    if can_ff && !args.no_ff {
        // Fast-forward merge
        if args.squash {
            writeln!(err, "Squash commit -- not updating HEAD")?;
            // Update working tree and index but don't update HEAD
            checkout_tree_to_working(&mut repo, &theirs_oid)?;
            write_merge_msg(&repo, &format!(
                "Squashed commit of the following:\n\ncommit {}\n",
                theirs_oid.to_hex()
            ))?;
            return Ok(0);
        }

        writeln!(
            err,
            "Updating {}..{}",
            &head_oid.to_hex()[..7],
            &theirs_oid.to_hex()[..7]
        )?;
        writeln!(err, "Fast-forward")?;

        // Show diffstat
        print_merge_diffstat(repo.odb(), &head_oid, &theirs_oid, &mut err)?;

        // Update HEAD ref
        update_head_to(&repo, &theirs_oid)?;

        // Write reflog entry for HEAD
        {
            let sig = get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", &repo)?;
            let entry = ReflogEntry {
                old_oid: head_oid,
                new_oid: theirs_oid,
                identity: sig,
                message: BString::from(format!("merge {}: Fast-forward", theirs_label)),
            };
            let head_ref = RefName::new(BString::from("HEAD"))?;
            append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
        }

        // Update working tree and index
        checkout_tree_to_working(&mut repo, &theirs_oid)?;

        return Ok(0);
    }

    // Fast-forward not possible
    if args.ff_only {
        writeln!(
            err,
            "fatal: Not possible to fast-forward, aborting."
        )?;
        return Ok(128);
    }

    // Need a real merge. Save ORIG_HEAD for --abort.
    save_orig_head(&repo, &head_oid)?;

    // Run the merge strategy
    let base = base_oid.unwrap_or(ObjectId::NULL_SHA1);
    let options = MergeOptions::default();
    let merge_result = git_merge::strategy::dispatch_merge(
        &mut repo,
        &head_oid,
        &theirs_oid,
        &base,
        &options,
    )?;

    if merge_result.is_clean {
        let tree_oid = merge_result
            .tree
            .ok_or_else(|| anyhow::anyhow!("clean merge produced no tree"))?;

        if args.squash {
            writeln!(err, "Squash commit -- not updating HEAD")?;
            checkout_tree_to_working_from_tree(&mut repo, &tree_oid)?;
            write_merge_msg(&repo, &format!(
                "Squashed commit of the following:\n\ncommit {}\n",
                theirs_oid.to_hex()
            ))?;
            return Ok(0);
        }

        if args.no_commit {
            // Write the tree and index but don't commit
            checkout_tree_to_working_from_tree(&mut repo, &tree_oid)?;
            write_merge_head(&repo, &theirs_oid)?;
            let msg = build_merge_message(args, theirs_label);
            write_merge_msg(&repo, &msg)?;
            writeln!(err, "Automatic merge went well; stopped before committing as requested.")?;
            return Ok(0);
        }

        // Create merge commit
        let msg = build_merge_message(args, theirs_label);
        let commit_oid = create_merge_commit(
            &repo,
            &tree_oid,
            &head_oid,
            &theirs_oid,
            &msg,
        )?;

        // Update HEAD
        update_head_to(&repo, &commit_oid)?;

        // Write reflog entry for HEAD
        {
            let sig = get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", &repo)?;
            let entry = ReflogEntry {
                old_oid: head_oid,
                new_oid: commit_oid,
                identity: sig,
                message: BString::from(format!("merge {}: Merge made by the 'ort' strategy.", theirs_label)),
            };
            let head_ref = RefName::new(BString::from("HEAD"))?;
            append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
        }

        // Update working tree and index
        checkout_tree_to_working_from_tree(&mut repo, &tree_oid)?;

        writeln!(
            err,
            "Merge made by the 'ort' strategy."
        )?;

        // Show diffstat
        print_merge_diffstat(repo.odb(), &head_oid, &commit_oid, &mut err)?;

        return Ok(0);
    }

    // Conflicts detected — emit Auto-merging per file, then CONFLICT lines
    for conflict in &merge_result.conflicts {
        writeln!(
            err,
            "Auto-merging {}",
            conflict.path.to_str_lossy()
        )?;
    }

    // Write conflict entries to the index
    write_conflict_index(&mut repo, &merge_result.conflicts)?;

    // Write conflict markers to working tree files
    write_conflict_files(&repo, &merge_result.conflicts, theirs_label)?;

    // Write MERGE_HEAD and MERGE_MSG for future --continue
    write_merge_head(&repo, &theirs_oid)?;
    let msg = build_merge_message(args, theirs_label);
    write_merge_msg(&repo, &msg)?;

    // Report conflicts
    for conflict in &merge_result.conflicts {
        writeln!(
            err,
            "CONFLICT ({}): Merge conflict in {}",
            conflict_type_label(conflict),
            conflict.path.to_str_lossy()
        )?;
    }
    writeln!(
        err,
        "Automatic merge failed; fix conflicts and then commit the result."
    )?;

    Ok(1)
}

/// Handle `git merge --abort`: reset to ORIG_HEAD.
fn handle_abort(
    repo: &mut git_repository::Repository,
    err: &mut impl Write,
) -> Result<i32> {
    let orig_head_path = repo.git_dir().join("ORIG_HEAD");
    if !orig_head_path.exists() {
        bail!("There is no merge to abort (ORIG_HEAD missing).");
    }

    let orig_head_hex = std::fs::read_to_string(&orig_head_path)?;
    let orig_head = ObjectId::from_hex(orig_head_hex.trim())?;

    // Reset HEAD to ORIG_HEAD
    update_head_to(repo, &orig_head)?;

    // Reset working tree and index
    checkout_tree_to_working(repo, &orig_head)?;

    // Clean up merge state files
    cleanup_merge_state(repo)?;

    writeln!(err, "Merge aborted.")?;
    Ok(0)
}

/// Handle `git merge --continue`: create merge commit after conflict resolution.
fn handle_continue(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    let merge_head_path = repo.git_dir().join("MERGE_HEAD");
    if !merge_head_path.exists() {
        bail!("There is no merge in progress (MERGE_HEAD missing).");
    }

    // Check that there are no remaining conflicts in the index
    {
        let index = repo.index()?;
        let conflicts = index.conflicts();
        if !conflicts.is_empty() {
            writeln!(err, "error: you need to resolve all merge conflicts before continuing.")?;
            writeln!(err, "Unmerged paths:")?;
            for path in &conflicts {
                writeln!(err, "\t{}", path.to_str_lossy())?;
            }
            return Ok(128);
        }
    }

    // Read MERGE_HEAD
    let merge_head_hex = std::fs::read_to_string(&merge_head_path)?;
    let theirs_oid = ObjectId::from_hex(merge_head_hex.trim())?;

    // Get current HEAD
    let head_oid = repo
        .head_oid()?
        .ok_or_else(|| anyhow::anyhow!("HEAD not found"))?;

    // Build tree from current index
    let index_path = repo.git_dir().join("index");
    let index = Index::read_from(&index_path)?;
    let tree_oid = index.write_tree(repo.odb())?;

    // Read merge message
    let merge_msg_path = repo.git_dir().join("MERGE_MSG");
    let msg = if merge_msg_path.exists() {
        std::fs::read_to_string(&merge_msg_path)?
    } else {
        format!("Merge commit '{}'", theirs_oid.to_hex())
    };

    // Create the merge commit
    let commit_oid = create_merge_commit(
        repo,
        &tree_oid,
        &head_oid,
        &theirs_oid,
        &msg,
    )?;

    // Update HEAD
    update_head_to(repo, &commit_oid)?;

    // Clean up merge state files
    cleanup_merge_state(repo)?;

    writeln!(out, "Merge made by the 'ort' strategy.")?;
    Ok(0)
}

/// Build the default merge commit message.
fn build_merge_message(args: &MergeArgs, theirs_label: &str) -> String {
    if let Some(ref msg) = args.message {
        msg.clone()
    } else {
        format!("Merge branch '{}'\n", theirs_label)
    }
}

/// Create a merge commit with two parents and write it to the ODB.
fn create_merge_commit(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    parent1: &ObjectId,
    parent2: &ObjectId,
    message: &str,
) -> Result<ObjectId> {
    let author = get_signature(
        "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL",
        "GIT_AUTHOR_DATE",
        repo,
    )?;
    let committer = get_signature(
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "GIT_COMMITTER_DATE",
        repo,
    )?;

    let commit = Commit {
        tree: *tree_oid,
        parents: vec![*parent1, *parent2],
        author,
        committer,
        encoding: None,
        gpgsig: None,
        extra_headers: Vec::new(),
        message: BString::from(message),
    };

    let obj = Object::Commit(commit);
    let oid = repo.odb().write(&obj)?;
    Ok(oid)
}

/// Update HEAD to point to the given OID.
///
/// If HEAD is a symbolic ref (pointing to a branch), update the branch ref.
/// If HEAD is detached, update HEAD directly.
fn update_head_to(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let head_ref = RefName::new(BString::from("HEAD"))?;
    match repo.refs().resolve(&head_ref)? {
        Some(git_ref::Reference::Symbolic { target, .. }) => {
            // HEAD points to a branch, update the branch
            repo.refs().write_ref(&target, oid)?;
        }
        _ => {
            // Detached HEAD, update HEAD directly
            repo.refs().write_ref(&head_ref, oid)?;
        }
    }
    Ok(())
}

/// Save ORIG_HEAD for merge --abort recovery.
fn save_orig_head(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let path = repo.git_dir().join("ORIG_HEAD");
    std::fs::write(path, format!("{}\n", oid.to_hex()))?;
    Ok(())
}

/// Write MERGE_HEAD file (used during conflict resolution).
fn write_merge_head(repo: &git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let path = repo.git_dir().join("MERGE_HEAD");
    std::fs::write(path, format!("{}\n", oid.to_hex()))?;
    Ok(())
}

/// Write MERGE_MSG file (used for the default merge commit message).
fn write_merge_msg(repo: &git_repository::Repository, msg: &str) -> Result<()> {
    let path = repo.git_dir().join("MERGE_MSG");
    std::fs::write(path, msg)?;
    Ok(())
}

/// Remove merge state files (MERGE_HEAD, MERGE_MSG, ORIG_HEAD).
fn cleanup_merge_state(repo: &git_repository::Repository) -> Result<()> {
    let git_dir = repo.git_dir();
    for name in &["MERGE_HEAD", "MERGE_MSG", "ORIG_HEAD"] {
        let path = git_dir.join(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Checkout a commit's tree into the working tree and rebuild the index.
fn checkout_tree_to_working(
    repo: &mut git_repository::Repository,
    commit_oid: &ObjectId,
) -> Result<()> {
    let obj = repo
        .odb()
        .read(commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit {} not found", commit_oid.to_hex()))?;

    let tree_oid = match obj {
        Object::Commit(c) => c.tree,
        _ => bail!("expected commit, got {}", obj.object_type()),
    };

    checkout_tree_to_working_from_tree(repo, &tree_oid)
}

/// Checkout a tree OID into the working tree and rebuild the index.
fn checkout_tree_to_working_from_tree(
    repo: &mut git_repository::Repository,
    tree_oid: &ObjectId,
) -> Result<()> {
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    let mut index_entries = Vec::new();
    checkout_tree_recursive(
        repo.odb(),
        tree_oid,
        &work_tree,
        &BString::from(""),
        &mut index_entries,
    )?;

    let mut index = Index::new();
    for entry in index_entries {
        index.add(entry);
    }
    let index_path = repo.git_dir().join("index");
    index.write_to(&index_path)?;
    repo.set_index(index);

    Ok(())
}

/// Recursively checkout a tree to the working directory, collecting index entries.
fn checkout_tree_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    work_tree: &Path,
    prefix: &BString,
    entries: &mut Vec<IndexEntry>,
) -> Result<()> {
    let obj = odb
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree {} not found", tree_oid.to_hex()))?;

    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree, got {}", obj.object_type()),
    };

    for entry in tree.iter() {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.push_byte(b'/');
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            let dir_path = work_tree.join(path.to_str_lossy().as_ref());
            std::fs::create_dir_all(&dir_path)?;
            checkout_tree_recursive(odb, &entry.oid, work_tree, &path, entries)?;
        } else {
            let file_path = work_tree.join(path.to_str_lossy().as_ref());
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let blob_obj = odb
                .read(&entry.oid)?
                .ok_or_else(|| anyhow::anyhow!("blob {} not found", entry.oid.to_hex()))?;

            let data = match blob_obj {
                Object::Blob(b) => b.data,
                _ => bail!("expected blob for {}", path.to_str_lossy()),
            };

            std::fs::write(&file_path, &data)?;

            #[cfg(unix)]
            if entry.mode == FileMode::Executable {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&file_path, perms)?;
            }

            if entry.mode == FileMode::Symlink {
                std::fs::remove_file(&file_path)?;
                #[cfg(unix)]
                {
                    let target = String::from_utf8_lossy(&data);
                    std::os::unix::fs::symlink(target.as_ref(), &file_path)?;
                }
            }

            let metadata = std::fs::symlink_metadata(&file_path)?;
            entries.push(IndexEntry {
                path,
                oid: entry.oid,
                mode: entry.mode,
                stage: Stage::Normal,
                stat: StatData::from_metadata(&metadata),
                flags: EntryFlags::default(),
            });
        }
    }

    Ok(())
}

/// Write conflict entries from MergeResult into the index.
fn write_conflict_index(
    repo: &mut git_repository::Repository,
    conflicts: &[ConflictEntry],
) -> Result<()> {
    let index = repo.index_mut()?;

    for conflict in conflicts {
        let path_bstr = &conflict.path;

        // Remove any existing stage-0 entry for this path
        index.remove(path_bstr.as_ref(), Stage::Normal);

        // Write stage 1 (base)
        if let Some(ref side) = conflict.base {
            index.add(IndexEntry {
                path: path_bstr.clone(),
                oid: side.oid,
                mode: side.mode,
                stage: Stage::Base,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            });
        }

        // Write stage 2 (ours)
        if let Some(ref side) = conflict.ours {
            index.add(IndexEntry {
                path: path_bstr.clone(),
                oid: side.oid,
                mode: side.mode,
                stage: Stage::Ours,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            });
        }

        // Write stage 3 (theirs)
        if let Some(ref side) = conflict.theirs {
            index.add(IndexEntry {
                path: path_bstr.clone(),
                oid: side.oid,
                mode: side.mode,
                stage: Stage::Theirs,
                stat: StatData::default(),
                flags: EntryFlags::default(),
            });
        }
    }

    repo.write_index()?;
    Ok(())
}

/// Write conflict markers to working tree files for content conflicts.
fn write_conflict_files(
    repo: &git_repository::Repository,
    conflicts: &[ConflictEntry],
    theirs_label: &str,
) -> Result<()> {
    let work_tree = match repo.work_tree() {
        Some(wt) => wt.to_path_buf(),
        None => return Ok(()),
    };
    let odb = repo.odb();

    for conflict in conflicts {
        if conflict.conflict_type != git_merge::ConflictType::Content {
            continue;
        }

        let path = work_tree.join(conflict.path.to_str_lossy().as_ref());

        // Read ours/theirs/base content
        let ours_content = conflict.ours.as_ref()
            .and_then(|s| odb.read(&s.oid).ok().flatten())
            .map(|obj| match obj {
                Object::Blob(b) => b.data.to_vec(),
                _ => Vec::new(),
            })
            .unwrap_or_default();

        let theirs_content = conflict.theirs.as_ref()
            .and_then(|s| odb.read(&s.oid).ok().flatten())
            .map(|obj| match obj {
                Object::Blob(b) => b.data.to_vec(),
                _ => Vec::new(),
            })
            .unwrap_or_default();

        let base_content = conflict.base.as_ref()
            .and_then(|s| odb.read(&s.oid).ok().flatten())
            .map(|obj| match obj {
                Object::Blob(b) => b.data.to_vec(),
                _ => Vec::new(),
            })
            .unwrap_or_default();

        // Generate conflict markers using line-level merge
        let merged = merge_with_markers(
            &base_content,
            &ours_content,
            &theirs_content,
            theirs_label,
        );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, merged)?;
    }

    Ok(())
}

/// Perform a simple line-level three-way merge with conflict markers.
fn merge_with_markers(
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    theirs_label: &str,
) -> Vec<u8> {
    let base_lines: Vec<&str> = std::str::from_utf8(base)
        .unwrap_or("")
        .lines()
        .collect();
    let ours_lines: Vec<&str> = std::str::from_utf8(ours)
        .unwrap_or("")
        .lines()
        .collect();
    let theirs_lines: Vec<&str> = std::str::from_utf8(theirs)
        .unwrap_or("")
        .lines()
        .collect();

    let mut result = String::new();
    let max_len = base_lines.len().max(ours_lines.len()).max(theirs_lines.len());

    let mut in_conflict = false;
    let mut ours_block: Vec<&str> = Vec::new();
    let mut theirs_block: Vec<&str> = Vec::new();

    for i in 0..max_len {
        let base_line = base_lines.get(i).copied().unwrap_or("");
        let ours_line = ours_lines.get(i).copied().unwrap_or("");
        let theirs_line = theirs_lines.get(i).copied().unwrap_or("");

        if ours_line == theirs_line {
            // Both sides agree
            if in_conflict {
                // Flush conflict block
                result.push_str("<<<<<<< HEAD\n");
                for l in &ours_block {
                    result.push_str(l);
                    result.push('\n');
                }
                result.push_str("=======\n");
                for l in &theirs_block {
                    result.push_str(l);
                    result.push('\n');
                }
                result.push_str(&format!(">>>>>>> {}\n", theirs_label));
                ours_block.clear();
                theirs_block.clear();
                in_conflict = false;
            }
            result.push_str(ours_line);
            result.push('\n');
        } else if ours_line == base_line {
            // Only theirs changed
            if in_conflict {
                ours_block.push(ours_line);
                theirs_block.push(theirs_line);
            } else {
                result.push_str(theirs_line);
                result.push('\n');
            }
        } else if theirs_line == base_line {
            // Only ours changed
            if in_conflict {
                ours_block.push(ours_line);
                theirs_block.push(theirs_line);
            } else {
                result.push_str(ours_line);
                result.push('\n');
            }
        } else {
            // Both sides changed — conflict
            if !in_conflict {
                in_conflict = true;
            }
            ours_block.push(ours_line);
            theirs_block.push(theirs_line);
        }
    }

    // Flush any remaining conflict
    if in_conflict {
        result.push_str("<<<<<<< HEAD\n");
        for l in &ours_block {
            result.push_str(l);
            result.push('\n');
        }
        result.push_str("=======\n");
        for l in &theirs_block {
            result.push_str(l);
            result.push('\n');
        }
        result.push_str(&format!(">>>>>>> {}\n", theirs_label));
    }

    result.into_bytes()
}

/// Print diffstat summary between two commits.
fn print_merge_diffstat(
    odb: &git_odb::ObjectDatabase,
    from_oid: &ObjectId,
    to_oid: &ObjectId,
    out: &mut impl Write,
) -> Result<()> {
    let from_tree = odb.read(from_oid)?.and_then(|o| match o {
        Object::Commit(c) => Some(c.tree),
        _ => None,
    });
    let to_tree = odb.read(to_oid)?.and_then(|o| match o {
        Object::Commit(c) => Some(c.tree),
        _ => None,
    });
    let diff_opts = git_diff::DiffOptions {
        output_format: git_diff::DiffOutputFormat::Stat,
        ..git_diff::DiffOptions::default()
    };
    if let Ok(result) = git_diff::tree::diff_trees(odb, from_tree.as_ref(), to_tree.as_ref(), &diff_opts) {
        if !result.is_empty() {
            let output = git_diff::format::format_diff(&result, &diff_opts);
            write!(out, "{}", output)?;
        }
    }
    Ok(())
}

/// Human-readable label for a conflict type.
fn conflict_type_label(conflict: &ConflictEntry) -> &'static str {
    match conflict.conflict_type {
        git_merge::ConflictType::Content => "content",
        git_merge::ConflictType::ModifyDelete => "modify/delete",
        git_merge::ConflictType::AddAdd => "add/add",
        git_merge::ConflictType::RenameRename => "rename/rename",
        git_merge::ConflictType::RenameDelete => "rename/delete",
        git_merge::ConflictType::DirectoryFile => "directory/file",
    }
}

/// Build a signature from environment variables or repository config.
fn get_signature(
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

use std::io::{self, Write};
use std::path::Path;

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice, ByteVec};
use clap::Args;
use git_hash::ObjectId;
use git_index::{EntryFlags, Index, IndexEntry, Stage, StatData};
use git_merge::{ConflictStyle, MergeOptions, MergeStrategyType, ConflictEntry};
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

    /// Merge strategy to use
    #[arg(short = 's', long = "strategy")]
    pub strategy: Option<String>,

    /// Pass option to the merge strategy
    #[arg(short = 'X', long = "strategy-option")]
    pub strategy_option: Vec<String>,

    /// Be verbose
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Be quiet
    #[arg(short, long)]
    pub quiet: bool,

    /// Show diffstat at end of merge
    #[arg(long)]
    pub stat: bool,

    /// Do not show diffstat at end of merge
    #[arg(long)]
    pub no_stat: bool,

    /// Open an editor for the merge message
    #[arg(short = 'e', long)]
    pub edit: bool,

    /// Allow merging unrelated histories
    #[arg(long)]
    pub allow_unrelated_histories: bool,

    /// Add Signed-off-by trailer
    #[arg(long)]
    pub signoff: bool,

    /// Run pre-merge and commit-msg hooks
    #[arg(long)]
    pub verify: bool,

    /// Bypass pre-merge and commit-msg hooks
    #[arg(long)]
    pub no_verify: bool,

    /// Branch(es) or commit(s) to merge
    #[arg(required_unless_present_any = ["abort", "continue"])]
    pub commit: Vec<String>,
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

    // Resolve all merge targets
    if args.commit.is_empty() {
        bail!("no commit specified to merge");
    }

    let mut theirs_oids = Vec::new();
    for spec in &args.commit {
        theirs_oids.push(resolve_revision(&repo, spec)?);
    }

    // Multi-head merge path (octopus)
    if theirs_oids.len() >= 2 {
        return run_octopus_merge(args, &mut repo, &head_oid, &theirs_oids, &mut out, &mut err);
    }

    // Single-commit merge path
    let theirs_oid = theirs_oids[0];

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
    let theirs_label = &args.commit[0];

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
    let options = build_merge_options(args, &repo)?;
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
            write_merge_head(&repo, &[theirs_oid])?;
            let msg = build_merge_message(args, &[theirs_label.as_str()]);
            write_merge_msg(&repo, &msg)?;
            writeln!(err, "Automatic merge went well; stopped before committing as requested.")?;
            return Ok(0);
        }

        // Create merge commit
        let msg = build_merge_message(args, &[theirs_label.as_str()]);
        let parents = vec![head_oid, theirs_oid];
        let commit_oid = create_merge_commit(
            &repo,
            &tree_oid,
            &parents,
            &msg,
        )?;

        // Update HEAD
        update_head_to(&repo, &commit_oid)?;

        // Write reflog entry for HEAD
        {
            let strategy_name = options.strategy.name();
            let sig = get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", &repo)?;
            let entry = ReflogEntry {
                old_oid: head_oid,
                new_oid: commit_oid,
                identity: sig,
                message: BString::from(format!("merge {}: Merge made by the '{}' strategy.", theirs_label, strategy_name)),
            };
            let head_ref = RefName::new(BString::from("HEAD"))?;
            append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
        }

        // Update working tree and index
        checkout_tree_to_working_from_tree(&mut repo, &tree_oid)?;

        writeln!(
            err,
            "Merge made by the '{}' strategy.",
            options.strategy.name()
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
    write_merge_head(&repo, &[theirs_oid])?;
    let msg = build_merge_message(args, &[theirs_label.as_str()]);
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

/// Handle octopus merge (2+ additional heads).
fn run_octopus_merge(
    args: &MergeArgs,
    repo: &mut git_repository::Repository,
    head_oid: &ObjectId,
    theirs_oids: &[ObjectId],
    _out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    if args.ff_only {
        writeln!(err, "fatal: Not possible to fast-forward, aborting.")?;
        return Ok(128);
    }

    save_orig_head(repo, head_oid)?;

    let mut options = build_merge_options(args, repo)?;
    // Force octopus strategy for multi-head merges
    options.strategy = MergeStrategyType::Octopus;

    // Like git, if the first branch can be fast-forwarded (i.e., HEAD is an
    // ancestor of branch1), fast-forward to it first. This avoids an extra
    // parent in the resulting commit.
    let mut current_oid = *head_oid;
    let mut merge_parents: Vec<ObjectId> = Vec::new();
    let mut remaining_heads: Vec<ObjectId> = Vec::new();

    for (i, theirs) in theirs_oids.iter().enumerate() {
        if i == 0 {
            let base = merge_base_one(repo, &current_oid, theirs)?;
            let can_ff = match base {
                Some(ref b) => *b == current_oid,
                None => false,
            };
            if can_ff {
                // Fast-forward to first branch — it becomes the first parent
                current_oid = *theirs;
                merge_parents.push(*theirs);
            } else {
                merge_parents.push(current_oid);
                remaining_heads.push(*theirs);
            }
        } else {
            remaining_heads.push(*theirs);
        }
    }

    if remaining_heads.is_empty() {
        // All branches were fast-forwarded (shouldn't happen with 2+ heads, but handle it)
        let tree_oid = {
            let obj = repo.odb().read(&current_oid)?
                .ok_or_else(|| anyhow::anyhow!("commit not found"))?;
            match obj {
                Object::Commit(c) => c.tree,
                _ => bail!("expected commit"),
            }
        };
        checkout_tree_to_working_from_tree(repo, &tree_oid)?;
        update_head_to(repo, &current_oid)?;
        return Ok(0);
    }

    // Compute merge bases for remaining heads
    let mut bases = Vec::new();
    for theirs in &remaining_heads {
        match merge_base_one(repo, &current_oid, theirs)? {
            Some(b) => bases.push(b),
            None => bases.push(ObjectId::NULL_SHA1),
        }
    }

    let octopus = git_merge::strategy::octopus::OctopusStrategy;
    let merge_result = match octopus.merge_multi(repo, &current_oid, &remaining_heads, &bases, &options) {
        Ok(r) => r,
        Err(e) => {
            writeln!(err, "Merge with strategy octopus failed.")?;
            writeln!(err, "{}", e)?;
            return Ok(2);
        }
    };

    let tree_oid = merge_result
        .tree
        .ok_or_else(|| anyhow::anyhow!("octopus merge produced no tree"))?;

    // Build merge commit message
    let labels: Vec<&str> = args.commit.iter().map(|s| s.as_str()).collect();
    let msg = build_merge_message(args, &labels);

    // Create merge commit: parents = [first_parent] + remaining heads
    // If we fast-forwarded, first_parent is the ff'd branch; otherwise it's the original HEAD
    for h in &remaining_heads {
        merge_parents.push(*h);
    }
    let commit_oid = create_merge_commit(repo, &tree_oid, &merge_parents, &msg)?;

    // Update HEAD
    update_head_to(repo, &commit_oid)?;

    // Write reflog
    {
        let sig = get_signature("GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE", repo)?;
        let entry = ReflogEntry {
            old_oid: *head_oid,
            new_oid: commit_oid,
            identity: sig,
            message: BString::from(format!("merge {}: Merge made by the 'octopus' strategy.", labels.join(", "))),
        };
        let head_ref = RefName::new(BString::from("HEAD"))?;
        append_reflog_entry(repo.git_dir(), &head_ref, &entry)?;
    }

    // Update working tree and index
    checkout_tree_to_working_from_tree(repo, &tree_oid)?;

    writeln!(err, "Merge made by the 'octopus' strategy.")?;

    Ok(0)
}

/// Build MergeOptions from CLI args and repository config.
///
/// Strategy selection priority: `--strategy` flag > `merge.strategy` config > default (ort).
/// Conflict style priority: `merge.conflictStyle` config > default (merge).
/// Strategy options: `-X` flags are passed through directly.
fn build_merge_options(args: &MergeArgs, repo: &git_repository::Repository) -> Result<MergeOptions> {
    let mut options = MergeOptions::default();

    // Resolve merge strategy: CLI flag takes precedence over config.
    if let Some(ref strategy_name) = args.strategy {
        match MergeStrategyType::from_name(strategy_name) {
            Some(st) => options.strategy = st,
            None => bail!(
                "Could not find merge strategy '{}'.\nAvailable strategies are: ort, recursive, ours, subtree, octopus.",
                strategy_name
            ),
        }
    } else if let Some(config_strategy) = repo.config().get_string("merge.strategy")?.as_deref() {
        if let Some(st) = MergeStrategyType::from_name(config_strategy) {
            options.strategy = st;
        }
    }

    // Pass through -X / --strategy-option values.
    options.strategy_options = args.strategy_option.clone();

    // Read merge.conflictStyle from config.
    if let Some(style_name) = repo.config().get_string("merge.conflictStyle")?.as_deref() {
        if let Some(style) = ConflictStyle::from_name(style_name) {
            options.conflict_style = style;
        }
    }

    // Allow unrelated histories flag.
    options.allow_unrelated_histories = args.allow_unrelated_histories;

    Ok(options)
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

    // Read MERGE_HEAD (may contain multiple OIDs, one per line)
    let merge_head_content = std::fs::read_to_string(&merge_head_path)?;
    let mut theirs_oids = Vec::new();
    for line in merge_head_content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            theirs_oids.push(ObjectId::from_hex(trimmed)?);
        }
    }
    if theirs_oids.is_empty() {
        bail!("MERGE_HEAD is empty");
    }

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
        format!("Merge commit '{}'", theirs_oids[0].to_hex())
    };

    // Create the merge commit
    let mut parents = vec![head_oid];
    parents.extend_from_slice(&theirs_oids);
    let commit_oid = create_merge_commit(
        repo,
        &tree_oid,
        &parents,
        &msg,
    )?;

    // Update HEAD
    update_head_to(repo, &commit_oid)?;

    // Clean up merge state files
    cleanup_merge_state(repo)?;

    // Determine strategy name from config for the status message.
    let strategy_name = repo.config()
        .get_string("merge.strategy")
        .ok()
        .flatten()
        .and_then(|s| MergeStrategyType::from_name(&s))
        .unwrap_or(MergeStrategyType::Ort)
        .name();
    writeln!(out, "Merge made by the '{}' strategy.", strategy_name)?;
    Ok(0)
}

/// Build the default merge commit message.
fn build_merge_message(args: &MergeArgs, theirs_labels: &[&str]) -> String {
    if let Some(ref msg) = args.message {
        msg.clone()
    } else if theirs_labels.len() == 1 {
        format!("Merge branch '{}'\n", theirs_labels[0])
    } else {
        let quoted: Vec<String> = theirs_labels.iter().map(|l| format!("'{}'", l)).collect();
        format!("Merge branches {}\n", quoted.join(", "))
    }
}

/// Create a merge commit with N parents and write it to the ODB.
fn create_merge_commit(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    parents: &[ObjectId],
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
        parents: parents.to_vec(),
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
fn write_merge_head(repo: &git_repository::Repository, oids: &[ObjectId]) -> Result<()> {
    let path = repo.git_dir().join("MERGE_HEAD");
    let content: String = oids.iter().map(|o| format!("{}\n", o.to_hex())).collect();
    std::fs::write(path, content)?;
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

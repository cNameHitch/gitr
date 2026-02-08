use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat};
use git_object::Object;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DiffArgs {
    /// Show staged changes (index vs HEAD)
    #[arg(long)]
    cached: bool,

    /// Alias for --cached
    #[arg(long)]
    staged: bool,

    /// Show diffstat instead of patch
    #[arg(long)]
    stat: bool,

    /// Show short stat
    #[arg(long)]
    shortstat: bool,

    /// Show numstat
    #[arg(long)]
    numstat: bool,

    /// Show name-only
    #[arg(long)]
    name_only: bool,

    /// Show name-status
    #[arg(long)]
    name_status: bool,

    /// Show summary
    #[arg(long)]
    summary: bool,

    /// Show raw diff output
    #[arg(long)]
    raw: bool,

    /// Don't show diff, just check for changes
    #[arg(long)]
    quiet: bool,

    /// Generate diff in unified format with <n> lines of context
    #[arg(short = 'U', long = "unified")]
    context_lines: Option<u32>,

    /// Detect renames
    #[arg(short = 'M', long)]
    find_renames: bool,

    /// Detect copies
    #[arg(long)]
    find_copies: bool,

    /// Commits or paths to diff
    #[arg(value_name = "commit-or-path")]
    args: Vec<String>,
}

pub fn run(args: &DiffArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut diff_opts = DiffOptions {
        output_format: determine_output_format(args),
        detect_renames: args.find_renames,
        detect_copies: args.find_copies,
        ..DiffOptions::default()
    };
    if let Some(ctx) = args.context_lines {
        diff_opts.context_lines = ctx;
    }

    let is_cached = args.cached || args.staged;

    // Parse arguments: figure out if we have commits, pathspecs, or both
    let (commits, _pathspecs) = parse_diff_args(&args.args);

    let result = if commits.len() == 2 {
        // git diff <commit1> <commit2>
        let oid_a = git_revwalk::resolve_revision(&repo, &commits[0])?;
        let oid_b = git_revwalk::resolve_revision(&repo, &commits[1])?;

        let tree_a = get_commit_tree(&repo, &oid_a)?;
        let tree_b = get_commit_tree(&repo, &oid_b)?;

        git_diff::tree::diff_trees(repo.odb(), Some(&tree_a), Some(&tree_b), &diff_opts)?
    } else if commits.len() == 1 && commits[0].contains("..") {
        // git diff A..B
        let parts: Vec<&str> = commits[0].split("..").collect();
        let oid_a = git_revwalk::resolve_revision(&repo, parts[0])?;
        let oid_b = git_revwalk::resolve_revision(&repo, parts[1])?;

        let tree_a = get_commit_tree(&repo, &oid_a)?;
        let tree_b = get_commit_tree(&repo, &oid_b)?;

        git_diff::tree::diff_trees(repo.odb(), Some(&tree_a), Some(&tree_b), &diff_opts)?
    } else if commits.len() == 1 {
        // git diff <commit> — diff commit against working tree (or index if --cached)
        let oid = git_revwalk::resolve_revision(&repo, &commits[0])?;
        let tree = get_commit_tree(&repo, &oid)?;

        if is_cached {
            // Compare commit tree vs index
            let index_path = repo.git_dir().join("index");
            let index = if index_path.exists() {
                git_index::Index::read_from(&index_path)?
            } else {
                git_index::Index::new()
            };
            let index_tree = index.write_tree(repo.odb())?;
            git_diff::tree::diff_trees(repo.odb(), Some(&tree), Some(&index_tree), &diff_opts)?
        } else {
            // Compare commit tree against worktree:
            // First get HEAD-vs-index diff and index-vs-worktree diff, combine them.
            // Simpler approach: get index tree and worktree changes, show combined.
            // Actually for `diff HEAD`, we need: HEAD tree -> worktree content.
            // This equals: (HEAD -> index) + (index -> worktree) merged.
            let staged = git_diff::worktree::diff_head_to_index(&mut repo, &diff_opts)?;
            let unstaged = git_diff::worktree::diff_index_to_worktree(&mut repo, &diff_opts)?;
            // Merge: include all staged files and all unstaged files
            let mut files_map: std::collections::BTreeMap<bstr::BString, git_diff::FileDiff> = std::collections::BTreeMap::new();
            for file in staged.files {
                files_map.insert(file.path().clone(), file);
            }
            for file in unstaged.files {
                let path = file.path().clone();
                match files_map.entry(path) {
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        // File appears in both staged and unstaged — re-diff HEAD vs worktree
                        // Read the HEAD blob and worktree content
                        let commit_tree = tree;
                        let key = entry.key().clone();
                        if let Ok(old_data) = resolve_blob_from_tree(&repo, &commit_tree, &key) {
                            let work_tree = repo.work_tree().unwrap().to_path_buf();
                            let fs_path = work_tree.join(key.to_str_lossy().as_ref());
                            if let Ok(new_data) = std::fs::read(&fs_path) {
                                let binary = git_diff::binary::is_binary(&old_data) || git_diff::binary::is_binary(&new_data);
                                let hunks = if binary {
                                    Vec::new()
                                } else {
                                    git_diff::algorithm::diff_lines(&old_data, &new_data, diff_opts.algorithm, diff_opts.context_lines)
                                };
                                let old_oid = git_hash::hasher::Hasher::hash_object(git_hash::HashAlgorithm::Sha1, "blob", &old_data).ok();
                                let new_oid = git_hash::hasher::Hasher::hash_object(git_hash::HashAlgorithm::Sha1, "blob", &new_data).ok();
                                entry.insert(git_diff::FileDiff {
                                    status: git_diff::FileStatus::Modified,
                                    old_path: Some(file.path().clone()),
                                    new_path: Some(file.path().clone()),
                                    old_mode: file.old_mode,
                                    new_mode: file.new_mode,
                                    old_oid,
                                    new_oid,
                                    hunks,
                                    is_binary: binary,
                                    similarity: None,
                                });
                            }
                        }
                    }
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(file);
                    }
                }
            }
            git_diff::DiffResult { files: files_map.into_values().collect() }
        }
    } else if is_cached {
        // git diff --cached — staged changes (index vs HEAD)
        git_diff::worktree::diff_head_to_index(&mut repo, &diff_opts)?
    } else {
        // git diff — unstaged changes (worktree vs index)
        git_diff::worktree::diff_index_to_worktree(&mut repo, &diff_opts)?
    };

    if args.quiet {
        return Ok(if result.is_empty() { 0 } else { 1 });
    }

    if !result.is_empty() {
        let output = format_diff(&result, &diff_opts);
        write!(out, "{}", output)?;
    }

    Ok(0)
}

fn determine_output_format(args: &DiffArgs) -> DiffOutputFormat {
    if args.stat {
        DiffOutputFormat::Stat
    } else if args.shortstat {
        DiffOutputFormat::ShortStat
    } else if args.numstat {
        DiffOutputFormat::NumStat
    } else if args.name_only {
        DiffOutputFormat::NameOnly
    } else if args.name_status {
        DiffOutputFormat::NameStatus
    } else if args.summary {
        DiffOutputFormat::Summary
    } else if args.raw {
        DiffOutputFormat::Raw
    } else {
        DiffOutputFormat::Unified
    }
}

fn parse_diff_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    let mut commits = Vec::new();
    let mut pathspecs = Vec::new();
    let mut saw_separator = false;

    for arg in args {
        if arg == "--" {
            saw_separator = true;
            continue;
        }
        if saw_separator {
            pathspecs.push(arg.clone());
        } else {
            commits.push(arg.clone());
        }
    }

    (commits, pathspecs)
}

fn get_commit_tree(
    repo: &git_repository::Repository,
    oid: &git_hash::ObjectId,
) -> Result<git_hash::ObjectId> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;
    match obj {
        Object::Commit(c) => Ok(c.tree),
        _ => anyhow::bail!("not a commit: {}", oid),
    }
}

fn resolve_blob_from_tree(
    repo: &git_repository::Repository,
    tree_oid: &git_hash::ObjectId,
    path: &bstr::BString,
) -> Result<Vec<u8>> {
    let path_str = path.to_str_lossy();
    let components: Vec<&str> = path_str.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = *tree_oid;
    for component in &components {
        let obj = repo.odb().read(&current)?
            .ok_or_else(|| anyhow::anyhow!("tree not found"))?;
        let tree = match obj {
            Object::Tree(t) => t,
            _ => anyhow::bail!("not a tree"),
        };
        let entry = tree.entries.iter()
            .find(|e| e.name.as_bstr() == component.as_bytes().as_bstr())
            .ok_or_else(|| anyhow::anyhow!("path not found"))?;
        current = entry.oid;
    }
    let obj = repo.odb().read(&current)?
        .ok_or_else(|| anyhow::anyhow!("blob not found"))?;
    match obj {
        Object::Blob(b) => Ok(b.data.to_vec()),
        _ => anyhow::bail!("not a blob"),
    }
}

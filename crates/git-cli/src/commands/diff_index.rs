use std::io::{self, Write};

use anyhow::Result;
use bstr::{BString, ByteSlice};
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat, DiffResult, FileDiff, FileStatus};
use git_hash::ObjectId;
use git_index::Stage;
use git_object::{FileMode, Object};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DiffIndexArgs {
    /// Compare tree to index instead of working tree
    #[arg(long)]
    cached: bool,

    /// Generate patch output
    #[arg(short = 'p', long = "patch")]
    patch: bool,

    /// Show raw diff output format
    #[arg(long)]
    raw: bool,

    /// Show only names of changed files
    #[arg(long)]
    name_only: bool,

    /// Show names and status of changed files
    #[arg(long)]
    name_status: bool,

    /// Tree-ish to compare against
    #[arg(value_name = "tree-ish")]
    tree_ish: String,

    /// Paths to limit diff to
    #[arg(value_name = "path")]
    pathspecs: Vec<String>,
}

pub fn run(args: &DiffIndexArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Resolve tree-ish to a tree OID
    let commit_oid = git_revwalk::resolve_revision(&repo, &args.tree_ish)?;
    let tree_oid = get_commit_tree(&repo, &commit_oid)?;

    // Read the tree entries
    let tree_entries = read_tree_entries_recursive(repo.odb(), &tree_oid)?;

    if args.cached {
        // Compare tree against index
        let index_entries: Vec<(BString, ObjectId, FileMode)> = {
            let index = repo.index()?;
            index
                .iter()
                .filter(|e| e.stage == Stage::Normal)
                .filter(|e| matches_pathspecs(&e.path, &args.pathspecs))
                .map(|e| (e.path.clone(), e.oid, e.mode))
                .collect()
        };

        let files = diff_tree_vs_index(&tree_entries, &index_entries);
        let has_changes = !files.is_empty();
        let result = DiffResult { files };

        if has_changes {
            let diff_opts = DiffOptions {
                output_format: determine_output_format(args),
                ..DiffOptions::default()
            };

            // For non-raw formats that need hunks, recompute with content
            let result = if needs_content(&diff_opts) {
                recompute_with_hunks(&result, repo.odb(), &diff_opts)
            } else {
                result
            };

            let output = format_diff(&result, &diff_opts);
            write!(out, "{}", output)?;
        }

        Ok(if has_changes { 1 } else { 0 })
    } else {
        // Compare tree against working tree
        let work_tree = repo
            .work_tree()
            .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
            .to_path_buf();

        // Read index for mode/oid info, then compare tree entries against working tree
        let index_entries: Vec<(BString, ObjectId, FileMode)> = {
            let index = repo.index()?;
            index
                .iter()
                .filter(|e| e.stage == Stage::Normal)
                .map(|e| (e.path.clone(), e.oid, e.mode))
                .collect()
        };

        let odb = repo.odb();
        let mut files: Vec<FileDiff> = Vec::new();

        // Build a map of tree entries for lookup
        let mut tree_map: std::collections::BTreeMap<BString, (ObjectId, FileMode)> =
            std::collections::BTreeMap::new();
        for (path, oid, mode) in &tree_entries {
            tree_map.insert(path.clone(), (*oid, *mode));
        }

        // Build a set of index/worktree paths
        let mut index_map: std::collections::BTreeMap<BString, (ObjectId, FileMode)> =
            std::collections::BTreeMap::new();
        for (path, oid, mode) in &index_entries {
            index_map.insert(path.clone(), (*oid, *mode));
        }

        // Collect all paths from both tree and index
        let mut all_paths: std::collections::BTreeSet<BString> = std::collections::BTreeSet::new();
        for path in tree_map.keys() {
            all_paths.insert(path.clone());
        }
        for path in index_map.keys() {
            all_paths.insert(path.clone());
        }

        for path in &all_paths {
            if !matches_pathspecs(path, &args.pathspecs) {
                continue;
            }

            let tree_entry = tree_map.get(path);
            let fs_path = work_tree.join(path.to_str_lossy().as_ref());

            match tree_entry {
                Some((tree_oid_entry, tree_mode)) => {
                    if !fs_path.exists() {
                        // File in tree but not in working tree -> deleted
                        files.push(FileDiff {
                            status: FileStatus::Deleted,
                            old_path: Some(path.clone()),
                            new_path: None,
                            old_mode: Some(*tree_mode),
                            new_mode: None,
                            old_oid: Some(*tree_oid_entry),
                            new_oid: None,
                            hunks: Vec::new(),
                            is_binary: false,
                            similarity: None,
                        });
                    } else {
                        let worktree_content = std::fs::read(&fs_path)?;
                        let wt_oid = hash_blob(&worktree_content);
                        let wt_mode =
                            file_mode_from_metadata(&std::fs::symlink_metadata(&fs_path)?);

                        if wt_oid.as_ref() == Some(tree_oid_entry) && wt_mode == *tree_mode {
                            continue; // No change
                        }

                        let old_data = read_blob_data(odb, tree_oid_entry).unwrap_or_default();
                        let binary = git_diff::binary::is_binary(&old_data)
                            || git_diff::binary::is_binary(&worktree_content);
                        let hunks = if binary {
                            Vec::new()
                        } else {
                            git_diff::algorithm::diff_lines(
                                &old_data,
                                &worktree_content,
                                git_diff::DiffAlgorithm::Myers,
                                3,
                            )
                        };

                        files.push(FileDiff {
                            status: FileStatus::Modified,
                            old_path: Some(path.clone()),
                            new_path: Some(path.clone()),
                            old_mode: Some(*tree_mode),
                            new_mode: Some(wt_mode),
                            old_oid: Some(*tree_oid_entry),
                            new_oid: wt_oid,
                            hunks,
                            is_binary: binary,
                            similarity: None,
                        });
                    }
                }
                None => {
                    // File not in tree but exists in working tree -> added
                    if fs_path.exists() {
                        let worktree_content = std::fs::read(&fs_path)?;
                        let wt_oid = hash_blob(&worktree_content);
                        let wt_mode =
                            file_mode_from_metadata(&std::fs::symlink_metadata(&fs_path)?);
                        let binary = git_diff::binary::is_binary(&worktree_content);
                        let hunks = if binary {
                            Vec::new()
                        } else {
                            git_diff::algorithm::diff_lines(
                                &[],
                                &worktree_content,
                                git_diff::DiffAlgorithm::Myers,
                                3,
                            )
                        };

                        files.push(FileDiff {
                            status: FileStatus::Added,
                            old_path: None,
                            new_path: Some(path.clone()),
                            old_mode: None,
                            new_mode: Some(wt_mode),
                            old_oid: None,
                            new_oid: wt_oid,
                            hunks,
                            is_binary: binary,
                            similarity: None,
                        });
                    }
                }
            }
        }

        let has_changes = !files.is_empty();
        let result = DiffResult { files };

        if has_changes {
            let diff_opts = DiffOptions {
                output_format: determine_output_format(args),
                ..DiffOptions::default()
            };
            let output = format_diff(&result, &diff_opts);
            write!(out, "{}", output)?;
        }

        Ok(if has_changes { 1 } else { 0 })
    }
}

/// Compare tree entries against index entries, returning FileDiff for each difference.
fn diff_tree_vs_index(
    tree_entries: &[(BString, ObjectId, FileMode)],
    index_entries: &[(BString, ObjectId, FileMode)],
) -> Vec<FileDiff> {
    let mut tree_map: std::collections::BTreeMap<&BString, (&ObjectId, &FileMode)> =
        std::collections::BTreeMap::new();
    for (path, oid, mode) in tree_entries {
        tree_map.insert(path, (oid, mode));
    }

    let mut index_map: std::collections::BTreeMap<&BString, (&ObjectId, &FileMode)> =
        std::collections::BTreeMap::new();
    for (path, oid, mode) in index_entries {
        index_map.insert(path, (oid, mode));
    }

    let mut files = Vec::new();

    // Collect all paths
    let mut all_paths: std::collections::BTreeSet<&BString> = std::collections::BTreeSet::new();
    for path in tree_map.keys() {
        all_paths.insert(path);
    }
    for path in index_map.keys() {
        all_paths.insert(path);
    }

    for path in all_paths {
        let tree_entry = tree_map.get(path);
        let index_entry = index_map.get(path);

        match (tree_entry, index_entry) {
            (Some((tree_oid, tree_mode)), Some((idx_oid, idx_mode))) => {
                if tree_oid != idx_oid || tree_mode != idx_mode {
                    files.push(FileDiff {
                        status: FileStatus::Modified,
                        old_path: Some(path.clone()),
                        new_path: Some(path.clone()),
                        old_mode: Some(**tree_mode),
                        new_mode: Some(**idx_mode),
                        old_oid: Some(**tree_oid),
                        new_oid: Some(**idx_oid),
                        hunks: Vec::new(),
                        is_binary: false,
                        similarity: None,
                    });
                }
            }
            (Some((tree_oid, tree_mode)), None) => {
                // In tree but not in index -> deleted
                files.push(FileDiff {
                    status: FileStatus::Deleted,
                    old_path: Some(path.clone()),
                    new_path: None,
                    old_mode: Some(**tree_mode),
                    new_mode: None,
                    old_oid: Some(**tree_oid),
                    new_oid: None,
                    hunks: Vec::new(),
                    is_binary: false,
                    similarity: None,
                });
            }
            (None, Some((idx_oid, idx_mode))) => {
                // In index but not in tree -> added
                files.push(FileDiff {
                    status: FileStatus::Added,
                    old_path: None,
                    new_path: Some(path.clone()),
                    old_mode: None,
                    new_mode: Some(**idx_mode),
                    old_oid: None,
                    new_oid: Some(**idx_oid),
                    hunks: Vec::new(),
                    is_binary: false,
                    similarity: None,
                });
            }
            (None, None) => unreachable!(),
        }
    }

    files
}

/// Recompute FileDiff entries with actual content hunks for unified output.
fn recompute_with_hunks(
    result: &DiffResult,
    odb: &git_odb::ObjectDatabase,
    opts: &DiffOptions,
) -> DiffResult {
    let files: Vec<FileDiff> = result
        .files
        .iter()
        .map(|f| {
            let old_data = f
                .old_oid
                .as_ref()
                .and_then(|oid| read_blob_data(odb, oid))
                .unwrap_or_default();
            let new_data = f
                .new_oid
                .as_ref()
                .and_then(|oid| read_blob_data(odb, oid))
                .unwrap_or_default();
            let binary = git_diff::binary::is_binary(&old_data)
                || git_diff::binary::is_binary(&new_data);
            let hunks = if binary {
                Vec::new()
            } else {
                git_diff::algorithm::diff_lines(
                    &old_data,
                    &new_data,
                    opts.algorithm,
                    opts.context_lines,
                )
            };
            FileDiff {
                status: f.status,
                old_path: f.old_path.clone(),
                new_path: f.new_path.clone(),
                old_mode: f.old_mode,
                new_mode: f.new_mode,
                old_oid: f.old_oid,
                new_oid: f.new_oid,
                hunks,
                is_binary: binary,
                similarity: f.similarity,
            }
        })
        .collect();
    DiffResult { files }
}

fn needs_content(opts: &DiffOptions) -> bool {
    matches!(opts.output_format, DiffOutputFormat::Unified)
}

fn determine_output_format(args: &DiffIndexArgs) -> DiffOutputFormat {
    if args.raw {
        DiffOutputFormat::Raw
    } else if args.name_only {
        DiffOutputFormat::NameOnly
    } else if args.name_status {
        DiffOutputFormat::NameStatus
    } else {
        DiffOutputFormat::Unified
    }
}

fn get_commit_tree(
    repo: &git_repository::Repository,
    oid: &ObjectId,
) -> Result<ObjectId> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid))?;
    match obj {
        Object::Commit(c) => Ok(c.tree),
        Object::Tree(_) => Ok(*oid), // Allow passing a tree directly
        _ => anyhow::bail!("not a commit or tree: {}", oid),
    }
}

/// Read all tree entries recursively, returning flat list of (path, oid, mode) for blobs.
fn read_tree_entries_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
) -> Result<Vec<(BString, ObjectId, FileMode)>> {
    let mut result = Vec::new();
    read_tree_recursive_inner(odb, tree_oid, &BString::from(""), &mut result)?;
    Ok(result)
}

fn read_tree_recursive_inner(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    prefix: &BString,
    result: &mut Vec<(BString, ObjectId, FileMode)>,
) -> Result<()> {
    let obj = odb
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree not found: {}", tree_oid))?;
    let tree = match obj {
        Object::Tree(t) => t,
        _ => anyhow::bail!("not a tree: {}", tree_oid),
    };

    for entry in &tree.entries {
        let full_path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.push(b'/');
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            read_tree_recursive_inner(odb, &entry.oid, &full_path, result)?;
        } else {
            result.push((full_path, entry.oid, entry.mode));
        }
    }

    Ok(())
}

fn matches_pathspecs(path: &BString, pathspecs: &[String]) -> bool {
    if pathspecs.is_empty() {
        return true;
    }
    pathspecs.iter().any(|spec| {
        let spec_bytes = spec.as_bytes();
        path.starts_with(spec_bytes) || spec_bytes.starts_with(path.as_ref())
    })
}

fn read_blob_data(odb: &git_odb::ObjectDatabase, oid: &ObjectId) -> Option<Vec<u8>> {
    let obj = odb.read(oid).ok()??;
    match obj {
        Object::Blob(b) => Some(b.data.to_vec()),
        _ => None,
    }
}

fn hash_blob(data: &[u8]) -> Option<ObjectId> {
    git_hash::hasher::Hasher::hash_object(git_hash::HashAlgorithm::Sha1, "blob", data).ok()
}

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

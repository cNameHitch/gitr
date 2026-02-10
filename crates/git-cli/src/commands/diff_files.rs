use std::io::{self, Write};

use anyhow::Result;
use bstr::ByteSlice;
use clap::Args;
use git_diff::format::format_diff;
use git_diff::{DiffOptions, DiffOutputFormat, DiffResult, FileDiff, FileStatus};
use git_hash::ObjectId;
use git_index::Stage;
use git_object::FileMode;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DiffFilesArgs {
    /// Generate patch output (default)
    #[arg(short = 'p', long = "patch")]
    patch: bool,

    /// Only report whether files differ (exit code only)
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Show raw diff output format
    #[arg(long)]
    raw: bool,

    /// Show only names of changed files
    #[arg(long)]
    name_only: bool,

    /// Show names and status of changed files
    #[arg(long)]
    name_status: bool,

    /// Paths to limit diff to
    #[arg(value_name = "path")]
    pathspecs: Vec<String>,
}

pub fn run(args: &DiffFilesArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("this operation must be run in a work tree"))?
        .to_path_buf();

    // Collect index entries (stage 0 only).
    let entries: Vec<(bstr::BString, ObjectId, FileMode)> = {
        let index = repo.index()?;
        index
            .iter()
            .filter(|e| e.stage == Stage::Normal)
            .filter(|e| matches_pathspecs(&e.path, &args.pathspecs))
            .map(|e| (e.path.clone(), e.oid, e.mode))
            .collect()
    };

    let odb = repo.odb();
    let mut files: Vec<FileDiff> = Vec::new();

    for (path, index_oid, index_mode) in &entries {
        let fs_path = work_tree.join(path.to_str_lossy().as_ref());

        if !fs_path.exists() {
            // File deleted from working tree
            let old_data = read_blob_data(odb, index_oid);
            let binary = old_data.as_ref().is_some_and(|d| git_diff::binary::is_binary(d));
            let hunks = if binary || args.raw || args.name_only || args.name_status || args.quiet {
                Vec::new()
            } else {
                let data = old_data.unwrap_or_default();
                git_diff::algorithm::diff_lines(&data, &[], git_diff::DiffAlgorithm::Myers, 3)
            };
            files.push(FileDiff {
                status: FileStatus::Deleted,
                old_path: Some(path.clone()),
                new_path: None,
                old_mode: Some(*index_mode),
                new_mode: None,
                old_oid: Some(*index_oid),
                new_oid: None,
                hunks,
                is_binary: binary,
                similarity: None,
            });
            continue;
        }

        let worktree_content = std::fs::read(&fs_path)?;
        let worktree_oid = hash_blob(&worktree_content);

        // Compare OID: if identical, skip
        if let Some(ref wt_oid) = worktree_oid {
            if wt_oid == index_oid {
                // Check mode change
                let wt_mode = file_mode_from_metadata(&std::fs::symlink_metadata(&fs_path)?);
                if wt_mode == *index_mode {
                    continue;
                }
            }
        }

        let wt_mode = file_mode_from_metadata(&std::fs::symlink_metadata(&fs_path)?);
        let old_data = read_blob_data(odb, index_oid).unwrap_or_default();
        let binary = git_diff::binary::is_binary(&old_data)
            || git_diff::binary::is_binary(&worktree_content);

        let hunks = if binary || args.raw || args.name_only || args.name_status || args.quiet {
            Vec::new()
        } else {
            git_diff::algorithm::diff_lines(
                &old_data,
                &worktree_content,
                git_diff::DiffAlgorithm::Myers,
                3,
            )
        };

        // Skip if no actual difference (content and mode both match)
        if hunks.is_empty() && !binary && wt_mode == *index_mode && !args.raw && !args.name_only && !args.name_status {
            // Re-check with actual byte comparison
            if old_data == worktree_content && wt_mode == *index_mode {
                continue;
            }
        }

        // Ensure we have a real difference
        if old_data == worktree_content && wt_mode == *index_mode {
            continue;
        }

        files.push(FileDiff {
            status: FileStatus::Modified,
            old_path: Some(path.clone()),
            new_path: Some(path.clone()),
            old_mode: Some(*index_mode),
            new_mode: Some(wt_mode),
            old_oid: Some(*index_oid),
            new_oid: worktree_oid,
            hunks,
            is_binary: binary,
            similarity: None,
        });
    }

    let has_changes = !files.is_empty();
    let result = DiffResult { files };

    if args.quiet {
        return Ok(if has_changes { 1 } else { 0 });
    }

    if has_changes {
        let output_format = determine_output_format(args);
        let diff_opts = DiffOptions {
            output_format,
            ..DiffOptions::default()
        };

        let output = format_diff(&result, &diff_opts);
        write!(out, "{}", output)?;
    }

    Ok(if has_changes { 1 } else { 0 })
}

fn determine_output_format(args: &DiffFilesArgs) -> DiffOutputFormat {
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

fn matches_pathspecs(path: &bstr::BString, pathspecs: &[String]) -> bool {
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
        git_object::Object::Blob(b) => Some(b.data.to_vec()),
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

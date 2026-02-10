use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};
use clap::Args;
use git_object::Object;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct DifftoolArgs {
    /// Use the specified diff tool
    #[arg(long = "tool", short = 't', value_name = "tool")]
    tool: Option<String>,

    /// Do not prompt before launching the diff tool
    #[arg(long = "no-prompt", short = 'y')]
    no_prompt: bool,

    /// Prompt before each invocation of the diff tool
    #[arg(long)]
    prompt: bool,

    /// List available diff tools
    #[arg(long)]
    tool_help: bool,

    /// Perform a directory diff
    #[arg(long = "dir-diff", short = 'd')]
    dir_diff: bool,

    /// Use symlinks in dir-diff mode
    #[arg(long)]
    symlinks: bool,

    /// Do not use symlinks in dir-diff mode
    #[arg(long)]
    no_symlinks: bool,

    /// Use an external command as the diff tool
    #[arg(long, value_name = "command")]
    extcmd: Option<String>,

    /// Show staged changes (index vs HEAD)
    #[arg(long)]
    cached: bool,

    /// Alias for --cached
    #[arg(long)]
    staged: bool,

    /// Commit or revision to diff against
    #[arg(value_name = "commit")]
    commit: Option<String>,

    /// Paths to diff
    #[arg(last = true)]
    paths: Vec<String>,
}

/// Known external diff tools and their invocation patterns.
struct ToolInfo {
    name: &'static str,
    cmd: &'static str,
    /// Arguments pattern: $LOCAL $REMOTE are replaced with actual file paths
    args: &'static [&'static str],
}

const KNOWN_TOOLS: &[ToolInfo] = &[
    ToolInfo { name: "vimdiff", cmd: "vimdiff", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "nvimdiff", cmd: "nvim", args: &["-d", "$LOCAL", "$REMOTE"] },
    ToolInfo { name: "meld", cmd: "meld", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "opendiff", cmd: "opendiff", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "kdiff3", cmd: "kdiff3", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "tkdiff", cmd: "tkdiff", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "xxdiff", cmd: "xxdiff", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "kompare", cmd: "kompare", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "bc", cmd: "bcompare", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "diffuse", cmd: "diffuse", args: &["$LOCAL", "$REMOTE"] },
    ToolInfo { name: "vscode", cmd: "code", args: &["--wait", "--diff", "$LOCAL", "$REMOTE"] },
];

pub fn run(args: &DifftoolArgs, cli: &Cli) -> Result<i32> {
    // Handle --tool-help: list available tools
    if args.tool_help {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        writeln!(out, "Available diff tools:")?;
        for tool in KNOWN_TOOLS {
            writeln!(out, "  {:12} {}", tool.name, tool.cmd)?;
        }
        writeln!(out)?;
        writeln!(out, "Use --tool=<tool> to select a tool, or")?;
        writeln!(out, "set diff.tool in your git configuration.")?;
        return Ok(0);
    }

    let mut repo = open_repo(cli)?;

    // Determine which tool to use
    let tool_name = determine_tool(args, &repo)?;

    let is_cached = args.cached || args.staged;

    // Get the list of changed files
    let changed_files = get_changed_files(&mut repo, is_cached, args.commit.as_deref())?;

    if changed_files.is_empty() {
        return Ok(0);
    }

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("bare repositories not supported for difftool"))?
        .to_path_buf();

    // For each changed file, launch the diff tool
    for file_info in &changed_files {
        // Filter by paths if specified
        if !args.paths.is_empty() {
            let matches = args.paths.iter().any(|p| file_info.path.starts_with(p));
            if !matches {
                continue;
            }
        }

        // Prompt user if needed
        if args.prompt && !args.no_prompt {
            eprint!("View diff for '{}' in {}? [Y/n] ", file_info.path, tool_name);
            let mut response = String::new();
            io::stdin().read_line(&mut response)?;
            let response = response.trim().to_lowercase();
            if response == "n" || response == "no" {
                continue;
            }
        }

        // Create temp file for the old version
        let tmp_dir = tempfile::tempdir()?;
        let old_path = tmp_dir.path().join(format!(
            "a_{}",
            file_info
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&file_info.path)
        ));

        // Write old content to temp file
        if let Some(ref old_oid) = file_info.old_oid {
            if let Some(Object::Blob(blob)) = repo.odb().read(old_oid)? {
                std::fs::write(&old_path, &blob.data)?;
            } else {
                std::fs::write(&old_path, b"")?;
            }
        } else {
            std::fs::write(&old_path, b"")?;
        }

        // Determine the new file path
        let new_path = if is_cached {
            // For cached/staged: write new version from index to temp file
            let new_tmp = tmp_dir.path().join(format!(
                "b_{}",
                file_info
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&file_info.path)
            ));
            if let Some(ref new_oid) = file_info.new_oid {
                if let Some(Object::Blob(blob)) = repo.odb().read(new_oid)? {
                    std::fs::write(&new_tmp, &blob.data)?;
                } else {
                    std::fs::write(&new_tmp, b"")?;
                }
            }
            new_tmp
        } else {
            // For working tree: use the actual file
            work_tree.join(&file_info.path)
        };

        // Launch the diff tool
        launch_tool(&tool_name, &old_path, &new_path, args.extcmd.as_deref())?;
    }

    Ok(0)
}

/// Determine which diff tool to use.
fn determine_tool(args: &DifftoolArgs, repo: &git_repository::Repository) -> Result<String> {
    // 1. Command-line --tool flag
    if let Some(ref tool) = args.tool {
        return Ok(tool.clone());
    }

    // 2. --extcmd overrides tool selection
    if args.extcmd.is_some() {
        return Ok("extcmd".to_string());
    }

    // 3. Config: diff.tool
    if let Ok(Some(tool)) = repo.config().get_string("diff.tool") {
        return Ok(tool);
    }

    // 4. Config: merge.tool (fallback)
    if let Ok(Some(tool)) = repo.config().get_string("merge.tool") {
        return Ok(tool);
    }

    // 5. Default based on platform
    #[cfg(target_os = "macos")]
    {
        Ok("opendiff".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok("vimdiff".to_string())
    }
}

/// Information about a changed file.
struct ChangedFile {
    path: String,
    old_oid: Option<git_hash::ObjectId>,
    new_oid: Option<git_hash::ObjectId>,
}

/// Get list of changed files (simplified).
fn get_changed_files(
    repo: &mut git_repository::Repository,
    cached: bool,
    _commit: Option<&str>,
) -> Result<Vec<ChangedFile>> {
    let head_oid = repo.head_oid()?;
    let work_tree = repo.work_tree().map(|p| p.to_path_buf());

    // Collect index entries into owned data to release the borrow on repo
    let index_entries: Vec<(String, git_hash::ObjectId, git_index::StatData)> = {
        let index = repo.index()?;
        index
            .iter()
            .map(|e| (e.path.to_string(), e.oid, e.stat))
            .collect()
    };

    let mut changed = Vec::new();

    if cached {
        // Compare index vs HEAD
        if let Some(head_oid) = head_oid {
            let head_tree = get_commit_tree_from_odb(repo.odb(), &head_oid)?;
            let tree_entries = collect_tree_entries(repo.odb(), &head_tree, "")?;

            for (path, oid, _stat) in &index_entries {
                let tree_oid = tree_entries.get(path).copied();
                if tree_oid != Some(*oid) {
                    changed.push(ChangedFile {
                        path: path.clone(),
                        old_oid: tree_oid,
                        new_oid: Some(*oid),
                    });
                }
            }
        }
    } else {
        // Compare working tree vs index
        if let Some(ref wt) = work_tree {
            for (path, oid, stat) in &index_entries {
                let file_path = wt.join(path);
                if file_path.exists() {
                    if let Ok(meta) = file_path.metadata() {
                        if !stat.matches(&meta) {
                            changed.push(ChangedFile {
                                path: path.clone(),
                                old_oid: Some(*oid),
                                new_oid: None, // working tree file
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(changed)
}

/// Get the tree OID from a commit via the object database.
fn get_commit_tree_from_odb(
    odb: &git_odb::ObjectDatabase,
    commit_oid: &git_hash::ObjectId,
) -> Result<git_hash::ObjectId> {
    match odb.read(commit_oid)? {
        Some(Object::Commit(commit)) => Ok(commit.tree),
        _ => bail!("not a commit: {}", commit_oid.to_hex()),
    }
}

/// Collect tree entries into a path -> OID map.
fn collect_tree_entries(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &git_hash::ObjectId,
    prefix: &str,
) -> Result<std::collections::HashMap<String, git_hash::ObjectId>> {
    let mut entries = std::collections::HashMap::new();
    collect_tree_recursive(odb, tree_oid, prefix, &mut entries)?;
    Ok(entries)
}

fn collect_tree_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &git_hash::ObjectId,
    prefix: &str,
    entries: &mut std::collections::HashMap<String, git_hash::ObjectId>,
) -> Result<()> {
    let tree = match odb.read(tree_oid)? {
        Some(Object::Tree(t)) => t,
        _ => return Ok(()),
    };

    for entry in tree.iter() {
        let name = String::from_utf8_lossy(&entry.name).to_string();
        let full_path = if prefix.is_empty() {
            name
        } else {
            format!("{}/{}", prefix, name)
        };

        if entry.mode.is_tree() {
            collect_tree_recursive(odb, &entry.oid, &full_path, entries)?;
        } else {
            entries.insert(full_path, entry.oid);
        }
    }

    Ok(())
}

/// Launch an external diff tool.
fn launch_tool(
    tool_name: &str,
    old_path: &PathBuf,
    new_path: &PathBuf,
    extcmd: Option<&str>,
) -> Result<()> {
    if let Some(ext) = extcmd {
        // Use external command directly
        let status = Command::new(ext)
            .arg(old_path)
            .arg(new_path)
            .status()?;
        if !status.success() {
            eprintln!(
                "external diff tool '{}' exited with status {:?}",
                ext,
                status.code()
            );
        }
        return Ok(());
    }

    // Look up known tool
    if let Some(tool_info) = KNOWN_TOOLS.iter().find(|t| t.name == tool_name) {
        let args: Vec<String> = tool_info
            .args
            .iter()
            .map(|a| {
                a.replace("$LOCAL", &old_path.to_string_lossy())
                    .replace("$REMOTE", &new_path.to_string_lossy())
            })
            .collect();

        let status = Command::new(tool_info.cmd)
            .args(&args)
            .status()?;

        if !status.success() {
            eprintln!(
                "diff tool '{}' exited with status {:?}",
                tool_name,
                status.code()
            );
        }
    } else {
        // Try to run tool_name as a command directly
        let status = Command::new(tool_name)
            .arg(old_path)
            .arg(new_path)
            .status();

        match status {
            Ok(s) if !s.success() => {
                eprintln!(
                    "diff tool '{}' exited with status {:?}",
                    tool_name,
                    s.code()
                );
            }
            Err(e) => {
                bail!(
                    "could not launch diff tool '{}': {}. Use --tool-help to list available tools.",
                    tool_name,
                    e
                );
            }
            _ => {}
        }
    }

    Ok(())
}

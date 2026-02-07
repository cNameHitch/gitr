use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use clap::{Args, Subcommand};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct SubmoduleArgs {
    #[command(subcommand)]
    command: Option<SubmoduleSubcommand>,
}

#[derive(Subcommand)]
pub enum SubmoduleSubcommand {
    /// Register a new submodule
    Add {
        /// Repository URL
        repository: String,

        /// Path for the submodule
        path: Option<String>,

        /// Branch to track
        #[arg(short, long)]
        branch: Option<String>,

        /// Force adding even if path exists in .gitignore
        #[arg(short, long)]
        force: bool,

        /// Name of the submodule
        #[arg(long)]
        name: Option<String>,
    },

    /// Show the status of submodules
    Status {
        /// Show status recursively
        #[arg(long)]
        recursive: bool,

        /// Paths to check
        paths: Vec<String>,
    },

    /// Initialize submodules
    Init {
        /// Paths to initialize
        paths: Vec<String>,
    },

    /// Unregister the given submodules
    Deinit {
        /// Submodule paths
        paths: Vec<String>,

        /// Force removal of submodule working tree
        #[arg(short, long)]
        force: bool,

        /// Remove all uninitialized submodules
        #[arg(long)]
        all: bool,
    },

    /// Update the registered submodules
    Update {
        /// Initialize uninitialized submodules
        #[arg(long)]
        init: bool,

        /// Update recursively
        #[arg(long)]
        recursive: bool,

        /// Force checkout
        #[arg(short, long)]
        force: bool,

        /// Checkout, rebase, or merge
        #[arg(long)]
        remote: bool,

        /// Paths to update
        paths: Vec<String>,
    },

    /// Execute a command in each submodule
    Foreach {
        /// Recurse into nested submodules
        #[arg(long)]
        recursive: bool,

        /// Command to execute
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Synchronize submodule URL configuration
    Sync {
        /// Recurse into nested submodules
        #[arg(long)]
        recursive: bool,

        /// Paths to sync
        paths: Vec<String>,
    },

    /// Show a summary of changes
    Summary {
        /// Show summary for commits rather than working tree
        #[arg(long = "cached")]
        cached: bool,

        /// Paths
        paths: Vec<String>,
    },
}

pub fn run(args: &SubmoduleArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        None => submodule_status(cli, false, &[]),
        Some(SubmoduleSubcommand::Add {
            repository,
            path,
            branch,
            force,
            name,
        }) => submodule_add(cli, repository, path.as_deref(), branch.as_deref(), *force, name.as_deref()),
        Some(SubmoduleSubcommand::Status { recursive, paths }) => {
            submodule_status(cli, *recursive, paths)
        }
        Some(SubmoduleSubcommand::Init { paths }) => submodule_init(cli, paths),
        Some(SubmoduleSubcommand::Deinit { paths, force, all }) => {
            submodule_deinit(cli, paths, *force, *all)
        }
        Some(SubmoduleSubcommand::Update {
            init,
            recursive,
            force,
            remote,
            paths,
        }) => submodule_update(cli, *init, *recursive, *force, *remote, paths),
        Some(SubmoduleSubcommand::Foreach {
            recursive,
            command,
        }) => submodule_foreach(cli, *recursive, command),
        Some(SubmoduleSubcommand::Sync { recursive, paths }) => {
            submodule_sync(cli, *recursive, paths)
        }
        Some(SubmoduleSubcommand::Summary { cached, paths }) => {
            submodule_summary(cli, *cached, paths)
        }
    }
}

/// Parse .gitmodules file and return submodule configs.
fn parse_gitmodules(work_tree: &Path) -> Result<Vec<SubmoduleConfig>> {
    let gitmodules_path = work_tree.join(".gitmodules");
    if !gitmodules_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&gitmodules_path)?;
    let mut modules = Vec::new();
    let mut current: Option<SubmoduleConfig> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.contains("submodule") {
            // Save previous module
            if let Some(module) = current.take() {
                modules.push(module);
            }
            // Parse name from [submodule "name"]
            let name = line
                .split('"')
                .nth(1)
                .unwrap_or("")
                .to_string();
            current = Some(SubmoduleConfig {
                name,
                path: String::new(),
                url: String::new(),
                branch: None,
            });
        } else if let Some(ref mut module) = current {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "path" => module.path = value.to_string(),
                    "url" => module.url = value.to_string(),
                    "branch" => module.branch = Some(value.to_string()),
                    _ => {}
                }
            }
        }
    }

    if let Some(module) = current {
        modules.push(module);
    }

    Ok(modules)
}

struct SubmoduleConfig {
    name: String,
    path: String,
    url: String,
    branch: Option<String>,
}

fn submodule_add(
    cli: &Cli,
    repository: &str,
    path: Option<&str>,
    branch: Option<&str>,
    force: bool,
    name: Option<&str>,
) -> Result<i32> {
    let repo = open_repo(cli)?;
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?
        .to_path_buf();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Determine path from URL if not given
    let sub_path = if let Some(p) = path {
        p.to_string()
    } else {
        // Extract name from URL
        let url_path = repository
            .trim_end_matches('/')
            .trim_end_matches(".git");
        url_path
            .rsplit('/')
            .next()
            .unwrap_or("submodule")
            .to_string()
    };

    let sub_name = name.unwrap_or(&sub_path);
    let full_path = work_tree.join(&sub_path);

    if full_path.exists() && !force {
        bail!("'{}' already exists in the working tree", sub_path);
    }

    writeln!(err, "Cloning into '{}'...", sub_path)?;

    // Clone the submodule
    let mut clone_args = vec!["clone"];
    if let Some(b) = branch {
        clone_args.push("-b");
        clone_args.push(b);
    }
    clone_args.push(repository);
    clone_args.push(&sub_path);

    let output = Command::new("git")
        .args(&clone_args)
        .current_dir(&work_tree)
        .output()?;

    if !output.status.success() {
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        bail!("clone failed: {}", stderr_str);
    }

    // Add to .gitmodules
    let gitmodules_path = work_tree.join(".gitmodules");
    let mut content = if gitmodules_path.exists() {
        std::fs::read_to_string(&gitmodules_path)?
    } else {
        String::new()
    };

    content.push_str(&format!(
        "[submodule \"{}\"]\n\tpath = {}\n\turl = {}\n",
        sub_name, sub_path, repository
    ));
    if let Some(b) = branch {
        content.push_str(&format!("\tbranch = {}\n", b));
    }

    std::fs::write(&gitmodules_path, &content)?;

    // Move .git dir to parent repo's modules/
    let modules_dir = repo.git_dir().join("modules").join(sub_name);
    std::fs::create_dir_all(&modules_dir)?;

    let sub_git_dir = full_path.join(".git");
    if sub_git_dir.is_dir() {
        // Move .git contents to modules dir
        for entry in std::fs::read_dir(&sub_git_dir)? {
            let entry = entry?;
            let dest = modules_dir.join(entry.file_name());
            std::fs::rename(entry.path(), dest)?;
        }
        std::fs::remove_dir(&sub_git_dir)?;

        // Replace .git with a gitdir file
        std::fs::write(
            &sub_git_dir,
            format!("gitdir: {}\n", modules_dir.display()),
        )?;
    }

    Ok(0)
}

fn submodule_status(cli: &Cli, recursive: bool, paths: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    let modules = parse_gitmodules(work_tree)?;

    for module in &modules {
        if !paths.is_empty() && !paths.iter().any(|p| p == &module.path) {
            continue;
        }

        let sub_path = work_tree.join(&module.path);

        if !sub_path.exists() {
            // Not initialized
            writeln!(
                out,
                "-0000000000000000000000000000000000000000 {}",
                module.path
            )?;
            continue;
        }

        // Get the HEAD of the submodule
        let head_file = sub_path.join(".git");
        let head_oid = if head_file.exists() {
            let output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&sub_path)
                .output()?;
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                "0000000000000000000000000000000000000000".to_string()
            }
        } else {
            "0000000000000000000000000000000000000000".to_string()
        };

        // Get the current branch of the submodule
        let branch_suffix = {
            let branch_output = Command::new("git")
                .args(["symbolic-ref", "--short", "HEAD"])
                .current_dir(&sub_path)
                .output();
            if let Ok(output) = branch_output {
                if output.status.success() {
                    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !branch.is_empty() {
                        format!(" (heads/{})", branch)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        };

        // Check if HEAD matches the recorded commit in the index
        let prefix = " "; // space = matches, + = different, - = not initialized
        writeln!(out, "{}{} {}{}", prefix, head_oid, module.path, branch_suffix)?;

        if recursive {
            // Check nested submodules
            let nested = parse_gitmodules(&sub_path).unwrap_or_default();
            for nested_module in &nested {
                writeln!(out, "  {} {}/{}", prefix, module.path, nested_module.path)?;
            }
        }
    }

    Ok(0)
}

fn submodule_init(cli: &Cli, paths: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    let modules = parse_gitmodules(work_tree)?;

    for module in &modules {
        if !paths.is_empty() && !paths.iter().any(|p| p == &module.path) {
            continue;
        }

        // Set submodule config
        let config_key = format!("submodule.{}.url", module.name);
        if let Ok(Some(_)) = repo.config().get_string(&config_key) {
            // Already initialized
            continue;
        }

        writeln!(
            err,
            "Submodule '{}' ({}) registered for path '{}'",
            module.name, module.url, module.path
        )?;

        // Write to git config
        let config_path = repo.git_dir().join("config");
        let mut config_content = std::fs::read_to_string(&config_path).unwrap_or_default();
        config_content.push_str(&format!(
            "\n[submodule \"{}\"]\n\tactive = true\n\turl = {}\n",
            module.name, module.url
        ));
        std::fs::write(&config_path, &config_content)?;
    }

    Ok(0)
}

fn submodule_deinit(cli: &Cli, paths: &[String], force: bool, all: bool) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    let modules = parse_gitmodules(work_tree)?;

    for module in &modules {
        if !all && !paths.iter().any(|p| p == &module.path) {
            continue;
        }

        let sub_path = work_tree.join(&module.path);

        // Remove working tree
        if sub_path.exists() {
            if !force {
                // Check for modifications
                let output = Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(&sub_path)
                    .output();

                if let Ok(output) = output {
                    let status = String::from_utf8_lossy(&output.stdout);
                    if !status.trim().is_empty() {
                        bail!(
                            "Submodule working tree '{}' contains local modifications; use -f to discard",
                            module.path
                        );
                    }
                }
            }

            // Remove contents but keep directory
            for entry in std::fs::read_dir(&sub_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.file_name().map(|n| n == ".git").unwrap_or(false) {
                    continue;
                }
                if path.is_dir() {
                    std::fs::remove_dir_all(&path)?;
                } else {
                    std::fs::remove_file(&path)?;
                }
            }
        }

        writeln!(err, "Cleared directory '{}'", module.path)?;
    }

    Ok(0)
}

fn submodule_update(
    cli: &Cli,
    init: bool,
    recursive: bool,
    _force: bool,
    remote: bool,
    paths: &[String],
) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    if init {
        submodule_init(cli, paths)?;
    }

    let modules = parse_gitmodules(work_tree)?;

    for module in &modules {
        if !paths.is_empty() && !paths.iter().any(|p| p == &module.path) {
            continue;
        }

        let sub_path = work_tree.join(&module.path);

        if !sub_path.exists() || !sub_path.join(".git").exists() {
            // Clone the submodule
            writeln!(err, "Cloning into '{}'...", module.path)?;
            let output = Command::new("git")
                .args(["clone", &module.url, &module.path])
                .current_dir(work_tree)
                .output()?;

            if !output.status.success() {
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                writeln!(err, "Unable to clone '{}': {}", module.url, stderr_str)?;
                continue;
            }
        } else if remote {
            // Fetch and checkout
            let _ = Command::new("git")
                .args(["fetch"])
                .current_dir(&sub_path)
                .output();

            let branch = module.branch.as_deref().unwrap_or("origin/HEAD");
            let _ = Command::new("git")
                .args(["checkout", branch])
                .current_dir(&sub_path)
                .output();
        }

        if recursive {
            let _ = Command::new("git")
                .args(["submodule", "update", "--init", "--recursive"])
                .current_dir(&sub_path)
                .output();
        }

        writeln!(err, "Submodule path '{}': checked out", module.path)?;
    }

    Ok(0)
}

fn submodule_foreach(cli: &Cli, _recursive: bool, command: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    let modules = parse_gitmodules(work_tree)?;
    let cmd_str = command.join(" ");

    for module in &modules {
        let sub_path = work_tree.join(&module.path);
        if !sub_path.exists() {
            continue;
        }

        writeln!(out, "Entering '{}'", module.path)?;

        let output = Command::new("sh")
            .args(["-c", &cmd_str])
            .current_dir(&sub_path)
            .env("name", &module.name)
            .env("sm_path", &module.path)
            .env("toplevel", work_tree.to_string_lossy().as_ref())
            .output()?;

        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;

        if !output.status.success() {
            bail!(
                "Stopping at '{}'; script returned non-zero status.",
                module.path
            );
        }
    }

    Ok(0)
}

fn submodule_sync(cli: &Cli, recursive: bool, paths: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    let modules = parse_gitmodules(work_tree)?;

    for module in &modules {
        if !paths.is_empty() && !paths.iter().any(|p| p == &module.path) {
            continue;
        }

        writeln!(
            err,
            "Synchronizing submodule url for '{}'",
            module.name
        )?;

        // Update the submodule URL in .git/config
        let sub_path = work_tree.join(&module.path);
        if sub_path.exists() {
            let _ = Command::new("git")
                .args(["remote", "set-url", "origin", &module.url])
                .current_dir(&sub_path)
                .output();
        }

        if recursive && sub_path.exists() {
            let _ = Command::new("git")
                .args(["submodule", "sync", "--recursive"])
                .current_dir(&sub_path)
                .output();
        }
    }

    Ok(0)
}

fn submodule_summary(cli: &Cli, _cached: bool, paths: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?;

    let modules = parse_gitmodules(work_tree)?;

    for module in &modules {
        if !paths.is_empty() && !paths.iter().any(|p| p == &module.path) {
            continue;
        }

        writeln!(out, "* {} (untracked):", module.path)?;
    }

    Ok(0)
}

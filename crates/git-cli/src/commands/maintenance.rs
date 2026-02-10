use std::io::{self, Write};

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use git_hash::HashAlgorithm;
use git_object::Object;
use git_ref::RefStore;
use git_revwalk::{CommitGraphWriter, RevWalk};

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct MaintenanceArgs {
    #[command(subcommand)]
    command: MaintenanceCommand,
}

#[derive(Subcommand)]
pub enum MaintenanceCommand {
    /// Run maintenance tasks on the repository
    Run {
        /// Specific task to run (gc, commit-graph, prefetch, loose-objects,
        /// incremental-repack, pack-refs)
        #[arg(long, value_name = "task")]
        task: Vec<String>,

        /// Only run tasks needed based on heuristics
        #[arg(long)]
        auto: bool,

        /// Schedule frequency (hourly, daily, weekly)
        #[arg(long, value_name = "frequency")]
        schedule: Option<String>,

        /// Suppress progress output
        #[arg(long)]
        quiet: bool,
    },
    /// Start background maintenance for the repository
    Start {
        /// Schedule frequency (hourly, daily, weekly)
        #[arg(long, value_name = "frequency")]
        scheduler: Option<String>,
    },
    /// Stop background maintenance for the repository
    Stop {
        /// Schedule frequency (hourly, daily, weekly)
        #[arg(long, value_name = "frequency")]
        scheduler: Option<String>,
    },
    /// Register the current repository for background maintenance
    Register,
    /// Unregister the current repository from background maintenance
    Unregister,
}

pub fn run(args: &MaintenanceArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        MaintenanceCommand::Run { task, auto, schedule, quiet } => {
            run_maintenance(cli, task, *auto, schedule.as_deref(), *quiet)
        }
        MaintenanceCommand::Start { scheduler } => run_start(cli, scheduler.as_deref()),
        MaintenanceCommand::Stop { scheduler } => run_stop(cli, scheduler.as_deref()),
        MaintenanceCommand::Register => run_register(cli),
        MaintenanceCommand::Unregister => run_unregister(cli),
    }
}

/// Known maintenance tasks.
const ALL_TASKS: &[&str] = &[
    "gc",
    "commit-graph",
    "prefetch",
    "loose-objects",
    "incremental-repack",
    "pack-refs",
];

fn run_maintenance(
    cli: &Cli,
    tasks: &[String],
    _auto: bool,
    _schedule: Option<&str>,
    quiet: bool,
) -> Result<i32> {
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Determine which tasks to run
    let tasks_to_run: Vec<&str> = if tasks.is_empty() {
        // Default: run all tasks
        ALL_TASKS.to_vec()
    } else {
        // Validate specified tasks
        for t in tasks {
            if !ALL_TASKS.contains(&t.as_str()) {
                bail!(
                    "error: unknown task: '{}'\nValid tasks: {}",
                    t,
                    ALL_TASKS.join(", ")
                );
            }
        }
        tasks.iter().map(|s| s.as_str()).collect()
    };

    for task_name in &tasks_to_run {
        if !quiet {
            writeln!(err, "Running maintenance task: {}", task_name)?;
        }

        let result = run_task(task_name, cli, quiet);
        if let Err(e) = result {
            if !quiet {
                writeln!(err, "warning: task '{}' failed: {}", task_name, e)?;
            }
        }
    }

    Ok(0)
}

fn run_task(task_name: &str, cli: &Cli, quiet: bool) -> Result<()> {
    match task_name {
        "gc" => {
            // Delegate to gitr gc via subprocess to avoid private field access
            let mut cmd = std::process::Command::new("gitr");
            cmd.arg("gc");
            if quiet {
                cmd.arg("--quiet");
            }
            // If gitr is not on PATH, fall back silently
            match cmd.status() {
                Ok(status) if status.success() => {}
                _ => {
                    // Fall back to git gc
                    let mut git_cmd = std::process::Command::new("git");
                    git_cmd.arg("gc");
                    if quiet {
                        git_cmd.arg("--quiet");
                    }
                    let _ = git_cmd.status();
                }
            }
        }
        "commit-graph" => {
            write_commit_graph(cli)?;
        }
        "pack-refs" => {
            let repo = open_repo(cli)?;
            pack_loose_refs(&repo)?;
        }
        "loose-objects" => {
            // Pack loose objects via repack (use subprocess to avoid private fields)
            let mut cmd = std::process::Command::new("gitr");
            cmd.args(["repack", "-d"]);
            if quiet {
                cmd.arg("-q");
            }
            match cmd.status() {
                Ok(status) if status.success() => {}
                _ => {
                    let mut git_cmd = std::process::Command::new("git");
                    git_cmd.args(["repack", "-d"]);
                    if quiet {
                        git_cmd.arg("-q");
                    }
                    let _ = git_cmd.status();
                }
            }
        }
        "incremental-repack" => {
            let mut cmd = std::process::Command::new("gitr");
            cmd.args(["repack", "-a", "-d"]);
            if quiet {
                cmd.arg("-q");
            }
            match cmd.status() {
                Ok(status) if status.success() => {}
                _ => {
                    let mut git_cmd = std::process::Command::new("git");
                    git_cmd.args(["repack", "-a", "-d"]);
                    if quiet {
                        git_cmd.arg("-q");
                    }
                    let _ = git_cmd.status();
                }
            }
        }
        "prefetch" => {
            // Prefetch from remotes - delegate to git fetch for transport
            if !quiet {
                eprintln!("gitr: prefetch task is not yet fully implemented");
            }
        }
        _ => {
            bail!("unknown maintenance task: {}", task_name);
        }
    }

    Ok(())
}

/// Write a commit-graph file (inline implementation for maintenance).
fn write_commit_graph(cli: &Cli) -> Result<()> {
    let repo = open_repo(cli)?;
    let objects_dir = repo.odb().objects_dir().to_path_buf();
    let graph_path = objects_dir.join("info").join("commit-graph");

    let mut walk = RevWalk::new(&repo)?;
    walk.push_all()?;

    let hash_algo = HashAlgorithm::Sha1;
    let mut writer = CommitGraphWriter::new(hash_algo);
    let mut count = 0u32;

    for result in &mut walk {
        let oid = result?;
        let obj = repo.odb().read(&oid)?;
        if let Some(Object::Commit(commit)) = obj {
            let tree_oid = commit.tree;
            let parents = commit.parents;
            let commit_time = commit.committer.date.timestamp;
            writer.add_commit(oid, tree_oid, parents, commit_time);
            count += 1;
        }
    }

    if count > 0 {
        writer.write(&graph_path)?;
    }

    Ok(())
}

/// Pack loose refs into packed-refs.
fn pack_loose_refs(repo: &git_repository::Repository) -> Result<()> {
    let common_dir = repo.common_dir().to_path_buf();
    let packed_refs_path = common_dir.join("packed-refs");

    let mut packed_lines: Vec<String> = Vec::new();
    packed_lines.push("# pack-refs with: peeled fully-peeled sorted".to_string());

    let refs = repo.refs().iter(Some("refs/"))?;
    for r in refs {
        let r = r?;
        let name = r.name().as_str().to_string();
        let oid = r.peel_to_oid(repo.refs())?;
        packed_lines.push(format!("{} {}", oid.to_hex(), name));
    }

    if packed_lines.len() > 1 {
        let content = packed_lines.join("\n") + "\n";
        std::fs::write(&packed_refs_path, content)?;
    }

    Ok(())
}

fn run_start(cli: &Cli, _scheduler: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    // Register the repo and set up a cron/launchd job
    // For now, write a marker file indicating maintenance is enabled
    let maintenance_dir = git_dir.join("maintenance");
    std::fs::create_dir_all(&maintenance_dir)?;
    std::fs::write(maintenance_dir.join("enabled"), "true\n")?;

    eprintln!("gitr: background maintenance started (stub)");
    eprintln!("hint: full cron/launchd integration is not yet implemented");

    Ok(0)
}

fn run_stop(cli: &Cli, _scheduler: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    let maintenance_dir = git_dir.join("maintenance");
    let enabled_file = maintenance_dir.join("enabled");
    if enabled_file.exists() {
        std::fs::remove_file(&enabled_file)?;
    }

    eprintln!("gitr: background maintenance stopped (stub)");

    Ok(0)
}

fn run_register(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let repo_path = repo.git_dir().to_path_buf();

    // In git, this adds the repo to the global maintenance config.
    // We write a marker in the git dir for now.
    let maintenance_dir = repo_path.join("maintenance");
    std::fs::create_dir_all(&maintenance_dir)?;
    std::fs::write(maintenance_dir.join("registered"), "true\n")?;

    eprintln!(
        "gitr: registered repository at '{}' for maintenance",
        repo_path.display()
    );

    Ok(0)
}

fn run_unregister(cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let repo_path = repo.git_dir().to_path_buf();

    let registered_file = repo_path.join("maintenance").join("registered");
    if registered_file.exists() {
        std::fs::remove_file(&registered_file)?;
    }

    eprintln!(
        "gitr: unregistered repository at '{}' from maintenance",
        repo_path.display()
    );

    Ok(0)
}

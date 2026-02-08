use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;
use git_ref::{RefName, RefStore};
use git_revwalk::RevWalk;

use super::open_repo;
use crate::Cli;

#[derive(Args)]
pub struct BisectArgs {
    /// Bisect subcommand (start, good, bad, skip, reset, run, log, visualize)
    subcommand: Option<String>,

    /// Additional arguments (commit OIDs, scripts, etc.)
    args: Vec<String>,
}

pub fn run(args: &BisectArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let subcmd = args.subcommand.as_deref().unwrap_or("log");

    match subcmd {
        "start" => bisect_start(&mut repo, &args.args, &mut out, &mut err),
        "good" | "old" => bisect_mark(&mut repo, &args.args, true, &mut out, &mut err),
        "bad" | "new" => bisect_mark(&mut repo, &args.args, false, &mut out, &mut err),
        "skip" => bisect_skip(&mut repo, &args.args, &mut out, &mut err),
        "reset" => bisect_reset(&mut repo, &mut out),
        "run" => bisect_run(&mut repo, &args.args, &mut out, &mut err),
        "log" => bisect_log(&repo, &mut out),
        "visualize" | "view" => bisect_visualize(&repo, &mut out),
        other => {
            writeln!(err, "error: unknown bisect subcommand: {}", other)?;
            Ok(1)
        }
    }
}

fn bisect_start(
    repo: &mut git_repository::Repository,
    args: &[String],
    out: &mut impl Write,
    _err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    // Save original HEAD for reset
    if let Some(head_oid) = repo.head_oid()? {
        fs::write(git_dir.join("BISECT_START"), head_oid.to_hex())?;
    }

    // Parse optional bad and good commits from args
    if !args.is_empty() {
        // First arg = bad
        let bad_oid = git_revwalk::resolve_revision(repo, &args[0])?;
        write_bisect_ref(repo, "bad", &bad_oid)?;
        append_bisect_log(&git_dir, &format!("# bad: [{}]", bad_oid.to_hex()))?;
    }

    if args.len() >= 2 {
        // Second arg = good
        let good_oid = git_revwalk::resolve_revision(repo, &args[1])?;
        write_bisect_good(repo, &good_oid)?;
        append_bisect_log(&git_dir, &format!("# good: [{}]", good_oid.to_hex()))?;
    }

    writeln!(out, "Bisecting: started")?;

    // Try to checkout the midpoint
    bisect_next(repo, out)?;

    Ok(0)
}

fn bisect_mark(
    repo: &mut git_repository::Repository,
    args: &[String],
    is_good: bool,
    out: &mut impl Write,
    _err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    let oid = if args.is_empty() {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?
    } else {
        git_revwalk::resolve_revision(repo, &args[0])?
    };

    if is_good {
        write_bisect_good(repo, &oid)?;
        let label = "good";
        append_bisect_log(&git_dir, &format!("# {}: [{}]", label, oid.to_hex()))?;
        writeln!(out, "Bisecting: marking {} as good", &oid.to_hex()[..7])?;
    } else {
        write_bisect_ref(repo, "bad", &oid)?;
        append_bisect_log(&git_dir, &format!("# bad: [{}]", oid.to_hex()))?;
        writeln!(out, "Bisecting: marking {} as bad", &oid.to_hex()[..7])?;
    }

    bisect_next(repo, out)?;

    Ok(0)
}

fn bisect_skip(
    repo: &mut git_repository::Repository,
    args: &[String],
    out: &mut impl Write,
    _err: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    let oid = if args.is_empty() {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?
    } else {
        git_revwalk::resolve_revision(repo, &args[0])?
    };

    // Save skip marker
    let skip_path = git_dir.join("BISECT_SKIP");
    let mut existing = if skip_path.exists() {
        fs::read_to_string(&skip_path)?
    } else {
        String::new()
    };
    existing.push_str(&oid.to_hex());
    existing.push('\n');
    fs::write(&skip_path, &existing)?;

    append_bisect_log(&git_dir, &format!("# skip: [{}]", oid.to_hex()))?;
    writeln!(out, "Bisecting: skipping {}", &oid.to_hex()[..7])?;

    bisect_next(repo, out)?;

    Ok(0)
}

fn bisect_reset(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
) -> Result<i32> {
    let git_dir = repo.git_dir().to_path_buf();

    // Restore original HEAD
    let start_path = git_dir.join("BISECT_START");
    if start_path.exists() {
        let orig_hex = fs::read_to_string(&start_path)?.trim().to_string();
        let orig_oid = ObjectId::from_hex(&orig_hex)?;

        // Update HEAD
        let head_ref = RefName::new(BString::from("HEAD"))?;
        repo.refs().write_ref(&head_ref, &orig_oid)?;

        // Checkout the tree
        if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
            let obj = repo.odb().read(&orig_oid)?;
            if let Some(Object::Commit(c)) = obj {
                super::reset::checkout_tree_to_worktree(repo.odb(), &c.tree, &work_tree)?;
            }
        }
    }

    // Clean up bisect state files
    cleanup_bisect_state(&git_dir)?;

    writeln!(out, "Bisect reset")?;
    Ok(0)
}

fn bisect_run(
    repo: &mut git_repository::Repository,
    args: &[String],
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<i32> {
    if args.is_empty() {
        bail!("bisect run requires a command");
    }

    let cmd = &args[0];
    let cmd_args = &args[1..];

    loop {
        // Run the user's test script
        let status = Command::new(cmd)
            .args(cmd_args)
            .current_dir(repo.work_tree().unwrap_or(Path::new(".")))
            .status()?;

        let exit_code = status.code().unwrap_or(128);

        let head_oid = repo
            .head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD not set"))?;

        if exit_code == 0 {
            writeln!(out, "Bisecting: {} is good", &head_oid.to_hex()[..7])?;
            write_bisect_good(repo, &head_oid)?;
        } else if exit_code == 125 {
            writeln!(out, "Bisecting: {} is skip", &head_oid.to_hex()[..7])?;
            // Skip this commit
            let git_dir = repo.git_dir().to_path_buf();
            let skip_path = git_dir.join("BISECT_SKIP");
            let mut existing = if skip_path.exists() {
                fs::read_to_string(&skip_path)?
            } else {
                String::new()
            };
            existing.push_str(&head_oid.to_hex());
            existing.push('\n');
            fs::write(&skip_path, &existing)?;
        } else if exit_code < 128 {
            writeln!(out, "Bisecting: {} is bad", &head_oid.to_hex()[..7])?;
            write_bisect_ref(repo, "bad", &head_oid)?;
        } else {
            writeln!(err, "bisect run failed with exit code {}", exit_code)?;
            return Ok(exit_code);
        }

        // Try to continue bisecting
        if !bisect_next(repo, out)? {
            break;
        }
    }

    Ok(0)
}

fn bisect_log(repo: &git_repository::Repository, out: &mut impl Write) -> Result<i32> {
    let git_dir = repo.git_dir();
    let log_path = git_dir.join("BISECT_LOG");

    if log_path.exists() {
        let content = fs::read_to_string(&log_path)?;
        write!(out, "{}", content)?;
    }

    Ok(0)
}

fn bisect_visualize(repo: &git_repository::Repository, out: &mut impl Write) -> Result<i32> {
    // Show the remaining commits to test
    let (good_oids, bad_oid) = read_bisect_state(repo)?;

    if let Some(bad) = bad_oid {
        let mut walker = RevWalk::new(repo)?;
        walker.push(bad)?;
        for good in &good_oids {
            walker.hide(*good)?;
        }

        for oid_result in walker {
            let oid = oid_result?;
            let obj = repo.odb().read(&oid)?;
            if let Some(Object::Commit(c)) = obj {
                let summary = String::from_utf8_lossy(c.summary());
                writeln!(out, "{} {}", &oid.to_hex()[..7], summary)?;
            }
        }
    }

    Ok(0)
}

/// Calculate the next bisect midpoint and check it out.
/// Returns false if bisect is complete.
fn bisect_next(
    repo: &mut git_repository::Repository,
    out: &mut impl Write,
) -> Result<bool> {
    let (good_oids, bad_oid) = read_bisect_state(repo)?;

    let bad = match bad_oid {
        Some(b) => b,
        None => {
            writeln!(out, "Bisecting: need a bad commit")?;
            return Ok(false);
        }
    };

    if good_oids.is_empty() {
        writeln!(out, "Bisecting: need a good commit")?;
        return Ok(false);
    }

    // Read skip list
    let git_dir = repo.git_dir().to_path_buf();
    let skip_set = read_skip_list(&git_dir)?;

    // Collect commits between good and bad
    let mut walker = RevWalk::new(repo)?;
    walker.push(bad)?;
    for good in &good_oids {
        walker.hide(*good)?;
    }

    let mut candidates: Vec<ObjectId> = Vec::new();
    for oid_result in walker {
        let oid = oid_result?;
        if !skip_set.contains(&oid) {
            candidates.push(oid);
        }
    }

    if candidates.is_empty() {
        writeln!(
            out,
            "The first bad commit is:\n{}",
            bad.to_hex()
        )?;
        return Ok(false);
    }

    if candidates.len() == 1 {
        writeln!(
            out,
            "The first bad commit is:\n{}",
            candidates[0].to_hex()
        )?;
        return Ok(false);
    }

    // Pick the midpoint
    let mid_idx = candidates.len() / 2;
    let mid_oid = candidates[mid_idx];

    writeln!(
        out,
        "Bisecting: {} revisions left to test after this (roughly {} steps)",
        candidates.len(),
        (candidates.len() as f64).log2().ceil() as usize
    )?;

    // Checkout the midpoint
    checkout_commit(repo, &mid_oid)?;

    let obj = repo.odb().read(&mid_oid)?;
    if let Some(Object::Commit(c)) = obj {
        let summary = String::from_utf8_lossy(c.summary());
        writeln!(out, "[{}] {}", &mid_oid.to_hex()[..7], summary)?;
    }

    Ok(true)
}

fn checkout_commit(repo: &mut git_repository::Repository, oid: &ObjectId) -> Result<()> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("commit not found: {}", oid))?;

    let tree_oid = match obj {
        Object::Commit(c) => c.tree,
        _ => bail!("not a commit: {}", oid),
    };

    // Update HEAD to detached
    let head_ref = RefName::new(BString::from("HEAD"))?;
    repo.refs().write_ref(&head_ref, oid)?;

    // Checkout tree to worktree
    if let Some(work_tree) = repo.work_tree().map(|p| p.to_path_buf()) {
        super::reset::checkout_tree_to_worktree(repo.odb(), &tree_oid, &work_tree)?;
    }

    Ok(())
}

fn read_bisect_state(
    repo: &git_repository::Repository,
) -> Result<(Vec<ObjectId>, Option<ObjectId>)> {
    let mut good_oids = Vec::new();
    let mut bad_oid = None;

    // Read good refs
    let refs = repo.refs().iter(Some("refs/bisect/"))?;
    for r in refs {
        let r = r?;
        let name = r.name().as_str();
        if name.starts_with("refs/bisect/good-") {
            if let Some(oid) = r.target_oid() {
                good_oids.push(oid);
            }
        } else if name == "refs/bisect/bad" {
            if let Some(oid) = r.target_oid() {
                bad_oid = Some(oid);
            }
        }
    }

    Ok((good_oids, bad_oid))
}

fn read_skip_list(git_dir: &Path) -> Result<std::collections::HashSet<ObjectId>> {
    let mut set = std::collections::HashSet::new();
    let path = git_dir.join("BISECT_SKIP");
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() {
                if let Ok(oid) = ObjectId::from_hex(line) {
                    set.insert(oid);
                }
            }
        }
    }
    Ok(set)
}

fn write_bisect_ref(
    repo: &git_repository::Repository,
    name: &str,
    oid: &ObjectId,
) -> Result<()> {
    let refname = RefName::new(BString::from(format!("refs/bisect/{}", name)))?;
    repo.refs().write_ref(&refname, oid)?;
    Ok(())
}

fn write_bisect_good(
    repo: &git_repository::Repository,
    oid: &ObjectId,
) -> Result<()> {
    let hex = oid.to_hex();
    let short = &hex[..7.min(hex.len())];
    let refname = RefName::new(BString::from(format!("refs/bisect/good-{}", short)))?;
    repo.refs().write_ref(&refname, oid)?;
    Ok(())
}

fn append_bisect_log(git_dir: &Path, message: &str) -> Result<()> {
    let log_path = git_dir.join("BISECT_LOG");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    writeln!(file, "{}", message)?;
    Ok(())
}

fn cleanup_bisect_state(git_dir: &Path) -> Result<()> {
    let files = [
        "BISECT_START",
        "BISECT_LOG",
        "BISECT_NAMES",
        "BISECT_EXPECTED_REV",
        "BISECT_SKIP",
    ];

    for f in &files {
        let path = git_dir.join(f);
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }

    // Remove bisect refs
    let refs_dir = git_dir.join("refs").join("bisect");
    if refs_dir.exists() {
        fs::remove_dir_all(&refs_dir)?;
    }

    Ok(())
}

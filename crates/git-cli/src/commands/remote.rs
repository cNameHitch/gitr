use std::io::{self, Write};

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RemoteArgs {
    /// Be verbose (show URLs)
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<RemoteSubcommand>,
}

#[derive(Subcommand)]
pub enum RemoteSubcommand {
    /// Add a new remote
    Add {
        name: String,
        url: String,
    },
    /// Remove a remote
    Remove {
        name: String,
    },
    /// Rename a remote
    Rename {
        old: String,
        new: String,
    },
    /// Set URL for a remote
    #[command(name = "set-url")]
    SetUrl {
        name: String,
        url: String,
    },
    /// Show information about a remote
    Show {
        name: String,
    },
}

pub fn run(args: &RemoteArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match &args.command {
        None => {
            // List remotes
            list_remotes(&repo, args.verbose, &mut out)?;
        }
        Some(RemoteSubcommand::Add { name, url }) => {
            add_remote(&repo, name, url)?;
        }
        Some(RemoteSubcommand::Remove { name }) => {
            remove_remote(&repo, name)?;
        }
        Some(RemoteSubcommand::Rename { old, new }) => {
            rename_remote(&repo, old, new)?;
        }
        Some(RemoteSubcommand::SetUrl { name, url }) => {
            set_url(&repo, name, url)?;
        }
        Some(RemoteSubcommand::Show { name }) => {
            show_remote(&repo, name, &mut out)?;
        }
    }

    Ok(0)
}

fn list_remotes(repo: &git_repository::Repository, verbose: bool, out: &mut impl Write) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let mut remotes = Vec::new();
    let mut current_remote: Option<String> = None;
    let mut current_url: Option<String> = None;
    let mut current_push_url: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[remote \"") {
            // Save previous remote
            if let (Some(name), Some(url)) = (current_remote.take(), current_url.take()) {
                remotes.push((name, url, current_push_url.take()));
            }
            current_remote = rest.strip_suffix("\"]").map(|s| s.to_string());
            current_url = None;
            current_push_url = None;
        } else if let Some(url) = trimmed.strip_prefix("url = ") {
            if current_remote.is_some() {
                current_url = Some(url.to_string());
            }
        } else if let Some(url) = trimmed.strip_prefix("pushurl = ") {
            if current_remote.is_some() {
                current_push_url = Some(url.to_string());
            }
        }
    }
    if let (Some(name), Some(url)) = (current_remote, current_url) {
        remotes.push((name, url, current_push_url));
    }

    for (name, url, push_url) in &remotes {
        if verbose {
            writeln!(out, "{}\t{} (fetch)", name, url)?;
            writeln!(out, "{}\t{} (push)", name, push_url.as_deref().unwrap_or(url))?;
        } else {
            writeln!(out, "{}", name)?;
        }
    }

    Ok(())
}

fn add_remote(repo: &git_repository::Repository, name: &str, url: &str) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let mut content = std::fs::read_to_string(&config_path).unwrap_or_default();

    // Check if remote already exists
    if content.contains(&format!("[remote \"{}\"]", name)) {
        bail!("fatal: remote {} already exists.", name);
    }

    content.push_str(&format!(
        "\n[remote \"{}\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/{}/*\n",
        name, url, name
    ));
    std::fs::write(&config_path, content)?;
    Ok(())
}

fn remove_remote(repo: &git_repository::Repository, name: &str) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path)?;

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: No such remote: '{}'", name);
    }

    // Remove the section
    let mut new_content = String::new();
    let mut skip = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            skip = true;
            continue;
        }
        if skip && trimmed.starts_with('[') {
            skip = false;
        }
        if !skip {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    std::fs::write(&config_path, new_content)?;

    // Also clean up remote tracking refs
    let refs_dir = repo.git_dir().join("refs").join("remotes").join(name);
    if refs_dir.exists() {
        std::fs::remove_dir_all(&refs_dir)?;
    }

    Ok(())
}

fn rename_remote(repo: &git_repository::Repository, old: &str, new: &str) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path)?;

    let old_section = format!("[remote \"{}\"]", old);
    if !content.contains(&old_section) {
        bail!("fatal: No such remote: '{}'", old);
    }

    let new_section = format!("[remote \"{}\"]", new);
    let updated = content
        .replace(&old_section, &new_section)
        .replace(
            &format!("refs/remotes/{}/*", old),
            &format!("refs/remotes/{}/*", new),
        );
    std::fs::write(&config_path, updated)?;

    // Rename tracking refs directory
    let old_dir = repo.git_dir().join("refs").join("remotes").join(old);
    let new_dir = repo.git_dir().join("refs").join("remotes").join(new);
    if old_dir.exists() {
        std::fs::rename(&old_dir, &new_dir)?;
    }

    Ok(())
}

fn show_remote(repo: &git_repository::Repository, name: &str, out: &mut impl Write) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: '{}' does not appear to be a git repository", name);
    }

    // Parse remote config
    let mut url_str = String::new();
    let mut _fetch_refspec = String::new();
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with('[') {
            in_section = false;
        }
        if in_section {
            if let Some(u) = trimmed.strip_prefix("url = ") {
                url_str = u.to_string();
            } else if let Some(f) = trimmed.strip_prefix("fetch = ") {
                _fetch_refspec = f.to_string();
            }
        }
    }

    // Find HEAD branch from remote tracking
    let remote_prefix = format!("refs/remotes/{}/", name);
    let mut tracked_branches = Vec::new();
    let mut head_branch = String::from("(unknown)");

    if let Ok(refs) = repo.refs().iter(Some(&remote_prefix)) {
        for r in refs.flatten() {
            let full = r.name().as_str().to_string();
            let short = full.strip_prefix(&remote_prefix).unwrap_or(&full).to_string();
            if short == "HEAD" {
                // Try to determine HEAD branch by matching OID
                if let Ok(head_oid) = r.peel_to_oid(repo.refs()) {
                    if let Ok(refs2) = repo.refs().iter(Some(&remote_prefix)) {
                        for r2 in refs2.flatten() {
                            let name2 = r2.name().as_str().to_string();
                            let short2 = name2.strip_prefix(&remote_prefix).unwrap_or(&name2).to_string();
                            if short2 != "HEAD" {
                                if let Ok(oid2) = r2.peel_to_oid(repo.refs()) {
                                    if oid2 == head_oid {
                                        head_branch = short2;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                continue; // Don't list HEAD as a tracked branch
            }
            tracked_branches.push(short);
        }
    }

    writeln!(out, "* remote {}", name)?;
    writeln!(out, "  Fetch URL: {}", url_str)?;
    writeln!(out, "  Push  URL: {}", url_str)?;
    writeln!(out, "  HEAD branch: {}", head_branch)?;
    if !tracked_branches.is_empty() {
        writeln!(out, "  Remote branch:")?;
        for branch in &tracked_branches {
            writeln!(out, "    {} tracked", branch)?;
        }
    }

    // Show local branch tracking info (for git pull)
    let branch_section_header = "[branch \"";
    let mut pull_branches = Vec::new();
    let mut current_branch_name: Option<String> = None;
    let mut current_merge: Option<String> = None;
    let mut current_remote_name: Option<String> = None;
    let mut in_branch = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(branch_section_header) {
            // Save previous branch info
            if let (Some(bn), Some(merge), Some(rn)) =
                (current_branch_name.take(), current_merge.take(), current_remote_name.take())
            {
                if rn == name {
                    let remote_branch = merge
                        .strip_prefix("refs/heads/")
                        .unwrap_or(&merge)
                        .to_string();
                    pull_branches.push((bn, remote_branch));
                }
            }
            current_branch_name = rest.strip_suffix("\"]").map(|s| s.to_string());
            current_merge = None;
            current_remote_name = None;
            in_branch = true;
        } else if in_branch && trimmed.starts_with('[') {
            in_branch = false;
            // Save branch info before section ends
            if let (Some(bn), Some(merge), Some(rn)) =
                (current_branch_name.take(), current_merge.take(), current_remote_name.take())
            {
                if rn == name {
                    let remote_branch = merge
                        .strip_prefix("refs/heads/")
                        .unwrap_or(&merge)
                        .to_string();
                    pull_branches.push((bn, remote_branch));
                }
            }
        } else if in_branch {
            if let Some(m) = trimmed.strip_prefix("merge = ") {
                current_merge = Some(m.to_string());
            } else if let Some(r) = trimmed.strip_prefix("remote = ") {
                current_remote_name = Some(r.to_string());
            }
        }
    }
    // Handle last branch section
    if let (Some(bn), Some(merge), Some(rn)) =
        (current_branch_name, current_merge, current_remote_name)
    {
        if rn == name {
            let remote_branch = merge
                .strip_prefix("refs/heads/")
                .unwrap_or(&merge)
                .to_string();
            pull_branches.push((bn, remote_branch));
        }
    }

    if !pull_branches.is_empty() {
        writeln!(out, "  Local branch configured for 'git pull':")?;
        for (local, remote_branch) in &pull_branches {
            writeln!(out, "    {} merges with remote {}", local, remote_branch)?;
        }
    }

    // Show local ref configured for git push
    if !pull_branches.is_empty() {
        writeln!(out, "  Local ref configured for 'git push':")?;
        for (local, remote_branch) in &pull_branches {
            // Check if local branch is up to date with remote
            let mut status = "up to date";

            if let (Ok(local_rn), Ok(remote_rn)) = (
                RefName::new(format!("refs/heads/{}", local)),
                RefName::new(format!("refs/remotes/{}/{}", name, remote_branch)),
            ) {
                if let (Ok(Some(local_oid)), Ok(Some(remote_oid))) = (
                    repo.refs().resolve_to_oid(&local_rn),
                    repo.refs().resolve_to_oid(&remote_rn),
                ) {
                    if local_oid != remote_oid {
                        status = "local out of date";
                    }
                }
            }

            writeln!(out, "    {} pushes to {} ({})", local, remote_branch, status)?;
        }
    }

    Ok(())
}

fn set_url(repo: &git_repository::Repository, name: &str, url: &str) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path)?;

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: No such remote '{}'", name);
    }

    let mut new_content = String::new();
    let mut in_section = false;
    let mut url_replaced = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            new_content.push_str(line);
            new_content.push('\n');
            continue;
        }
        if in_section && trimmed.starts_with('[') {
            in_section = false;
        }
        if in_section && trimmed.starts_with("url = ") && !url_replaced {
            new_content.push_str(&format!("\turl = {}", url));
            new_content.push('\n');
            url_replaced = true;
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }
    std::fs::write(&config_path, new_content)?;
    Ok(())
}

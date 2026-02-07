use std::io::{self, Write};

use anyhow::{bail, Result};
use clap::{Args, Subcommand};

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

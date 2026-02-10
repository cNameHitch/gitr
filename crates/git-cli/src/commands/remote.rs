use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::BString;
use clap::{Args, Subcommand};
use git_ref::{RefName, RefStore};

use crate::Cli;
use super::{open_repo, fetch};

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
    /// Set the default branch (HEAD) for a remote
    #[command(name = "set-head")]
    SetHead {
        name: String,
        /// Branch to set as HEAD
        branch: Option<String>,
        /// Determine remote HEAD automatically
        #[arg(short, long)]
        auto: bool,
        /// Delete the remote HEAD reference
        #[arg(short, long)]
        delete: bool,
    },
    /// Remove stale remote-tracking branches
    Prune {
        name: String,
        /// Report what would be pruned without actually doing it
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Fetch updates for remote groups
    Update {
        /// Remote group to update
        group: Option<String>,
        /// Prune stale branches during update
        #[arg(short, long)]
        prune: bool,
    },
    /// Change the list of branches tracked by a remote
    #[command(name = "set-branches")]
    SetBranches {
        name: String,
        /// Branches to track
        branches: Vec<String>,
        /// Add to existing tracked branches instead of replacing
        #[arg(long)]
        add: bool,
    },
    /// Show URLs for a remote
    #[command(name = "get-url")]
    GetUrl {
        name: String,
        /// Show push URL instead of fetch URL
        #[arg(long)]
        push: bool,
        /// Show all URLs
        #[arg(long)]
        all: bool,
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
        Some(RemoteSubcommand::SetHead { name, branch, auto, delete }) => {
            set_head(&repo, name, branch.as_deref(), *auto, *delete, &mut out)?;
        }
        Some(RemoteSubcommand::Prune { name, dry_run }) => {
            prune_remote(&repo, name, *dry_run, &mut out)?;
        }
        Some(RemoteSubcommand::Update { group, prune }) => {
            update_remotes(&repo, group.as_deref(), *prune, cli)?;
        }
        Some(RemoteSubcommand::SetBranches { name, branches, add }) => {
            set_branches(&repo, name, branches, *add)?;
        }
        Some(RemoteSubcommand::GetUrl { name, push, all }) => {
            get_url(&repo, name, *push, *all, &mut out)?;
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

fn get_url(
    repo: &git_repository::Repository,
    name: &str,
    push: bool,
    all: bool,
    out: &mut impl Write,
) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: No such remote '{}'", name);
    }

    // Parse all URLs and push URLs from the remote section
    let mut urls: Vec<String> = Vec::new();
    let mut push_urls: Vec<String> = Vec::new();
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with('[') {
            break;
        }
        if in_section {
            if let Some(u) = trimmed.strip_prefix("url = ") {
                urls.push(u.to_string());
            } else if let Some(u) = trimmed.strip_prefix("pushurl = ") {
                push_urls.push(u.to_string());
            }
        }
    }

    if push {
        // Show push URLs; fall back to regular URLs if no pushurl is set
        let effective = if push_urls.is_empty() { &urls } else { &push_urls };
        if all {
            for u in effective {
                writeln!(out, "{}", u)?;
            }
        } else if let Some(u) = effective.first() {
            writeln!(out, "{}", u)?;
        }
    } else {
        // Show fetch URLs
        if all {
            for u in &urls {
                writeln!(out, "{}", u)?;
            }
        } else if let Some(u) = urls.first() {
            writeln!(out, "{}", u)?;
        }
    }

    Ok(())
}

fn set_head(
    repo: &git_repository::Repository,
    name: &str,
    branch: Option<&str>,
    auto: bool,
    delete: bool,
    out: &mut impl Write,
) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: No such remote '{}'", name);
    }

    let head_ref_name = format!("refs/remotes/{}/HEAD", name);

    if delete {
        // Delete the symbolic ref
        let ref_name = RefName::new(head_ref_name.as_str())?;
        // Try to delete; ignore error if it doesn't exist
        let _ = repo.refs().delete_ref(&ref_name);
        return Ok(());
    }

    if auto {
        // Determine the default branch automatically by inspecting remote tracking refs.
        // Look at the current HEAD symbolic ref target, or find the branch that HEAD points to.
        let head_ref = RefName::new(head_ref_name.as_str())?;

        // Try to resolve HEAD to an OID and match against tracking branches
        let remote_prefix = format!("refs/remotes/{}/", name);
        let mut default_branch: Option<String> = None;

        // First, check if there's already a HEAD ref we can resolve by OID matching
        if let Ok(Some(head_oid)) = repo.refs().resolve_to_oid(&head_ref) {
            if let Ok(iter) = repo.refs().iter(Some(&remote_prefix)) {
                for r in iter.flatten() {
                    let rname = r.name().as_str().to_string();
                    let short = rname.strip_prefix(&remote_prefix).unwrap_or(&rname);
                    if short == "HEAD" {
                        continue;
                    }
                    if let Ok(oid) = r.peel_to_oid(repo.refs()) {
                        if oid == head_oid {
                            default_branch = Some(short.to_string());
                            break;
                        }
                    }
                }
            }
        }

        // If we still don't know, try the common default names
        if default_branch.is_none() {
            for candidate in &["main", "master"] {
                let candidate_ref = format!("refs/remotes/{}/{}", name, candidate);
                if let Ok(rn) = RefName::new(candidate_ref.as_str()) {
                    if repo.refs().resolve_to_oid(&rn).ok().flatten().is_some() {
                        default_branch = Some(candidate.to_string());
                        break;
                    }
                }
            }
        }

        match default_branch {
            Some(branch_name) => {
                let target = format!("refs/remotes/{}/{}", name, branch_name);
                let sym_name = RefName::new(head_ref_name.as_str())?;
                let sym_target = RefName::new(target.as_str())?;
                repo.refs().write_symbolic_ref(&sym_name, &sym_target)?;
                writeln!(
                    out,
                    "{}/HEAD set to {}",
                    name, branch_name
                )?;
            }
            None => {
                bail!(
                    "fatal: could not determine default branch for remote '{}'",
                    name
                );
            }
        }

        return Ok(());
    }

    // Explicit mode: set-head <remote> <branch>
    match branch {
        Some(branch_name) => {
            let target = format!("refs/remotes/{}/{}", name, branch_name);
            let sym_name = RefName::new(head_ref_name.as_str())?;
            let sym_target = RefName::new(target.as_str())?;
            repo.refs().write_symbolic_ref(&sym_name, &sym_target)?;
        }
        None => {
            bail!("fatal: set-head requires a branch name, --auto, or --delete");
        }
    }

    Ok(())
}

fn prune_remote(
    repo: &git_repository::Repository,
    name: &str,
    dry_run: bool,
    out: &mut impl Write,
) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: No such remote '{}'", name);
    }

    // Parse fetch refspecs from config to know the mapping
    let mut fetch_refspecs: Vec<String> = Vec::new();
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with('[') {
            break;
        }
        if in_section {
            if let Some(f) = trimmed.strip_prefix("fetch = ") {
                fetch_refspecs.push(f.to_string());
            }
        }
    }

    // Build the set of branches that the remote actually has.
    // We look at refs/remotes/<name>/* to find what we're tracking locally,
    // then check if the corresponding source ref would exist.
    // For prune, we need to figure out which local tracking refs are stale.
    // A tracking ref is stale if the remote no longer has the corresponding branch.
    //
    // Without actually connecting to the remote, we detect stale refs by checking
    // if the ref file exists but the remote branch is gone. The standard approach
    // is to compare against what was last fetched. For a local-only prune,
    // we remove tracking refs whose corresponding remote branch no longer appears
    // in the packed-refs or loose refs from the last fetch.
    //
    // Since we can't connect to the remote here (that's what fetch --prune does),
    // we prune refs that exist under refs/remotes/<name>/ but whose target OID
    // doesn't resolve to a valid object. However, the more standard approach for
    // `git remote prune` is to actually contact the remote.
    //
    // For this implementation, we'll use the same approach as fetch --prune:
    // list local tracking refs and compare against what the remote advertises.

    // Actually connect to the remote to get its current refs
    let remote_config = git_protocol::remote::RemoteConfig::from_config(repo.config(), name)?
        .ok_or_else(|| anyhow::anyhow!("fatal: '{}' does not appear to be a git repository", name))?;

    let url = git_transport::GitUrl::parse(&remote_config.url)?;
    let mut transport = git_transport::connect(&url, git_transport::Service::UploadPack)?;

    let reader = &mut git_protocol::pktline::PktLineReader::new(transport.reader());
    let (advertised_refs, _capabilities) = git_protocol::v1::parse_ref_advertisement(reader)?;

    // Build set of remote ref destinations using refspecs
    let refspecs: Vec<git_protocol::remote::RefSpec> = remote_config.fetch_refspecs.clone();

    let remote_ref_names: std::collections::HashSet<String> = advertised_refs
        .iter()
        .filter_map(|(_, rname)| {
            let n = String::from_utf8_lossy(rname.as_ref()).to_string();
            refspecs.iter().find_map(|rs| rs.map_to_destination(&n))
        })
        .collect();

    // Find local tracking refs that are no longer on the remote
    let prefix = format!("refs/remotes/{}/", name);
    if let Ok(iter) = repo.refs().iter(Some(&prefix)) {
        for r in iter.flatten() {
            let ref_full = r.name().as_str().to_string();
            // Skip HEAD
            if ref_full == format!("refs/remotes/{}/HEAD", name) {
                continue;
            }
            if !remote_ref_names.contains(&ref_full) {
                let short = ref_full.strip_prefix("refs/remotes/").unwrap_or(&ref_full);
                if dry_run {
                    writeln!(out, " * [would prune] {}", short)?;
                } else {
                    let ref_name = RefName::new(BString::from(ref_full.as_str()))?;
                    repo.refs().delete_ref(&ref_name)?;
                    writeln!(out, " * [pruned] {}", short)?;
                }
            }
        }
    }

    Ok(())
}

fn update_remotes(
    repo: &git_repository::Repository,
    group: Option<&str>,
    prune: bool,
    cli: &Cli,
) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    // Determine which remotes to update
    let remote_names: Vec<String> = if let Some(group_name) = group {
        // Check for remotes.group config
        let group_key = format!("remotes.{}", group_name);
        match repo.config().get_string(&group_key)? {
            Some(val) => val.split_whitespace().map(|s| s.to_string()).collect(),
            None => {
                // If not a group, maybe it's a single remote name
                let section_header = format!("[remote \"{}\"]", group_name);
                if content.contains(&section_header) {
                    vec![group_name.to_string()]
                } else {
                    bail!("fatal: '{}' is not a remote or remote group", group_name);
                }
            }
        }
    } else {
        // Update all remotes - parse remote names from config
        let mut names = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("[remote \"") {
                if let Some(name) = rest.strip_suffix("\"]") {
                    names.push(name.to_string());
                }
            }
        }
        names
    };

    let stderr = io::stderr();
    let mut err = stderr.lock();

    for remote_name in &remote_names {
        writeln!(err, "Fetching {}", remote_name)?;

        let fetch_args = fetch::FetchArgs {
            all: false,
            prune,
            depth: None,
            tags: false,
            quiet: false,
            verbose: false,
            force: false,
            dry_run: false,
            jobs: None,
            shallow_since: None,
            shallow_exclude: None,
            unshallow: false,
            deepen: None,
            recurse_submodules: false,
            set_upstream: false,
            remote: Some(remote_name.clone()),
            refspec: vec![],
        };

        match fetch::run(&fetch_args, cli) {
            Ok(_) => {}
            Err(e) => {
                writeln!(err, "error: Could not fetch {}: {}", remote_name, e)?;
            }
        }
    }

    Ok(())
}

fn set_branches(
    repo: &git_repository::Repository,
    name: &str,
    branches: &[String],
    add: bool,
) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let content = std::fs::read_to_string(&config_path)?;

    let section_header = format!("[remote \"{}\"]", name);
    if !content.contains(&section_header) {
        bail!("fatal: No such remote '{}'", name);
    }

    // Build the new fetch refspecs for the specified branches
    let new_refspecs: Vec<String> = branches
        .iter()
        .map(|b| format!("+refs/heads/{}:refs/remotes/{}/{}", b, name, b))
        .collect();

    // Parse the config and rebuild the remote section
    let mut new_content = String::new();
    let mut in_section = false;
    let mut existing_fetches: Vec<String> = Vec::new();
    let mut section_ended = false;
    let mut fetches_written = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == section_header {
            in_section = true;
            new_content.push_str(line);
            new_content.push('\n');
            continue;
        }

        if in_section && trimmed.starts_with('[') {
            // End of the remote section; write the fetch lines before the next section
            in_section = false;
            section_ended = true;

            if !fetches_written {
                write_fetch_lines(&mut new_content, &existing_fetches, &new_refspecs, add);
                fetches_written = true;
            }

            new_content.push_str(line);
            new_content.push('\n');
            continue;
        }

        if in_section {
            if trimmed.starts_with("fetch = ") {
                // Collect existing fetch refspecs (we'll replace or append)
                if let Some(f) = trimmed.strip_prefix("fetch = ") {
                    existing_fetches.push(f.to_string());
                }
                // Don't write old fetch lines; we'll write them later
                continue;
            }
            new_content.push_str(line);
            new_content.push('\n');
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    // If the remote section was the last section in the file
    if in_section && !fetches_written {
        write_fetch_lines(&mut new_content, &existing_fetches, &new_refspecs, add);
    }
    // If section ended via EOF without encountering another section header
    if !section_ended && !in_section && !fetches_written {
        // Already handled above
    }

    std::fs::write(&config_path, new_content)?;
    Ok(())
}

fn write_fetch_lines(
    content: &mut String,
    existing: &[String],
    new_refspecs: &[String],
    add: bool,
) {
    if add {
        // Keep existing and append new ones
        for f in existing {
            content.push_str(&format!("\tfetch = {}\n", f));
        }
        for f in new_refspecs {
            // Only add if not already present
            if !existing.contains(f) {
                content.push_str(&format!("\tfetch = {}\n", f));
            }
        }
    } else {
        // Replace: only write the new refspecs
        for f in new_refspecs {
            content.push_str(&format!("\tfetch = {}\n", f));
        }
    }
}

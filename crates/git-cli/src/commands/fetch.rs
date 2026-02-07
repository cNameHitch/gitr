use std::io::{self, Write};

use anyhow::Result;
use bstr::{BString, ByteSlice};
use clap::Args;
use git_hash::ObjectId;
use git_protocol::remote::{RefSpec, RemoteConfig};
use git_ref::{RefName, RefStore};
use git_transport::Service;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct FetchArgs {
    /// Fetch all remotes
    #[arg(long)]
    pub all: bool,

    /// Prune remote-tracking refs that no longer exist
    #[arg(short, long)]
    pub prune: bool,

    /// Limit fetching to specified depth
    #[arg(long)]
    pub depth: Option<u32>,

    /// Fetch all tags
    #[arg(long)]
    pub tags: bool,

    /// Be quiet
    #[arg(short, long)]
    pub quiet: bool,

    /// Remote name
    pub remote: Option<String>,

    /// Refspecs to fetch
    pub refspec: Vec<String>,
}

pub fn run(args: &FetchArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let remote_name = args.remote.as_deref().unwrap_or("origin");

    // Read remote config
    let remote_config = RemoteConfig::from_config(repo.config(), remote_name)?
        .ok_or_else(|| anyhow::anyhow!("fatal: '{}' does not appear to be a git repository", remote_name))?;

    let url = git_transport::GitUrl::parse(&remote_config.url)?;
    let mut transport = git_transport::connect(&url, Service::UploadPack)?;

    // Read ref advertisement
    let reader = &mut git_protocol::pktline::PktLineReader::new(transport.reader());
    let (advertised_refs, capabilities) = git_protocol::v1::parse_ref_advertisement(reader)?;

    if !args.quiet {
        writeln!(err, "From {}", remote_config.url)?;
    }

    // Determine wanted refs
    let refspecs: Vec<RefSpec> = if !args.refspec.is_empty() {
        args.refspec.iter()
            .map(|s| RefSpec::parse(s))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        remote_config.fetch_refspecs.clone()
    };

    let wanted_refs: Vec<String> = advertised_refs
        .iter()
        .filter(|(_, name)| {
            let n = name.to_str_lossy();
            refspecs.iter().any(|rs| rs.matches_source(&n))
                || (args.tags && n.starts_with("refs/tags/"))
        })
        .map(|(_, name)| name.to_str_lossy().to_string())
        .collect();

    // Collect local refs for negotiation
    let local_refs: Vec<(ObjectId, String)> = {
        let mut refs = Vec::new();
        if let Ok(iter) = repo.refs().iter(Some("refs/")) {
            for r in iter {
                if let Ok(r) = r {
                    if let Some(oid) = r.target_oid() {
                        refs.push((oid, r.name().as_str().to_string()));
                    }
                }
            }
        }
        refs
    };

    let fetch_opts = git_protocol::fetch::FetchOptions {
        depth: args.depth,
        filter: None,
        progress: !args.quiet,
    };

    let pack_dir = repo.common_dir().join("objects").join("pack");
    std::fs::create_dir_all(&pack_dir)?;

    let _result = git_protocol::fetch::fetch(
        transport.as_mut(),
        &advertised_refs,
        &capabilities,
        &local_refs,
        &wanted_refs,
        &fetch_opts,
        Some(&pack_dir),
    )?;

    // Update remote-tracking refs
    let mapped = git_protocol::remote::map_refs(&advertised_refs, &refspecs);
    for (oid, _source, dest) in &mapped {
        if !dest.is_empty() {
            let ref_name = RefName::new(BString::from(dest.as_str()))?;
            let is_new = repo.refs().resolve(&ref_name)?.is_none();
            repo.refs().write_ref(&ref_name, oid)?;
            if !args.quiet {
                let short_dest = dest.strip_prefix("refs/remotes/").unwrap_or(dest);
                if is_new {
                    writeln!(err, " * [new branch]      {} -> {}", _source, short_dest)?;
                }
            }
        }
    }

    // Handle tags
    if args.tags {
        for (oid, name) in &advertised_refs {
            let n = name.to_str_lossy();
            if n.starts_with("refs/tags/") {
                let ref_name = RefName::new(name.clone())?;
                if repo.refs().resolve(&ref_name)?.is_none() {
                    repo.refs().write_ref(&ref_name, oid)?;
                    if !args.quiet {
                        let short = n.strip_prefix("refs/tags/").unwrap_or(&n);
                        writeln!(err, " * [new tag]         {}", short)?;
                    }
                }
            }
        }
    }

    // Prune refs that no longer exist on remote
    if args.prune {
        let remote_ref_names: std::collections::HashSet<String> = advertised_refs
            .iter()
            .filter_map(|(_, name)| {
                let n = name.to_str_lossy();
                refspecs.iter()
                    .find_map(|rs| rs.map_to_destination(&n))
            })
            .collect();

        let prefix = format!("refs/remotes/{}/", remote_name);
        if let Ok(iter) = repo.refs().iter(Some(&prefix)) {
            for r in iter {
                if let Ok(r) = r {
                    let name = r.name().as_str().to_string();
                    if !remote_ref_names.contains(&name) {
                        let ref_name = RefName::new(BString::from(name.as_str()))?;
                        repo.refs().delete_ref(&ref_name)?;
                        if !args.quiet {
                            let short = name.strip_prefix("refs/remotes/").unwrap_or(&name);
                            writeln!(err, " - [deleted]         {} -> {}", remote_name, short)?;
                        }
                    }
                }
            }
        }
    }

    Ok(0)
}

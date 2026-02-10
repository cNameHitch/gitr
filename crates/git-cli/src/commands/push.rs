use std::io::{self, Write};

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice};
use clap::Args;
use git_hash::ObjectId;
use git_config::types::PushDefault;
use git_protocol::push::{PushUpdate, PushOptions as ProtoPushOptions, PushRefResult};
use git_protocol::remote::RemoteConfig;
use git_ref::{RefName, RefStore};
use git_transport::Service;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct PushArgs {
    /// Force push
    #[arg(short, long)]
    force: bool,

    /// Force push with lease (safer force push)
    #[arg(long)]
    force_with_lease: bool,

    /// Delete remote branches
    #[arg(short, long)]
    delete: bool,

    /// Push tags
    #[arg(long)]
    tags: bool,

    /// Set upstream tracking
    #[arg(short = 'u', long = "set-upstream")]
    set_upstream: bool,

    /// Atomic push
    #[arg(long)]
    atomic: bool,

    /// Dry run
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Push option to transmit
    #[arg(short = 'o', long = "push-option")]
    push_option: Vec<String>,

    /// Skip pre-push hook
    #[arg(long)]
    no_verify: bool,

    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Show progress
    #[arg(long)]
    progress: bool,

    /// Push all branches
    #[arg(long)]
    all: bool,

    /// Mirror all refs
    #[arg(long)]
    mirror: bool,

    /// Use thin pack transfer
    #[arg(long)]
    thin: bool,

    /// Don't use thin pack transfer
    #[arg(long)]
    no_thin: bool,

    /// GPG sign the push
    #[arg(long, value_name = "mode")]
    signed: Option<String>,

    /// Recurse into submodules
    #[arg(long, value_name = "check|on-demand|only|no")]
    recurse_submodules: Option<String>,

    /// Remote name
    remote: Option<String>,

    /// Refspecs to push
    refspec: Vec<String>,
}

pub fn run(args: &PushArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Resolve remote name
    let remote_name = if let Some(ref name) = args.remote {
        name.clone()
    } else {
        // Check branch.<current>.remote config
        if let Ok(Some(branch)) = repo.current_branch() {
            let key = format!("branch.{}.remote", branch);
            repo.config().get_string(&key)?
                .unwrap_or_else(|| "origin".to_string())
        } else {
            "origin".to_string()
        }
    };

    // Read remote config
    let remote_config = RemoteConfig::from_config(repo.config(), &remote_name)?
        .ok_or_else(|| anyhow::anyhow!("fatal: '{}' does not appear to be a git repository", remote_name))?;

    // Resolve push URL
    let push_url_str = remote_config.push_url();
    let url = git_transport::GitUrl::parse(push_url_str)?;

    // Connect to remote
    let mut transport = git_transport::connect(&url, Service::ReceivePack)?;

    // Read ref advertisement
    let reader = &mut git_protocol::pktline::PktLineReader::new(transport.reader());
    let (advertised_refs, capabilities) = git_protocol::v1::parse_ref_advertisement(reader)?;

    // Resolve refspecs to push
    let updates = resolve_push_updates(&repo, &remote_name, args, &advertised_refs)?;

    if updates.is_empty() {
        writeln!(err, "Everything up-to-date")?;
        return Ok(0);
    }

    if args.dry_run {
        for update in &updates {
            writeln!(err, "Would push {} -> {}",
                update.local_oid.map(|o| o.to_hex()).unwrap_or_else(|| "(delete)".to_string()),
                update.remote_ref)?;
        }
        return Ok(0);
    }

    // Compute objects to send
    let local_oids: Vec<ObjectId> = updates.iter()
        .filter_map(|u| u.local_oid)
        .collect();
    let remote_oids: Vec<ObjectId> = advertised_refs.iter()
        .map(|(oid, _)| *oid)
        .collect();
    let objects_to_send = git_protocol::push::compute_push_objects(&local_oids, &remote_oids);

    // Build thin pack data
    let pack_data = if objects_to_send.is_empty() {
        Vec::new()
    } else {
        build_pack_data(&repo, &objects_to_send)?
    };

    let push_opts = ProtoPushOptions {
        progress: args.verbose || args.progress,
        atomic: args.atomic,
        push_options: args.push_option.clone(),
        thin: true,
    };

    let result = git_protocol::push::push(
        transport.as_mut(),
        &advertised_refs,
        &capabilities,
        &updates,
        &pack_data,
        &push_opts,
    )?;

    // Report results
    for (refname, status) in &result.ref_results {
        match status {
            PushRefResult::Ok => {
                if args.verbose {
                    writeln!(err, "   {} -> {} (ok)", refname, refname)?;
                }
            }
            PushRefResult::Rejected(reason) => {
                writeln!(err, " ! [rejected]        {} -> {} ({})", refname, refname, reason)?;
            }
            PushRefResult::Error(msg) => {
                writeln!(err, " ! [error]           {} -> {} ({})", refname, refname, msg)?;
            }
        }
    }

    // Set upstream if requested
    if args.set_upstream {
        if let Ok(Some(branch)) = repo.current_branch() {
            set_upstream_config(&repo, &branch, &remote_name)?;
            writeln!(err, "branch '{}' set up to track '{}/{}'.", branch, remote_name, branch)?;
        }
    }

    if result.ok {
        if !args.verbose {
            let remote_display = push_url_str;
            writeln!(err, "To {}", remote_display)?;
            for update in &updates {
                let local = update.local_oid
                    .map(|o| format!("{}..{}", &o.to_hex()[..7], &o.to_hex()[..7]))
                    .unwrap_or_else(|| "[deleted]".to_string());
                writeln!(err, "   {}  {} -> {}", local, update.remote_ref, update.remote_ref)?;
            }
        }
        Ok(0)
    } else {
        bail!("failed to push some refs to '{}'", push_url_str);
    }
}

fn resolve_push_updates(
    repo: &git_repository::Repository,
    _remote_name: &str,
    args: &PushArgs,
    advertised_refs: &[(ObjectId, BString)],
) -> Result<Vec<PushUpdate>> {
    let mut updates = Vec::new();

    if !args.refspec.is_empty() {
        // Explicit refspecs
        for spec in &args.refspec {
            if args.delete || spec.starts_with(':') {
                // Delete refspec
                let remote_ref = spec.trim_start_matches(':');
                let remote_full = if remote_ref.starts_with("refs/") {
                    remote_ref.to_string()
                } else {
                    format!("refs/heads/{}", remote_ref)
                };
                updates.push(PushUpdate {
                    local_oid: None,
                    remote_ref: remote_full,
                    force: args.force,
                    expected_remote_oid: None,
                });
            } else if let Some((src, dst)) = spec.split_once(':') {
                let local_ref = if src.starts_with("refs/") {
                    src.to_string()
                } else {
                    format!("refs/heads/{}", src)
                };
                let remote_ref = if dst.starts_with("refs/") {
                    dst.to_string()
                } else {
                    format!("refs/heads/{}", dst)
                };
                let oid = resolve_ref_oid(repo, &local_ref)?;
                updates.push(PushUpdate {
                    local_oid: Some(oid),
                    remote_ref,
                    force: args.force,
                    expected_remote_oid: if args.force_with_lease {
                        find_remote_oid(advertised_refs, &local_ref)
                    } else {
                        None
                    },
                });
            } else {
                // Same source and destination
                let refname = if spec.starts_with("refs/") {
                    spec.to_string()
                } else {
                    format!("refs/heads/{}", spec)
                };
                let oid = resolve_ref_oid(repo, &refname)?;
                updates.push(PushUpdate {
                    local_oid: Some(oid),
                    remote_ref: refname,
                    force: args.force,
                    expected_remote_oid: None,
                });
            }
        }
    } else {
        // Use push.default config
        let push_default = repo.config().get_string("push.default")?
            .and_then(|v| PushDefault::from_config(&v).ok())
            .unwrap_or(PushDefault::Simple);

        match push_default {
            PushDefault::Nothing => {
                bail!("fatal: No configured push destination.\nSpecify the remote and refspec.");
            }
            PushDefault::Current => {
                if let Some(branch) = repo.current_branch()? {
                    let refname = format!("refs/heads/{}", branch);
                    let oid = resolve_ref_oid(repo, &refname)?;
                    updates.push(PushUpdate {
                        local_oid: Some(oid),
                        remote_ref: refname,
                        force: args.force,
                        expected_remote_oid: None,
                    });
                }
            }
            PushDefault::Upstream | PushDefault::Simple => {
                if let Some(branch) = repo.current_branch()? {
                    let local_ref = format!("refs/heads/{}", branch);
                    let remote_ref = if push_default == PushDefault::Simple {
                        local_ref.clone()
                    } else {
                        // Check branch.<name>.merge config
                        let merge_key = format!("branch.{}.merge", branch);
                        repo.config().get_string(&merge_key)?
                            .unwrap_or_else(|| local_ref.clone())
                    };
                    let oid = resolve_ref_oid(repo, &local_ref)?;
                    updates.push(PushUpdate {
                        local_oid: Some(oid),
                        remote_ref,
                        force: args.force,
                        expected_remote_oid: None,
                    });
                }
            }
            PushDefault::Matching => {
                // Push all branches that have a matching remote branch
                if let Ok(iter) = repo.refs().iter(Some("refs/heads/")) {
                    for r in iter.flatten() {
                        let name = r.name().as_str().to_string();
                        if find_remote_oid(advertised_refs, &name).is_some() {
                            if let Some(oid) = r.target_oid() {
                                updates.push(PushUpdate {
                                    local_oid: Some(oid),
                                    remote_ref: name,
                                    force: args.force,
                                    expected_remote_oid: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Add tags if requested
    if args.tags {
        if let Ok(iter) = repo.refs().iter(Some("refs/tags/")) {
            for r in iter.flatten() {
                if let Some(oid) = r.target_oid() {
                    let name = r.name().as_str().to_string();
                    if find_remote_oid(advertised_refs, &name).is_none() {
                        updates.push(PushUpdate {
                            local_oid: Some(oid),
                            remote_ref: name,
                            force: false,
                            expected_remote_oid: None,
                        });
                    }
                }
            }
        }
    }

    Ok(updates)
}

fn resolve_ref_oid(repo: &git_repository::Repository, refname: &str) -> Result<ObjectId> {
    let name = RefName::new(BString::from(refname))?;
    repo.refs().resolve_to_oid(&name)?
        .ok_or_else(|| anyhow::anyhow!("src refspec {} does not match any", refname))
}

fn find_remote_oid(advertised_refs: &[(ObjectId, BString)], refname: &str) -> Option<ObjectId> {
    advertised_refs.iter()
        .find(|(_, name)| name.to_str_lossy() == refname)
        .map(|(oid, _)| *oid)
}

fn set_upstream_config(repo: &git_repository::Repository, branch: &str, remote: &str) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let mut content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let section = format!("[branch \"{}\"]", branch);
    if !content.contains(&section) {
        content.push_str(&format!(
            "\n{}\n\tremote = {}\n\tmerge = refs/heads/{}\n",
            section, remote, branch
        ));
    }
    std::fs::write(&config_path, content)?;
    Ok(())
}

fn build_pack_data(
    repo: &git_repository::Repository,
    objects: &[ObjectId],
) -> Result<Vec<u8>> {
    use std::io::Write;
    use std::collections::HashSet;

    // Walk the full object graph from each commit OID to collect all reachable objects
    let mut all_oids = Vec::new();
    let mut seen = HashSet::new();

    fn walk_tree(
        odb: &git_odb::ObjectDatabase,
        oid: &ObjectId,
        all_oids: &mut Vec<ObjectId>,
        seen: &mut HashSet<ObjectId>,
    ) -> Result<()> {
        if !seen.insert(*oid) {
            return Ok(());
        }
        all_oids.push(*oid);
        if let Some(git_object::Object::Tree(tree)) = odb.read(oid)? {
            for entry in tree.iter() {
                walk_tree(odb, &entry.oid, all_oids, seen)?;
            }
        }
        Ok(())
    }

    for oid in objects {
        if !seen.insert(*oid) {
            continue;
        }
        all_oids.push(*oid);
        if let Some(git_object::Object::Commit(c)) = repo.odb().read(oid)? {
            walk_tree(repo.odb(), &c.tree, &mut all_oids, &mut seen)?;
        }
    }

    // Build a minimal pack containing the needed objects
    let mut pack = Vec::new();
    // Pack header: PACK, version 2, num objects
    pack.extend_from_slice(b"PACK");
    pack.extend_from_slice(&2u32.to_be_bytes());
    pack.extend_from_slice(&(all_oids.len() as u32).to_be_bytes());

    for oid in &all_oids {
        if let Some(obj) = repo.odb().read(oid)? {
            let content = obj.serialize_content();
            let obj_type_num: u8 = match obj.object_type() {
                git_object::ObjectType::Commit => 1,
                git_object::ObjectType::Tree => 2,
                git_object::ObjectType::Blob => 3,
                git_object::ObjectType::Tag => 4,
            };

            // Variable-length size encoding
            let size = content.len();
            let mut header_byte = (obj_type_num << 4) | (size as u8 & 0x0f);
            let mut remaining = size >> 4;
            if remaining > 0 {
                header_byte |= 0x80;
            }
            pack.push(header_byte);
            while remaining > 0 {
                let mut byte = (remaining & 0x7f) as u8;
                remaining >>= 7;
                if remaining > 0 {
                    byte |= 0x80;
                }
                pack.push(byte);
            }

            // Compress the content
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&content)?;
            let compressed = encoder.finish()?;
            pack.extend_from_slice(&compressed);
        }
    }

    // Append SHA1 checksum of pack
    let checksum = git_hash::hasher::Hasher::digest(git_hash::HashAlgorithm::Sha1, &pack)
        .map_err(|e| anyhow::anyhow!("hash error: {}", e))?;
    pack.extend_from_slice(checksum.as_bytes());

    Ok(pack)
}

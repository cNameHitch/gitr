use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use bstr::{BString, ByteSlice, ByteVec};
use clap::Args;
use git_hash::ObjectId;
use git_index::{IndexEntry, Stage, StatData, EntryFlags};
use git_object::{FileMode, Object};
use git_protocol::remote::RefSpec;
use git_ref::RefName;
use git_repository::{InitOptions, Repository};
use git_transport::{GitUrl, Service};

use crate::Cli;

#[derive(Args)]
pub struct CloneArgs {
    /// Create a shallow clone with a history truncated to the specified number of commits
    #[arg(long)]
    depth: Option<u32>,

    /// Checkout the specified branch instead of the remote HEAD
    #[arg(short, long, value_name = "name")]
    branch: Option<String>,

    /// Make a bare Git repository
    #[arg(long)]
    bare: bool,

    /// Be quiet, only report errors
    #[arg(short, long)]
    quiet: bool,

    /// Repository URL
    repository: String,

    /// Destination directory
    dest_dir: Option<String>,
}

pub fn run(args: &CloneArgs, _cli: &Cli) -> Result<i32> {
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Determine destination directory
    let dest = match &args.dest_dir {
        Some(d) => PathBuf::from(d),
        None => infer_directory(&args.repository)?,
    };

    if dest.exists() && std::fs::read_dir(&dest)?.next().is_some() {
        bail!(
            "fatal: destination path '{}' already exists and is not an empty directory.",
            dest.display()
        );
    }

    if !args.quiet {
        writeln!(err, "Cloning into '{}'...", dest.display())?;
    }

    // Parse URL and connect
    let url = GitUrl::parse(&args.repository)?;
    let mut transport = git_transport::connect(&url, Service::UploadPack)?;

    // Read ref advertisement
    let reader = &mut git_protocol::pktline::PktLineReader::new(transport.reader());
    let (advertised_refs, capabilities) = git_protocol::v1::parse_ref_advertisement(reader)?;

    if advertised_refs.is_empty() {
        if !args.quiet {
            writeln!(err, "warning: You appear to have cloned an empty repository.")?;
        }
    }

    // Initialize the destination repository
    let opts = InitOptions {
        bare: args.bare,
        ..Default::default()
    };
    let repo = Repository::init_opts(&dest, &opts)?;

    // Configure remote "origin"
    write_remote_config(&repo, &args.repository)?;

    // Determine which refs to fetch
    let fetch_refspec = RefSpec::parse("+refs/heads/*:refs/remotes/origin/*")?;
    let wanted_refs: Vec<String> = advertised_refs
        .iter()
        .filter(|(_, name)| {
            let n = name.to_str_lossy();
            n.starts_with("refs/heads/") || n.starts_with("refs/tags/")
        })
        .map(|(_, name)| name.to_str_lossy().to_string())
        .collect();

    if !advertised_refs.is_empty() {
        // Collect local refs (empty for a fresh clone)
        let local_refs: Vec<(ObjectId, String)> = Vec::new();

        let fetch_opts = git_protocol::fetch::FetchOptions {
            depth: args.depth,
            filter: None,
            progress: !args.quiet,
        };

        let pack_dir = repo.common_dir().join("objects").join("pack");
        std::fs::create_dir_all(&pack_dir)?;

        let _fetch_result = git_protocol::fetch::fetch(
            transport.as_mut(),
            &advertised_refs,
            &capabilities,
            &local_refs,
            &wanted_refs,
            &fetch_opts,
            Some(&pack_dir),
        )?;

        // Re-open the repository so the ODB discovers the new pack files
        drop(repo);
        let mut repo = Repository::open(&dest)?;

        // Determine which branch to checkout
        let checkout_branch = determine_checkout_branch(
            args.branch.as_deref(),
            &advertised_refs,
            &capabilities,
        );

        if args.bare {
            // Bare clone: store refs/heads/* directly (no remote-tracking refs)
            for (oid, refname) in &advertised_refs {
                let name = refname.to_str_lossy();
                if name.starts_with("refs/heads/") || name.starts_with("refs/tags/") {
                    let ref_name = RefName::new(refname.clone())?;
                    repo.refs().write_ref(&ref_name, oid)?;
                }
            }

            // Set HEAD
            if let Some((branch_name, _oid)) = checkout_branch {
                let head_ref = RefName::new(BString::from("HEAD"))?;
                let branch_ref = RefName::new(BString::from(format!("refs/heads/{}", branch_name)))?;
                repo.refs().write_symbolic_ref(&head_ref, &branch_ref)?;
            }
        } else {
            // Non-bare clone: create remote-tracking refs
            for (oid, refname) in &advertised_refs {
                let name = refname.to_str_lossy();
                if let Some(dest_ref) = fetch_refspec.map_to_destination(&name) {
                    let ref_name = RefName::new(BString::from(dest_ref.as_str()))?;
                    repo.refs().write_ref(&ref_name, oid)?;
                }
                // Also store tags directly
                if name.starts_with("refs/tags/") {
                    let ref_name = RefName::new(refname.clone())?;
                    repo.refs().write_ref(&ref_name, oid)?;
                }
            }

            // Create refs/remotes/origin/HEAD symbolic ref
            if let Some((ref branch_name, _)) = checkout_branch {
                let remote_head = RefName::new(BString::from("refs/remotes/origin/HEAD"))?;
                let remote_branch = RefName::new(BString::from(format!("refs/remotes/origin/{}", branch_name)))?;
                repo.refs().write_symbolic_ref(&remote_head, &remote_branch)?;
            }

            if let Some((branch_name, oid)) = checkout_branch {
                // Set HEAD to point to the branch
                let head_ref = RefName::new(BString::from("HEAD"))?;
                let branch_ref = RefName::new(BString::from(format!("refs/heads/{}", branch_name)))?;
                repo.refs().write_symbolic_ref(&head_ref, &branch_ref)?;

                // Create the local branch ref pointing to the same commit
                repo.refs().write_ref(&branch_ref, &oid)?;

                // Checkout the working tree
                checkout_tree(&mut repo, &oid)?;
            }
        }
    } else {
        // Empty repo: just set HEAD to default branch
        let head_ref = RefName::new(BString::from("HEAD"))?;
        let branch_ref = RefName::new(BString::from("refs/heads/main"))?;
        repo.refs().write_symbolic_ref(&head_ref, &branch_ref)?;
    }

    Ok(0)
}

fn infer_directory(url_str: &str) -> Result<PathBuf> {
    let path = url_str
        .rsplit('/')
        .next()
        .unwrap_or(url_str)
        .trim_end_matches(".git");
    if path.is_empty() {
        bail!("cannot infer directory name from '{}'", url_str);
    }
    Ok(PathBuf::from(path))
}

fn write_remote_config(repo: &Repository, url: &str) -> Result<()> {
    let config_path = repo.git_dir().join("config");
    let mut content = std::fs::read_to_string(&config_path).unwrap_or_default();
    content.push_str(&format!(
        "\n[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n",
        url
    ));
    std::fs::write(&config_path, content)?;
    Ok(())
}

fn determine_checkout_branch(
    requested: Option<&str>,
    advertised_refs: &[(ObjectId, BString)],
    capabilities: &git_protocol::capability::Capabilities,
) -> Option<(String, ObjectId)> {
    // If user requested a specific branch
    if let Some(branch) = requested {
        let full_ref = format!("refs/heads/{}", branch);
        for (oid, name) in advertised_refs {
            if name.to_str_lossy() == full_ref {
                return Some((branch.to_string(), *oid));
            }
        }
        return None;
    }

    // Try to find HEAD's target via symref capability
    if let Some(symref) = capabilities.get("symref") {
        // Format: symref=HEAD:refs/heads/main
        if let Some(target) = symref.strip_prefix("HEAD:refs/heads/") {
            let branch = target.to_string();
            for (oid, name) in advertised_refs {
                if name.to_str_lossy() == format!("refs/heads/{}", branch) {
                    return Some((branch, *oid));
                }
            }
        }
    }

    // Fall back to HEAD ref
    for (oid, name) in advertised_refs {
        if name.to_str_lossy() == "HEAD" {
            // Try to match HEAD oid to a branch
            for (branch_oid, branch_name) in advertised_refs {
                let bn = branch_name.to_str_lossy();
                if bn.starts_with("refs/heads/") && branch_oid == oid {
                    let short = bn.strip_prefix("refs/heads/").unwrap();
                    return Some((short.to_string(), *oid));
                }
            }
            // Detached HEAD: use "main" as branch name
            return Some(("main".to_string(), *oid));
        }
    }

    None
}

fn checkout_tree(repo: &mut Repository, commit_oid: &ObjectId) -> Result<()> {
    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("cannot checkout: bare repository"))?
        .to_path_buf();

    // Read the commit to get its tree
    let obj = repo
        .odb()
        .read(commit_oid)?
        .ok_or_else(|| anyhow::anyhow!("commit {} not found", commit_oid.to_hex()))?;

    let tree_oid = match obj {
        Object::Commit(c) => c.tree,
        _ => bail!("expected commit, got {}", obj.object_type()),
    };

    // Recursively checkout the tree
    let mut index_entries = Vec::new();
    checkout_tree_recursive(repo.odb(), &tree_oid, &work_tree, &BString::from(""), &mut index_entries)?;

    // Build and write the index
    let mut index = git_index::Index::new();
    for entry in index_entries {
        index.add(entry);
    }
    let index_path = repo.git_dir().join("index");
    index.write_to(&index_path)?;
    repo.set_index(index);

    Ok(())
}

fn checkout_tree_recursive(
    odb: &git_odb::ObjectDatabase,
    tree_oid: &ObjectId,
    work_tree: &Path,
    prefix: &BString,
    entries: &mut Vec<IndexEntry>,
) -> Result<()> {
    let obj = odb
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree {} not found", tree_oid.to_hex()))?;

    let tree = match obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree, got {}", obj.object_type()),
    };

    for entry in tree.iter() {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            let mut p = prefix.clone();
            p.push_byte(b'/');
            p.extend_from_slice(&entry.name);
            p
        };

        if entry.mode.is_tree() {
            // Create directory and recurse
            let dir_path = work_tree.join(path.to_str_lossy().as_ref());
            std::fs::create_dir_all(&dir_path)?;
            checkout_tree_recursive(odb, &entry.oid, work_tree, &path, entries)?;
        } else {
            // Write file to working tree
            let file_path = work_tree.join(path.to_str_lossy().as_ref());
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let blob_obj = odb
                .read(&entry.oid)?
                .ok_or_else(|| anyhow::anyhow!("blob {} not found", entry.oid.to_hex()))?;

            let data = match blob_obj {
                Object::Blob(b) => b.data,
                _ => bail!("expected blob for {}", path.to_str_lossy()),
            };

            std::fs::write(&file_path, &data)?;

            // Set executable permission if needed
            #[cfg(unix)]
            if entry.mode == FileMode::Executable {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&file_path, perms)?;
            }

            // Create symlink if needed
            if entry.mode == FileMode::Symlink {
                // Remove the file we just wrote and create a symlink instead
                std::fs::remove_file(&file_path)?;
                #[cfg(unix)]
                {
                    let target = String::from_utf8_lossy(&data);
                    std::os::unix::fs::symlink(target.as_ref(), &file_path)?;
                }
            }

            // Build index entry
            let metadata = std::fs::symlink_metadata(&file_path)?;
            entries.push(IndexEntry {
                path,
                oid: entry.oid,
                mode: entry.mode,
                stage: Stage::Normal,
                stat: StatData::from_metadata(&metadata),
                flags: EntryFlags::default(),
            });
        }
    }

    Ok(())
}

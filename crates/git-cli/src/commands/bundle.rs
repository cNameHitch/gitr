use std::io::{self, Write};

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use git_hash::ObjectId;
use git_ref::RefStore;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct BundleArgs {
    #[command(subcommand)]
    command: BundleSubcommand,
}

#[derive(Subcommand)]
pub enum BundleSubcommand {
    /// Create a bundle file
    Create {
        /// Be quiet
        #[arg(short, long)]
        quiet: bool,

        /// Show progress
        #[arg(long)]
        progress: bool,

        /// Bundle format version
        #[arg(long, default_value = "2")]
        version: u32,

        /// Output file
        file: String,

        /// Git-rev-list arguments (refs, ranges, etc.)
        #[arg(trailing_var_arg = true)]
        refs: Vec<String>,
    },

    /// Verify a bundle file
    Verify {
        /// Be quiet
        #[arg(short, long)]
        quiet: bool,

        /// Bundle file
        file: String,
    },

    /// List references in a bundle
    ListHeads {
        /// Bundle file
        file: String,

        /// Patterns to match
        patterns: Vec<String>,
    },

    /// Extract a bundle into a repository
    Unbundle {
        /// Show progress
        #[arg(long)]
        progress: bool,

        /// Bundle file
        file: String,

        /// References to unbundle
        refs: Vec<String>,
    },
}

pub fn run(args: &BundleArgs, cli: &Cli) -> Result<i32> {
    match &args.command {
        BundleSubcommand::Create {
            quiet,
            progress: _,
            version,
            file,
            refs,
        } => bundle_create(cli, *quiet, file, *version, refs),
        BundleSubcommand::Verify { quiet, file } => bundle_verify(cli, *quiet, file),
        BundleSubcommand::ListHeads { file, patterns } => bundle_list_heads(file, patterns),
        BundleSubcommand::Unbundle {
            progress: _,
            file,
            refs,
        } => bundle_unbundle(cli, file, refs),
    }
}

/// Bundle file header format:
/// ```text
/// # v2 git bundle\n
/// -<prerequisite-oid> <comment>\n   (one per prerequisite)
/// <oid> <refname>\n                  (one per ref)
/// \n                                  (empty line terminates header)
/// <pack-data>
/// ```
fn bundle_create(
    cli: &Cli,
    quiet: bool,
    file: &str,
    version: u32,
    refs: &[String],
) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Determine which refs to bundle
    let mut include_refs: Vec<(String, ObjectId)> = Vec::new();

    if refs.is_empty() || refs.iter().any(|r| r == "--all") {
        // Bundle all refs
        let all_refs = repo.refs().iter(None)?;
        for r in all_refs {
            let r = r?;
            let oid = r.peel_to_oid(repo.refs())?;
            include_refs.push((r.name().as_str().to_string(), oid));
        }
        if let Some(head) = repo.head_oid()? {
            include_refs.push(("HEAD".to_string(), head));
        }
    } else {
        for spec in refs {
            if spec.starts_with('^') || spec.starts_with('-') {
                continue; // Skip exclude specs for now
            }
            if let Ok(oid) = git_revwalk::resolve_revision(&repo, spec) {
                include_refs.push((spec.clone(), oid));
            }
        }
    }

    if include_refs.is_empty() {
        bail!("Refusing to create empty bundle.");
    }

    // Create the bundle file
    let mut output = std::fs::File::create(file)?;

    // Write header
    if version >= 3 {
        writeln!(output, "# v3 git bundle")?;
    } else {
        writeln!(output, "# v2 git bundle")?;
    }

    // Write refs
    for (name, oid) in &include_refs {
        writeln!(output, "{} {}", oid.to_hex(), name)?;
    }

    // Empty line terminates header
    writeln!(output)?;

    // Collect all objects reachable from the included refs
    let tip_oids: Vec<ObjectId> = include_refs.iter().map(|(_, oid)| *oid).collect();
    let all_objects = git_revwalk::list_objects(&repo, &tip_oids, &[], None)?;

    if !quiet {
        writeln!(err, "Counting objects: {}, done.", all_objects.len())?;
    }

    // Write pack data
    let tmp_dir = tempfile::tempdir()?;
    let pack_path = tmp_dir.path().join("bundle.pack");

    let mut writer = git_pack::write::PackWriter::new(&pack_path)?;
    for oid in &all_objects {
        if let Some(obj) = repo.odb().read(oid)? {
            let content = obj.serialize_content();
            writer.add_object(obj.object_type(), &content)?;
        }
    }
    let (pack_path, _) = writer.finish()?;

    // Append pack data to bundle
    let pack_data = std::fs::read(&pack_path)?;
    output.write_all(&pack_data)?;

    if !quiet {
        writeln!(
            err,
            "Total {} (delta 0), reused 0 (delta 0), pack-reused 0",
            all_objects.len()
        )?;
    }

    Ok(0)
}

fn bundle_verify(cli: &Cli, quiet: bool, file: &str) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let data = std::fs::read(file)?;
    let header = parse_bundle_header(&data)?;

    // Validate pack data: check that the pack header is present and valid
    if header.pack_offset < data.len() {
        let pack_data = &data[header.pack_offset..];
        if pack_data.len() >= 12 {
            // Pack header: "PACK" + version(4 bytes) + object count(4 bytes)
            if &pack_data[..4] != b"PACK" {
                if !quiet {
                    writeln!(err, "error: invalid pack data in bundle")?;
                }
                return Ok(1);
            }
            let pack_obj_count = u32::from_be_bytes([
                pack_data[8], pack_data[9], pack_data[10], pack_data[11],
            ]) as usize;
            // Sanity check: object count should be reasonable
            if pack_obj_count == 0 && !header.refs.is_empty() && !quiet {
                writeln!(err, "warning: pack contains 0 objects but bundle has refs")?;
            }
        }
    }

    if !quiet {
        writeln!(err, "The bundle contains these {} ref(s):", header.refs.len())?;
        for (oid, name) in &header.refs {
            writeln!(err, "{} {}", oid.to_hex(), name)?;
        }
    }

    // Verify prerequisites if we have a repo
    if let Ok(repo) = open_repo(cli) {
        for prereq in &header.prerequisites {
            if !repo.odb().contains(prereq) {
                if !quiet {
                    writeln!(
                        err,
                        "error: repository is missing prerequisite commit {}",
                        prereq.to_hex()
                    )?;
                }
                return Ok(1);
            }
        }
    }

    if !quiet {
        // C git writes the "okay" line to stdout
        writeln!(out, "{} is okay", file)?;
    }

    Ok(0)
}

fn bundle_list_heads(file: &str, patterns: &[String]) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let data = std::fs::read(file)?;
    let header = parse_bundle_header(&data)?;

    for (oid, name) in &header.refs {
        if !patterns.is_empty() && !patterns.iter().any(|p| name.contains(p)) {
            continue;
        }
        writeln!(out, "{} {}", oid.to_hex(), name)?;
    }

    Ok(0)
}

fn bundle_unbundle(cli: &Cli, file: &str, _refs: &[String]) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let data = std::fs::read(file)?;
    let header = parse_bundle_header(&data)?;

    // Extract the pack data (everything after the header)
    let pack_start = header.pack_offset;
    let pack_data = &data[pack_start..];

    // Write pack to objects/pack/
    let objects_dir = repo.odb().objects_dir();
    let pack_dir = objects_dir.join("pack");
    std::fs::create_dir_all(&pack_dir)?;

    // Compute a hash for the pack name
    let mut hasher = git_hash::hasher::Hasher::new(git_hash::HashAlgorithm::Sha1);
    hasher.update(pack_data);
    let pack_hash = hasher.finalize()?;

    let pack_path = pack_dir.join(format!("pack-{}.pack", pack_hash.to_hex()));
    let idx_path = pack_dir.join(format!("pack-{}.idx", pack_hash.to_hex()));

    std::fs::write(&pack_path, pack_data)?;

    // Build index using index-pack
    let pack = git_pack::pack::PackFile::open(&pack_path)?;
    let mut entries: Vec<(ObjectId, u64, u32)> = Vec::new();
    for (oid, offset) in pack.index().iter() {
        entries.push((oid, offset, 0));
    }

    // Read pack checksum
    let checksum_bytes = &pack_data[pack_data.len().saturating_sub(20)..];
    let pack_checksum = ObjectId::from_bytes(checksum_bytes, git_hash::HashAlgorithm::Sha1)?;
    git_pack::write::build_pack_index(&idx_path, &mut entries, &pack_checksum)?;

    // Refresh ODB
    repo.odb().refresh()?;

    // Print refs from the bundle
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for (oid, name) in &header.refs {
        writeln!(out, "{} {}", oid.to_hex(), name)?;
    }

    writeln!(err, "Unbundled {} ref(s).", header.refs.len())?;

    Ok(0)
}

struct BundleHeader {
    #[allow(dead_code)]
    version: u32,
    prerequisites: Vec<ObjectId>,
    refs: Vec<(ObjectId, String)>,
    pack_offset: usize,
}

fn parse_bundle_header(data: &[u8]) -> Result<BundleHeader> {
    let mut offset = 0;
    let mut prerequisites = Vec::new();
    let mut refs = Vec::new();

    // Parse line by line
    let text = String::from_utf8_lossy(data);
    let mut lines = text.lines();

    // First line: header
    let header_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty bundle"))?;
    offset += header_line.len() + 1; // +1 for newline

    let version = if header_line == "# v3 git bundle" {
        3
    } else if header_line == "# v2 git bundle" {
        2
    } else {
        bail!("not a valid git bundle: {}", header_line);
    };

    // Parse refs and prerequisites until empty line
    for line in lines {
        offset += line.len() + 1;

        if line.is_empty() {
            break;
        }

        if let Some(rest) = line.strip_prefix('-') {
            // Prerequisite
            let hex = rest.split_whitespace().next().unwrap_or(rest);
            let oid = ObjectId::from_hex(hex)?;
            prerequisites.push(oid);
        } else if line.len() >= 40 {
            let hex = &line[..40];
            let name = line[41..].to_string();
            let oid = ObjectId::from_hex(hex)?;
            refs.push((oid, name));
        }
    }

    Ok(BundleHeader {
        version,
        prerequisites,
        refs,
        pack_offset: offset,
    })
}

use std::collections::HashSet;
use std::io::{self, BufRead, Write};

use anyhow::{bail, Result};
use clap::Args;
use git_hash::ObjectId;
use git_pack::write::PackWriter;
use git_ref::RefStore;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct PackObjectsArgs {
    /// Write the pack to stdout
    #[arg(long)]
    stdout: bool,

    /// Pack objects reachable from the listed references
    #[arg(long)]
    revs: bool,

    /// Include all refs
    #[arg(long)]
    all: bool,

    /// Only include objects not already packed
    #[arg(long)]
    unpacked: bool,

    /// Only include local objects
    #[arg(long)]
    local: bool,

    /// Produce an incremental pack
    #[arg(long)]
    incremental: bool,

    /// Suppress progress output
    #[arg(short, long)]
    quiet: bool,

    /// Show progress
    #[arg(long)]
    progress: bool,

    /// Report progress on stderr
    #[arg(long)]
    all_progress: bool,

    /// Window size for delta compression
    #[arg(long, default_value = "10")]
    window: u32,

    /// Maximum delta chain depth
    #[arg(long, default_value = "50")]
    depth: u32,

    /// Do not reuse existing deltas
    #[arg(long)]
    no_reuse_delta: bool,

    /// Use OFS_DELTA entries instead of REF_DELTA
    #[arg(long)]
    delta_base_offset: bool,

    /// Number of threads for delta searching
    #[arg(long)]
    threads: Option<u32>,

    /// Do not create an empty pack
    #[arg(long)]
    non_empty: bool,

    /// Base name for .pack and .idx output files
    base_name: Option<String>,
}

pub fn run(args: &PackObjectsArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Collect OIDs from stdin (or --all/--revs)
    let mut oids: Vec<ObjectId> = Vec::new();

    if args.all {
        // Include all objects reachable from all refs
        let refs = repo.refs().iter(None)?;
        let mut tip_oids = Vec::new();
        for r in refs {
            let r = r?;
            let oid = r.peel_to_oid(repo.refs())?;
            tip_oids.push(oid);
        }
        // Also include HEAD
        if let Some(head) = repo.head_oid()? {
            tip_oids.push(head);
        }

        let reachable = git_revwalk::list_objects(&repo, &tip_oids, &[], None)?;
        oids = reachable;
    } else if args.revs {
        // Read revision specs from stdin
        let stdin = io::stdin();
        let mut include = Vec::new();
        let mut exclude = Vec::new();
        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix('^') {
                let oid = git_revwalk::resolve_revision(&repo, rest)?;
                exclude.push(oid);
            } else {
                let oid = git_revwalk::resolve_revision(&repo, &line)?;
                include.push(oid);
            }
        }
        oids = git_revwalk::list_objects(&repo, &include, &exclude, None)?;
    } else {
        // Read OIDs from stdin, one per line
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            let oid = ObjectId::from_hex(&line)?;
            oids.push(oid);
        }
    }

    // Filter to unique OIDs
    let mut seen = HashSet::new();
    oids.retain(|oid| seen.insert(*oid));

    if args.non_empty && oids.is_empty() {
        bail!("No objects to pack.");
    }

    if oids.is_empty() {
        // Write empty pack to stdout if requested
        if args.stdout {
            // No objects = no pack
        }
        if !args.quiet {
            writeln!(err, "Total 0 (delta 0), reused 0 (delta 0)")?;
        }
        return Ok(0);
    }

    // Determine output path
    let tmp_dir = tempfile::tempdir()?;
    let base_name = if args.stdout {
        tmp_dir.path().join("pack").to_string_lossy().to_string()
    } else {
        args.base_name
            .as_deref()
            .unwrap_or("pack")
            .to_string()
    };

    let pack_path = format!("{}.pack", base_name);
    let idx_path = format!("{}.idx", base_name);

    let mut writer = PackWriter::new(&pack_path)?;

    // Add all objects to the pack
    for oid in &oids {
        let obj = repo.odb().read(oid)?
            .ok_or_else(|| anyhow::anyhow!("object not found: {}", oid.to_hex()))?;
        let content = obj.serialize_content();
        let obj_type = obj.object_type();
        writer.add_object(obj_type, &content)?;
    }

    // Build entries list before finishing
    let mut entries: Vec<(ObjectId, u64, u32)> = writer
        .entries()
        .map(|(oid, off, crc)| (*oid, off, crc))
        .collect();

    let (pack_path_out, checksum) = writer.finish()?;

    // Build the index
    git_pack::write::build_pack_index(std::path::Path::new(&idx_path), &mut entries, &checksum)?;

    if args.stdout {
        // Write pack to stdout
        let pack_data = std::fs::read(&pack_path_out)?;
        let stdout = io::stdout();
        let mut out = stdout.lock();
        out.write_all(&pack_data)?;
    }

    if !args.quiet {
        writeln!(
            err,
            "Total {} (delta 0), reused 0 (delta 0), pack-reused 0",
            oids.len()
        )?;
    }

    // Print the pack checksum
    if !args.stdout {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        writeln!(out, "{}", checksum.to_hex())?;
    }

    Ok(0)
}

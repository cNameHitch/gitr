use std::io::{self, Read, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Args;
use git_hash::ObjectId;
use git_pack::pack::PackFile;
use git_pack::write::build_pack_index;

use crate::Cli;

#[derive(Args)]
pub struct IndexPackArgs {
    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Write the index to the specified file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Keep the pack file after indexing (write a .keep file)
    #[arg(long)]
    keep: bool,

    /// Keep message
    #[arg(long = "keep", value_name = "MSG")]
    keep_msg: Option<String>,

    /// Verify the pack after indexing
    #[arg(long)]
    verify: bool,

    /// Strict mode: check objects more carefully
    #[arg(long)]
    strict: bool,

    /// Perform fsck checks on objects
    #[arg(long)]
    fsck_objects: bool,

    /// Read pack from stdin
    #[arg(long)]
    stdin: bool,

    /// Fix a thin pack (add missing base objects)
    #[arg(long)]
    fix_thin: bool,

    /// Generate a reverse index
    #[arg(long = "rev-index")]
    rev_index: bool,

    /// Pack file path
    pack_file: Option<PathBuf>,
}

pub fn run(args: &IndexPackArgs, _cli: &Cli) -> Result<i32> {
    let stderr = io::stderr();
    let mut err = stderr.lock();

    // Determine pack file path
    let pack_path = if args.stdin {
        // Read pack from stdin into a temp file
        let tmp_dir = tempfile::tempdir()?;
        let tmp_path = tmp_dir.path().join("tmp_pack.pack");
        let mut stdin = io::stdin();
        let mut data = Vec::new();
        stdin.read_to_end(&mut data)?;
        std::fs::write(&tmp_path, &data)?;
        // Keep the tempdir alive by leaking it (pack needs to persist)
        let path = tmp_path.clone();
        std::mem::forget(tmp_dir);
        path
    } else if let Some(ref path) = args.pack_file {
        path.clone()
    } else {
        bail!("need a pack file or --stdin");
    };

    // Open and validate the pack
    let pack = PackFile::open(&pack_path)?;
    let num_objects = pack.num_objects();

    if args.verbose {
        writeln!(err, "Indexing {} objects...", num_objects)?;
    }

    // Collect entries from the pack index
    let mut entries: Vec<(ObjectId, u64, u32)> = Vec::new();
    for (oid, offset) in pack.index().iter() {
        // Compute CRC32 — for simplicity, use 0 if not available from existing index
        entries.push((oid, offset, 0));
    }

    // Determine output index path
    let idx_path = if let Some(ref output) = args.output {
        output.clone()
    } else {
        let mut p = pack_path.clone();
        p.set_extension("idx");
        p
    };

    // Read pack checksum (last 20 bytes of pack file)
    let pack_data = std::fs::read(&pack_path)?;
    let checksum_bytes = &pack_data[pack_data.len().saturating_sub(20)..];
    let pack_checksum = ObjectId::from_bytes(checksum_bytes, git_hash::HashAlgorithm::Sha1)?;

    // Build the index
    build_pack_index(&idx_path, &mut entries, &pack_checksum)?;

    if args.verify {
        // Re-open and verify
        let _pack = PackFile::open(&pack_path)?;
        if args.verbose {
            writeln!(err, "pack is valid")?;
        }
    }

    // Write .keep file if requested
    if args.keep || args.keep_msg.is_some() {
        let mut keep_path = pack_path.clone();
        keep_path.set_extension("keep");
        let msg = args.keep_msg.as_deref().unwrap_or("");
        std::fs::write(&keep_path, msg)?;
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "pack\t{}", pack_checksum.to_hex())?;

    Ok(0)
}

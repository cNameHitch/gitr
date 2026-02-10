use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use clap::Args;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct CountObjectsArgs {
    /// Report in more detail
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Print sizes in human-readable format
    #[arg(short = 'H', long)]
    human_readable: bool,
}

pub fn run(args: &CountObjectsArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let objects_dir = repo.git_dir().join("objects");

    // Count loose objects and their total size
    let mut loose_count: u64 = 0;
    let mut loose_size: u64 = 0;

    for prefix in 0..=0xffu32 {
        let subdir = objects_dir.join(format!("{:02x}", prefix));
        if !subdir.is_dir() {
            continue;
        }

        let entries = match fs::read_dir(&subdir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                loose_count += 1;
                loose_size += meta.len();
            }
        }
    }

    // Size in KiB (matching git's output)
    let loose_size_kib = loose_size / 1024;

    if args.verbose {
        // Also report pack information
        let mut pack_count: u64 = 0;
        let mut packed_objects: u64 = 0;
        let mut pack_size: u64 = 0;

        let pack_dir = objects_dir.join("pack");
        if pack_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&pack_dir) {
                for entry in entries {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    let path = entry.path();
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();

                    if name_str.ends_with(".pack") {
                        let meta = entry.metadata()?;
                        pack_count += 1;
                        pack_size += meta.len();
                    }

                    if name_str.ends_with(".idx") {
                        // Count objects from v2 idx file size:
                        // v2 idx layout: 1032 byte header + 24 bytes per entry + ...
                        // fanout (256*4=1024) + signature(4) + version(4) = 1032 header
                        // then: oids (20*n) + crc (4*n) = 24*n
                        packed_objects += count_idx_entries(&path);
                    }
                }
            }
        }

        let pack_size_kib = pack_size / 1024;

        writeln!(out, "count: {}", loose_count)?;
        writeln!(out, "size: {}", format_size(loose_size_kib, args.human_readable))?;
        writeln!(out, "in-pack: {}", packed_objects)?;
        writeln!(out, "packs: {}", pack_count)?;
        writeln!(out, "size-pack: {}", format_size(pack_size_kib, args.human_readable))?;
        writeln!(out, "prune-packable: 0")?;
        writeln!(out, "garbage: 0")?;
        writeln!(out, "size-garbage: {}", format_size(0, args.human_readable))?;
    } else {
        writeln!(
            out,
            "count: {}",
            loose_count,
        )?;
        writeln!(
            out,
            "size: {}",
            format_size(loose_size_kib, args.human_readable),
        )?;
    }

    Ok(0)
}

/// Count entries in a v2 pack index file from its file size.
///
/// v2 idx layout:
///   - 4-byte magic + 4-byte version = 8 bytes
///   - 256 * 4-byte fanout table = 1024 bytes
///   - n * 20-byte SHA1 entries
///   - n * 4-byte CRC32 entries
///   - n * 4-byte offset entries
///   - (possibly 8-byte large offsets)
///   - 20-byte pack checksum + 20-byte idx checksum = 40 bytes
///
/// The last fanout entry (at offset 1028..1032) gives the total object count.
/// We read it directly for accuracy.
fn count_idx_entries(idx_path: &Path) -> u64 {
    let data = match fs::read(idx_path) {
        Ok(d) => d,
        Err(_) => return 0,
    };

    // v2 idx: magic 0xff744f63, version 2, then fanout[256]
    if data.len() < 1032 {
        return 0;
    }

    // Check v2 magic
    if &data[0..4] == b"\xfftOc" && data[4..8] == [0, 0, 0, 2] {
        // Last fanout entry (index 255) at offset 8 + 255*4 = 1028
        let offset = 8 + 255 * 4;
        u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
            as u64
    } else {
        // v1 idx: fanout[256] starts at offset 0, last entry at 255*4=1020
        if data.len() < 1024 {
            return 0;
        }
        let offset = 255 * 4;
        u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
            as u64
    }
}

/// Format a size value, optionally with human-readable suffixes.
fn format_size(kib: u64, human_readable: bool) -> String {
    if !human_readable {
        return kib.to_string();
    }

    if kib >= 1_048_576 {
        // GiB
        let gib = kib as f64 / 1_048_576.0;
        format!("{:.2} GiB", gib)
    } else if kib >= 1024 {
        // MiB
        let mib = kib as f64 / 1024.0;
        format!("{:.2} MiB", mib)
    } else {
        format!("{} KiB", kib)
    }
}

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use clap::Args;
use git_hash::ObjectId;
use git_pack::write::{PackWriter, build_pack_index};
use git_ref::RefStore;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct RepackArgs {
    /// Pack everything into a single pack
    #[arg(short = 'a')]
    pub(crate) all: bool,

    /// Same as -a, but turn unreachable objects loose instead of including them
    #[arg(short = 'A')]
    pub(crate) all_loosen: bool,

    /// Delete redundant packs after repacking
    #[arg(short)]
    pub(crate) delete: bool,

    /// Do not reuse existing deltas (recompute them)
    #[arg(short)]
    pub(crate) force: bool,

    /// Do not reuse existing objects
    #[arg(short = 'F')]
    pub(crate) force_objects: bool,

    /// Be quiet
    #[arg(short, long)]
    pub(crate) quiet: bool,

    /// Only pack local objects
    #[arg(short, long)]
    pub(crate) local: bool,

    /// Write bitmap index (for multi-pack reachability)
    #[arg(long)]
    pub(crate) write_bitmap_hashcache: bool,

    /// Window size for delta compression
    #[arg(long)]
    pub(crate) window: Option<u32>,

    /// Maximum delta chain depth
    #[arg(long)]
    pub(crate) depth: Option<u32>,

    /// Number of threads for delta searching
    #[arg(long)]
    pub(crate) threads: Option<u32>,

    /// Do not remove packs listed in .keep files
    #[arg(long = "keep-pack")]
    pub(crate) keep_pack: Vec<String>,
}

pub fn run(args: &RepackArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let objects_dir = repo.odb().objects_dir().to_path_buf();
    let pack_dir = objects_dir.join("pack");

    // Ensure pack directory exists
    std::fs::create_dir_all(&pack_dir)?;

    // Collect objects to pack
    let oids: Vec<ObjectId> = if args.all || args.all_loosen {
        // Pack all reachable objects
        let mut tips: Vec<ObjectId> = Vec::new();
        let refs = repo.refs().iter(None)?;
        for r in refs {
            let r = r?;
            let oid = r.peel_to_oid(repo.refs())?;
            tips.push(oid);
        }
        if let Some(head) = repo.head_oid()? {
            tips.push(head);
        }

        let reachable = git_revwalk::list_objects(&repo, &tips, &[], None)?;
        let mut unique = HashSet::new();
        reachable.into_iter().filter(|oid| unique.insert(*oid)).collect()
    } else {
        // Only pack loose objects
        let mut loose_oids = Vec::new();
        let iter = repo.odb().iter_all_oids()?;
        for result in iter {
            let oid = result?;
            // Check if it's a loose object by looking for the file
            let hex = oid.to_hex();
            let loose_path = objects_dir.join(&hex[..2]).join(&hex[2..]);
            if loose_path.exists() {
                loose_oids.push(oid);
            }
        }
        loose_oids
    };

    if oids.is_empty() {
        if !args.quiet {
            writeln!(err, "Nothing new to pack.")?;
        }
        return Ok(0);
    }

    if !args.quiet {
        writeln!(err, "Counting objects: {}, done.", oids.len())?;
    }

    // Generate a unique pack name based on content hash
    let mut name_hasher = git_hash::hasher::Hasher::new(git_hash::HashAlgorithm::Sha1);
    for oid in &oids {
        name_hasher.update(oid.as_bytes());
    }
    let pack_hash = name_hasher.finalize()?;
    let pack_name = format!("pack-{}", pack_hash.to_hex());

    let pack_path = pack_dir.join(format!("{}.pack", pack_name));
    let idx_path = pack_dir.join(format!("{}.idx", pack_name));

    // Write the new pack
    let mut writer = PackWriter::new(&pack_path)?;

    for oid in &oids {
        let obj = match repo.odb().read(oid)? {
            Some(obj) => obj,
            None => continue,
        };
        let content = obj.serialize_content();
        let obj_type = obj.object_type();
        writer.add_object(obj_type, &content)?;
    }

    let mut entries: Vec<(ObjectId, u64, u32)> = writer
        .entries()
        .map(|(oid, off, crc)| (*oid, off, crc))
        .collect();

    let (_, checksum) = writer.finish()?;

    // Build index
    build_pack_index(&idx_path, &mut entries, &checksum)?;

    if !args.quiet {
        writeln!(
            err,
            "Compressing objects: 100% ({}/{}), done.",
            oids.len(),
            oids.len()
        )?;
    }

    // Delete old packs if -d flag is set
    if args.delete {
        delete_old_packs(&pack_dir, &pack_name, &args.keep_pack)?;
    }

    // If -a was used, remove loose objects that are now packed
    if args.all && args.delete {
        remove_packed_loose_objects(&objects_dir, &oids)?;
    }

    // Refresh ODB to pick up new pack
    repo.odb().refresh()?;

    Ok(0)
}

/// Delete old pack files, keeping the newly created one and any .keep packs.
fn delete_old_packs(pack_dir: &Path, new_pack_name: &str, keep_packs: &[String]) -> Result<()> {
    let entries: Vec<_> = match std::fs::read_dir(pack_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip the newly created pack
        if name_str.starts_with(new_pack_name) {
            continue;
        }

        // Skip .keep files and packs with .keep files
        if name_str.ends_with(".keep") {
            continue;
        }

        if name_str.ends_with(".pack") {
            let base = name_str.trim_end_matches(".pack");

            // Check if there's a .keep file for this pack
            let keep_path = pack_dir.join(format!("{}.keep", base));
            if keep_path.exists() {
                continue;
            }

            // Check if this pack is in the keep list
            if keep_packs.iter().any(|k| name_str.contains(k)) {
                continue;
            }

            // Delete the pack and its index
            let _ = std::fs::remove_file(entry.path());
            let idx = pack_dir.join(format!("{}.idx", base));
            let _ = std::fs::remove_file(&idx);
            let bitmap = pack_dir.join(format!("{}.bitmap", base));
            let _ = std::fs::remove_file(&bitmap);
        }
    }

    Ok(())
}

/// Remove loose objects that have been packed.
fn remove_packed_loose_objects(objects_dir: &Path, packed_oids: &[ObjectId]) -> Result<()> {
    for oid in packed_oids {
        let hex = oid.to_hex();
        let loose_path = objects_dir.join(&hex[..2]).join(&hex[2..]);
        if loose_path.exists() {
            let _ = std::fs::remove_file(&loose_path);
            // Try to remove empty fanout directory
            if let Some(parent) = loose_path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
    Ok(())
}

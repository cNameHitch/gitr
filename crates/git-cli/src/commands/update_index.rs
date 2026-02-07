use std::io::{self, BufRead};

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_index::{IndexEntry, Stage, StatData, EntryFlags};
use git_object::FileMode;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct UpdateIndexArgs {
    /// Add files to the index
    #[arg(long)]
    add: bool,

    /// Remove files from the index
    #[arg(long)]
    remove: bool,

    /// Force removal even if the file has local modifications
    #[arg(long)]
    force_remove: bool,

    /// Directly insert the specified info into the index
    #[arg(long, value_name = "mode,oid,path", num_args = 1)]
    cacheinfo: Option<String>,

    /// Read list of paths from stdin
    #[arg(long)]
    stdin: bool,

    /// Replace existing entries with the same name
    #[arg(long)]
    replace: bool,

    /// Refresh stat info for existing entries
    #[arg(long)]
    refresh: bool,

    /// Files to add/update
    #[arg(value_name = "file")]
    files: Vec<String>,
}

pub fn run(args: &UpdateIndexArgs, cli: &Cli) -> Result<i32> {
    let mut repo = open_repo(cli)?;
    let _ = repo.index_mut()?;

    if let Some(ref cacheinfo) = args.cacheinfo {
        let parts: Vec<&str> = cacheinfo.splitn(3, ',').collect();
        if parts.len() != 3 {
            bail!("--cacheinfo requires mode,oid,path");
        }
        let mode_raw = u32::from_str_radix(parts[0], 8)?;
        let mode = FileMode::from_raw(mode_raw);
        let oid = ObjectId::from_hex(parts[1])?;
        let path = BString::from(parts[2]);

        let entry = IndexEntry {
            path,
            oid,
            mode,
            stage: Stage::Normal,
            stat: StatData::default(),
            flags: EntryFlags::default(),
        };

        let index = repo.index_mut()?;
        index.add(entry);
        repo.write_index()?;
        return Ok(0);
    }

    // Collect file paths from args and/or stdin
    let mut paths: Vec<String> = args.files.clone();

    if args.stdin {
        let stdin_handle = io::stdin();
        for line in stdin_handle.lock().lines() {
            let line = line?;
            let line = line.trim().to_string();
            if !line.is_empty() {
                paths.push(line);
            }
        }
    }

    let work_tree = repo
        .work_tree()
        .ok_or_else(|| anyhow::anyhow!("not a working tree"))?
        .to_path_buf();

    for path_str in &paths {
        let file_path = work_tree.join(path_str);

        if args.remove || args.force_remove {
            let bpath = bstr::BStr::new(path_str.as_bytes());
            let index = repo.index_mut()?;
            index.remove(bpath, Stage::Normal);
            continue;
        }

        if args.add || args.refresh {
            if !file_path.exists() {
                if args.add {
                    bail!("error: {}: does not exist and --add not specified", path_str);
                }
                continue;
            }

            let data = std::fs::read(&file_path)?;
            let oid = repo.odb().write_raw(git_object::ObjectType::Blob, &data)?;
            let meta = std::fs::metadata(&file_path)?;

            let mode = if is_executable(&meta) {
                FileMode::Executable
            } else {
                FileMode::Regular
            };

            let entry = IndexEntry {
                path: BString::from(path_str.as_str()),
                oid,
                mode,
                stage: Stage::Normal,
                stat: StatData::from_metadata(&meta),
                flags: EntryFlags::default(),
            };

            let index = repo.index_mut()?;
            index.add(entry);
        }
    }

    repo.write_index()?;

    Ok(0)
}

#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    false
}

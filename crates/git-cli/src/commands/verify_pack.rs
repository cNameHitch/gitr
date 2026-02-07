use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_pack::pack::PackFile;

use crate::Cli;

#[derive(Args)]
pub struct VerifyPackArgs {
    /// Be verbose (show all objects)
    #[arg(short = 'v')]
    verbose: bool,

    /// Show statistics
    #[arg(short = 's')]
    stat_only: bool,

    /// Pack index file(s) to verify
    #[arg(required = true)]
    pack_idx: Vec<String>,
}

pub fn run(args: &VerifyPackArgs, _cli: &Cli) -> Result<i32> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for idx_path in &args.pack_idx {
        // Convert .idx path to .pack path
        let pack_path = if idx_path.ends_with(".idx") {
            idx_path.replace(".idx", ".pack")
        } else if idx_path.ends_with(".pack") {
            idx_path.to_string()
        } else {
            anyhow::bail!("expected .idx or .pack file: {}", idx_path);
        };

        let pack = PackFile::open(&pack_path)?;

        // Verify checksum
        pack.verify_checksum()?;

        if args.verbose {
            // Iterate and display all objects
            let mut count: u32 = 0;
            for result in pack.iter() {
                let (oid, obj) = result?;
                writeln!(
                    out,
                    "{} {} {}",
                    oid.to_hex(),
                    obj.obj_type,
                    obj.data.len(),
                )?;
                count += 1;
            }

            if args.stat_only {
                writeln!(
                    out,
                    "pack {}: {} objects, verified",
                    pack_path, count,
                )?;
            }
        } else if args.stat_only {
            writeln!(
                out,
                "pack {}: {} objects, verified",
                pack_path,
                pack.num_objects(),
            )?;
        }
    }

    Ok(0)
}

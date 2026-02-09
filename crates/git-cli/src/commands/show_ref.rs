use std::io::{self, Write};

use anyhow::Result;
use clap::Args;
use git_ref::{RefStore, Reference};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct ShowRefArgs {
    /// Show HEAD reference as well
    #[arg(long)]
    head: bool,

    /// Only show heads (refs/heads/)
    #[arg(long)]
    heads: bool,

    /// Only show tags (refs/tags/)
    #[arg(long)]
    tags: bool,

    /// Verify that the given refs exist
    #[arg(long)]
    verify: bool,

    /// Be quiet (for --verify)
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Dereference tags to their target object
    #[arg(short = 'd')]
    dereference: bool,

    /// Patterns or refs to verify
    #[arg(value_name = "pattern")]
    patterns: Vec<String>,
}

pub fn run(args: &ShowRefArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let refs = repo.refs();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.verify {
        return verify_refs(args, refs, &mut out);
    }

    let mut found_any = false;

    // Show HEAD if requested
    if args.head {
        if let Some(oid) = refs.resolve_to_oid(&git_ref::RefName::new(bstr::BString::from("HEAD"))?)? {
            writeln!(out, "{} HEAD", oid.to_hex())?;
            found_any = true;
        }
    }

    // Determine prefix filter
    let prefix = if args.heads {
        Some("refs/heads/")
    } else if args.tags {
        Some("refs/tags/")
    } else {
        Some("refs/")
    };

    let iter = refs.iter(prefix)?;
    for ref_result in iter {
        let reference = ref_result?;
        let oid = match &reference {
            Reference::Direct { target, .. } => *target,
            Reference::Symbolic { .. } => match reference.peel_to_oid(refs) {
                Ok(oid) => oid,
                Err(_) => continue,
            },
        };

        let refname = reference.name().as_str();

        // Filter by patterns if given
        if !args.patterns.is_empty() {
            let matches = args.patterns.iter().any(|p| refname.starts_with(p.as_str()) || refname == p);
            if !matches {
                continue;
            }
        }

        writeln!(out, "{} {}", oid.to_hex(), refname)?;
        found_any = true;

        // Dereference tags
        if args.dereference {
            if let Ok(Some(git_object::Object::Tag(tag))) = repo.odb().read(&oid) {
                writeln!(out, "{} {}^{{}}", tag.target.to_hex(), refname)?;
            }
        }
    }

    // show-ref returns 1 if no refs were found
    if found_any {
        Ok(0)
    } else {
        Ok(1)
    }
}

fn verify_refs(
    args: &ShowRefArgs,
    refs: &git_ref::FilesRefStore,
    out: &mut impl Write,
) -> Result<i32> {
    let mut all_ok = true;

    for pattern in &args.patterns {
        let refname = match git_ref::RefName::new(bstr::BString::from(pattern.as_str())) {
            Ok(r) => r,
            Err(_) => {
                if !args.quiet {
                    eprintln!("fatal: '{}' - not a valid ref", pattern);
                }
                all_ok = false;
                continue;
            }
        };
        match refs.resolve_to_oid(&refname)? {
            Some(oid) => {
                if !args.quiet {
                    writeln!(out, "{} {}", oid.to_hex(), pattern)?;
                }
            }
            None => {
                if !args.quiet {
                    eprintln!("fatal: '{}' - not a valid ref", pattern);
                }
                all_ok = false;
            }
        }
    }

    if all_ok { Ok(0) } else { Ok(128) }
}

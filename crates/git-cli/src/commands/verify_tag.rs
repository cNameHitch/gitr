use std::io::{self, Write};
use std::process::Command;

use anyhow::{bail, Result};
use clap::Args;
use git_object::Object;
use git_ref::{RefName, RefStore};
use bstr::BString;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct VerifyTagArgs {
    /// Print raw gpg status output
    #[arg(long)]
    raw: bool,

    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Output format
    #[arg(long)]
    format: Option<String>,

    /// Tags to verify
    tags: Vec<String>,
}

pub fn run(args: &VerifyTagArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let mut all_valid = true;

    for tag_name in &args.tags {
        // Resolve tag to OID
        let refname = RefName::new(BString::from(format!("refs/tags/{}", tag_name)))?;
        let tag_oid = repo
            .refs()
            .resolve_to_oid(&refname)?
            .ok_or_else(|| anyhow::anyhow!("tag '{}' not found", tag_name))?;

        let obj = repo
            .odb()
            .read(&tag_oid)?
            .ok_or_else(|| anyhow::anyhow!("object {} not found", tag_oid.to_hex()))?;

        match obj {
            Object::Tag(tag) => {
                if let Some(ref sig) = tag.gpgsig {
                    // Build the signed content
                    let signed_content = build_signed_tag_content(&tag);

                    match verify_gpg_signature(&signed_content, sig.as_ref()) {
                        Ok(output) => {
                            if args.verbose {
                                writeln!(err, "{}", output.summary)?;
                            }
                            if !output.valid {
                                all_valid = false;
                                writeln!(err, "error: tag '{}' has a bad signature", tag_name)?;
                            }
                        }
                        Err(e) => {
                            all_valid = false;
                            writeln!(err, "error: could not verify tag '{}': {}", tag_name, e)?;
                        }
                    }
                } else {
                    all_valid = false;
                    writeln!(err, "error: no signature found in tag '{}'", tag_name)?;
                }
            }
            _ => {
                // Lightweight tag — no signature possible
                all_valid = false;
                writeln!(err, "error: '{}' is not an annotated tag", tag_name)?;
            }
        }
    }

    if all_valid {
        Ok(0)
    } else {
        Ok(1)
    }
}

struct GpgOutput {
    valid: bool,
    summary: String,
}

fn verify_gpg_signature(signed_content: &[u8], signature: &[u8]) -> Result<GpgOutput> {
    let tmp_dir = tempfile::tempdir()?;
    let sig_path = tmp_dir.path().join("signature.sig");
    let content_path = tmp_dir.path().join("content");

    std::fs::write(&sig_path, signature)?;
    std::fs::write(&content_path, signed_content)?;

    let output = Command::new("gpg")
        .args(["--status-fd=1", "--verify"])
        .arg(&sig_path)
        .arg(&content_path)
        .output();

    match output {
        Ok(output) => {
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            let valid = output.status.success();

            Ok(GpgOutput {
                valid,
                summary: stderr_str,
            })
        }
        Err(e) => {
            bail!("failed to run gpg: {}", e);
        }
    }
}

fn build_signed_tag_content(tag: &git_object::Tag) -> Vec<u8> {
    let mut content = Vec::new();
    content.extend_from_slice(b"object ");
    content.extend_from_slice(tag.target.to_hex().as_bytes());
    content.push(b'\n');

    content.extend_from_slice(b"type ");
    content.extend_from_slice(tag.target_type.as_bytes());
    content.push(b'\n');

    content.extend_from_slice(b"tag ");
    content.extend_from_slice(&tag.tag_name);
    content.push(b'\n');

    if let Some(ref tagger) = tag.tagger {
        content.extend_from_slice(b"tagger ");
        content.extend_from_slice(&tagger.to_bytes());
        content.push(b'\n');
    }

    content.push(b'\n');
    content.extend_from_slice(&tag.message);

    content
}

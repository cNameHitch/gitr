use std::io::{self, Write};
use std::process::Command;

use anyhow::{bail, Result};
use clap::Args;
use git_object::Object;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct VerifyCommitArgs {
    /// Print raw gpg status output
    #[arg(long)]
    raw: bool,

    /// Be verbose
    #[arg(short, long)]
    verbose: bool,

    /// Commits to verify
    commits: Vec<String>,
}

pub fn run(args: &VerifyCommitArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();

    let mut all_valid = true;

    for spec in &args.commits {
        let oid = git_revwalk::resolve_revision(&repo, spec)?;
        let obj = repo
            .odb()
            .read(&oid)?
            .ok_or_else(|| anyhow::anyhow!("object {} not found", oid.to_hex()))?;

        let commit = match obj {
            Object::Commit(c) => c,
            _ => bail!("{} is not a commit", oid.to_hex()),
        };

        if let Some(ref sig) = commit.gpgsig {
            // Extract the signature and signed payload
            let signed_content = build_signed_commit_content(&commit, &oid);

            match verify_gpg_signature(&signed_content, sig.as_ref()) {
                Ok(output) => {
                    if args.raw {
                        out.write_all(output.status.as_bytes())?;
                    }
                    if args.verbose {
                        writeln!(err, "{}", output.summary)?;
                    }
                    if !output.valid {
                        all_valid = false;
                        writeln!(err, "error: commit {} has a bad GPG signature", oid.to_hex())?;
                    }
                }
                Err(e) => {
                    all_valid = false;
                    writeln!(err, "error: could not verify commit {}: {}", oid.to_hex(), e)?;
                }
            }
        } else {
            all_valid = false;
            writeln!(err, "error: no signature found in commit {}", oid.to_hex())?;
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
    status: String,
}

/// Verify a GPG signature by calling the gpg binary.
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
            let status = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            let valid = output.status.success();

            Ok(GpgOutput {
                valid,
                summary: stderr_str,
                status,
            })
        }
        Err(e) => {
            bail!("failed to run gpg: {}", e);
        }
    }
}

/// Reconstruct the signed content from a commit (everything except the gpgsig header).
fn build_signed_commit_content(commit: &git_object::Commit, _oid: &git_hash::ObjectId) -> Vec<u8> {
    // Serialize the commit without the gpgsig field
    let mut content = Vec::new();
    content.extend_from_slice(b"tree ");
    content.extend_from_slice(commit.tree.to_hex().as_bytes());
    content.push(b'\n');

    for parent in &commit.parents {
        content.extend_from_slice(b"parent ");
        content.extend_from_slice(parent.to_hex().as_bytes());
        content.push(b'\n');
    }

    content.extend_from_slice(b"author ");
    content.extend_from_slice(&commit.author.to_bytes());
    content.push(b'\n');

    content.extend_from_slice(b"committer ");
    content.extend_from_slice(&commit.committer.to_bytes());
    content.push(b'\n');

    for (key, value) in &commit.extra_headers {
        if key.as_slice() != b"gpgsig" {
            content.extend_from_slice(key.as_slice());
            content.push(b' ');
            content.extend_from_slice(value.as_slice());
            content.push(b'\n');
        }
    }

    content.push(b'\n');
    content.extend_from_slice(&commit.message);

    content
}

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Args;
use git_hash::ObjectId;
use git_object::Object;

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct ArchiveArgs {
    /// Archive format (tar, tar.gz, tgz, zip)
    #[arg(long, default_value = "tar")]
    format: String,

    /// Prepend <prefix>/ to each filename in the archive
    #[arg(long)]
    prefix: Option<String>,

    /// Write the archive to <file> instead of stdout
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Compression level for zip format (0-9)
    #[arg(short = '0')]
    no_compression: bool,

    /// Remote repository
    #[arg(long)]
    remote: Option<String>,

    /// Tree-ish to produce an archive for
    tree_ish: Option<String>,

    /// Paths to include (if not specified, include all)
    #[arg(trailing_var_arg = true)]
    paths: Vec<String>,
}

pub fn run(args: &ArchiveArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;

    // Resolve tree-ish to a tree OID
    let commit_or_tree_oid = if let Some(ref spec) = args.tree_ish {
        git_revwalk::resolve_revision(&repo, spec)?
    } else {
        repo.head_oid()?
            .ok_or_else(|| anyhow::anyhow!("HEAD is not valid"))?
    };

    // Peel to tree
    let tree_oid = peel_to_tree(&repo, &commit_or_tree_oid)?;

    // Determine output
    let mut output: Box<dyn Write> = if let Some(ref path) = args.output {
        Box::new(std::fs::File::create(path)?)
    } else {
        Box::new(io::stdout().lock())
    };

    // Detect format from output filename if not explicitly set
    let format = if args.output.is_some() && args.format == "tar" {
        // Auto-detect from extension
        let ext = args
            .output
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("tar");
        match ext {
            "zip" => "zip",
            "gz" | "tgz" => "tar.gz",
            _ => "tar",
        }
    } else {
        &args.format
    };

    let prefix = args.prefix.as_deref().unwrap_or("");

    match format {
        "tar" => write_tar_archive(&repo, &tree_oid, prefix, &mut output)?,
        "tar.gz" | "tgz" => {
            let mut gz_writer = flate2::write::GzEncoder::new(&mut output, flate2::Compression::default());
            write_tar_archive(&repo, &tree_oid, prefix, &mut gz_writer)?;
            gz_writer.finish()?;
        }
        "zip" => write_zip_archive(&repo, &tree_oid, prefix, &mut output)?,
        other => bail!("unknown archive format: {}", other),
    }

    Ok(0)
}

/// Peel an OID to a tree, handling commits and tags.
fn peel_to_tree(
    repo: &git_repository::Repository,
    oid: &ObjectId,
) -> Result<ObjectId> {
    let obj = repo
        .odb()
        .read(oid)?
        .ok_or_else(|| anyhow::anyhow!("object {} not found", oid.to_hex()))?;

    match obj {
        Object::Tree(_) => Ok(*oid),
        Object::Commit(commit) => Ok(commit.tree),
        Object::Tag(tag) => peel_to_tree(repo, &tag.target),
        Object::Blob(_) => bail!("cannot create archive from a blob"),
    }
}

/// Write a tar archive of the tree.
fn write_tar_archive(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    prefix: &str,
    output: &mut dyn Write,
) -> Result<()> {
    // Collect all file entries from the tree
    let mut entries = Vec::new();
    collect_tree_entries(repo, tree_oid, prefix, &mut entries)?;

    // Write tar entries
    for entry in &entries {
        write_tar_entry(output, &entry.path, &entry.data, entry.mode)?;
    }

    // Write two 512-byte zero blocks as end-of-archive marker
    output.write_all(&[0u8; 512])?;
    output.write_all(&[0u8; 512])?;

    Ok(())
}

/// Write a zip archive of the tree.
fn write_zip_archive(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    prefix: &str,
    output: &mut dyn Write,
) -> Result<()> {
    // Collect all file entries
    let mut entries = Vec::new();
    collect_tree_entries(repo, tree_oid, prefix, &mut entries)?;

    // Simple ZIP format implementation
    let mut central_directory = Vec::new();
    let mut buf = Vec::new();

    for entry in &entries {
        let path_bytes = entry.path.as_bytes();
        let data = &entry.data;

        let crc = crc32fast::hash(data);

        // Local file header
        let local_header_start = buf.len() as u32;
        buf.extend_from_slice(b"PK\x03\x04"); // signature
        buf.extend_from_slice(&20u16.to_le_bytes()); // version needed
        buf.extend_from_slice(&0u16.to_le_bytes()); // flags
        buf.extend_from_slice(&0u16.to_le_bytes()); // compression: stored
        buf.extend_from_slice(&0u16.to_le_bytes()); // mod time
        buf.extend_from_slice(&0u16.to_le_bytes()); // mod date
        buf.extend_from_slice(&crc.to_le_bytes()); // crc32
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // compressed size
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncompressed size
        buf.extend_from_slice(&(path_bytes.len() as u16).to_le_bytes()); // filename length
        buf.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        buf.extend_from_slice(path_bytes);
        buf.extend_from_slice(data);

        // Central directory entry
        central_directory.extend_from_slice(b"PK\x01\x02"); // signature
        central_directory.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central_directory.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // flags
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // compression
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // mod date
        central_directory.extend_from_slice(&crc.to_le_bytes());
        central_directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central_directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central_directory.extend_from_slice(&(path_bytes.len() as u16).to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // extra field len
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // comment len
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // disk number
        central_directory.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        // External attrs: Unix permissions
        let ext_attrs = if entry.mode == 0o100755 {
            0o755u32 << 16
        } else {
            0o644u32 << 16
        };
        central_directory.extend_from_slice(&ext_attrs.to_le_bytes());
        central_directory.extend_from_slice(&local_header_start.to_le_bytes()); // local header offset
        central_directory.extend_from_slice(path_bytes);

    }

    let cd_offset = buf.len() as u32;
    buf.extend_from_slice(&central_directory);

    // End of central directory
    let cd_size = central_directory.len() as u32;
    buf.extend_from_slice(b"PK\x05\x06");
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk number
    buf.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // entries on this disk
    buf.extend_from_slice(&(entries.len() as u16).to_le_bytes()); // total entries
    buf.extend_from_slice(&cd_size.to_le_bytes()); // cd size
    buf.extend_from_slice(&cd_offset.to_le_bytes()); // cd offset
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment len

    output.write_all(&buf)?;

    Ok(())
}

struct ArchiveEntry {
    path: String,
    data: Vec<u8>,
    mode: u32,
}

/// Recursively collect all file entries from a tree.
fn collect_tree_entries(
    repo: &git_repository::Repository,
    tree_oid: &ObjectId,
    prefix: &str,
    entries: &mut Vec<ArchiveEntry>,
) -> Result<()> {
    let tree_obj = repo
        .odb()
        .read(tree_oid)?
        .ok_or_else(|| anyhow::anyhow!("tree {} not found", tree_oid.to_hex()))?;

    let tree = match tree_obj {
        Object::Tree(t) => t,
        _ => bail!("expected tree object"),
    };

    for entry in &tree.entries {
        let name = String::from_utf8_lossy(entry.name.as_ref()).to_string();
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}{}", prefix, name)
        };

        let mode = entry.mode.raw();
        if mode == 0o040000 {
            // Subtree
            let sub_prefix = format!("{}/", path);
            collect_tree_entries(repo, &entry.oid, &sub_prefix, entries)?;
        } else if mode == 0o160000 {
            // Submodule — skip in archive
            continue;
        } else {
            // Blob
            if let Some(Object::Blob(blob)) = repo.odb().read(&entry.oid)? {
                entries.push(ArchiveEntry {
                    path,
                    data: blob.data.clone(),
                    mode,
                });
            }
        }
    }

    Ok(())
}

/// Write a single tar entry.
fn write_tar_entry(output: &mut dyn Write, path: &str, data: &[u8], mode: u32) -> Result<()> {
    let mut header = [0u8; 512];

    // Name (100 bytes)
    let name_bytes = path.as_bytes();
    let name_len = name_bytes.len().min(100);
    header[..name_len].copy_from_slice(&name_bytes[..name_len]);

    // Mode (8 bytes, octal ASCII)
    let mode_str = format!("{:07o}\0", if mode == 0o100755 { 0o755 } else { 0o644 });
    header[100..108].copy_from_slice(mode_str.as_bytes());

    // UID (8 bytes)
    header[108..116].copy_from_slice(b"0000000\0");

    // GID (8 bytes)
    header[116..124].copy_from_slice(b"0000000\0");

    // Size (12 bytes, octal ASCII)
    let size_str = format!("{:011o}\0", data.len());
    header[124..136].copy_from_slice(size_str.as_bytes());

    // Mtime (12 bytes, octal ASCII)
    header[136..148].copy_from_slice(b"00000000000\0");

    // Typeflag
    header[156] = b'0'; // regular file

    // Magic
    header[257..263].copy_from_slice(b"ustar\0");

    // Version
    header[263..265].copy_from_slice(b"00");

    // Compute checksum
    // First, fill checksum field with spaces
    header[148..156].copy_from_slice(b"        ");
    let checksum: u32 = header.iter().map(|&b| b as u32).sum();
    let checksum_str = format!("{:06o}\0 ", checksum);
    header[148..156].copy_from_slice(checksum_str.as_bytes());

    output.write_all(&header)?;
    output.write_all(data)?;

    // Pad to 512-byte boundary
    let padding = (512 - (data.len() % 512)) % 512;
    if padding > 0 {
        output.write_all(&vec![0u8; padding])?;
    }

    Ok(())
}

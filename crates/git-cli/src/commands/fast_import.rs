use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::{bail, Result};
use bstr::BString;
use clap::Args;
use git_hash::ObjectId;
use git_object::{Commit, FileMode, Object, ObjectType, Tag, Tree, TreeEntry};
use git_ref::RefName;
use git_utils::date::{GitDate, Signature};

use crate::Cli;
use super::open_repo;

#[derive(Args)]
pub struct FastImportArgs {
    /// Be quiet
    #[arg(long)]
    quiet: bool,

    /// Show statistics after import
    #[arg(long)]
    stats: bool,

    /// Force import even if marks file exists
    #[arg(long)]
    force: bool,

    /// Import/export marks file
    #[arg(long = "import-marks")]
    import_marks: Option<PathBuf>,

    #[arg(long = "export-marks")]
    export_marks: Option<PathBuf>,

    /// Maximum pack size
    #[arg(long = "max-pack-size")]
    max_pack_size: Option<u64>,

    /// Date format
    #[arg(long = "date-format", default_value = "raw")]
    date_format: String,

    /// Terminate after done command
    #[arg(long)]
    done: bool,
}

pub fn run(args: &FastImportArgs, cli: &Cli) -> Result<i32> {
    let repo = open_repo(cli)?;
    let stderr = io::stderr();
    let mut err = stderr.lock();
    let stdin = io::stdin();

    let mut marks: HashMap<String, ObjectId> = HashMap::new();
    let mut blob_count = 0u64;
    let mut commit_count = 0u64;
    let mut tag_count = 0u64;

    // Import existing marks
    if let Some(ref marks_path) = args.import_marks {
        if marks_path.exists() {
            let content = std::fs::read_to_string(marks_path)?;
            for line in content.lines() {
                if let Some((mark, hex)) = line.split_once(' ') {
                    if let Ok(oid) = ObjectId::from_hex(hex) {
                        marks.insert(mark.to_string(), oid);
                    }
                }
            }
        }
    }

    // Process input stream
    let mut lines = stdin.lock().lines();
    let mut current_line: Option<String> = None;

    loop {
        let line = if let Some(cached) = current_line.take() {
            cached
        } else {
            match lines.next() {
                Some(Ok(l)) => l,
                Some(Err(e)) => bail!("read error: {}", e),
                None => break,
            }
        };

        let line = line.trim().to_string();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "done" {
            break;
        }

        if line.starts_with("blob") {
            // Read blob data
            let mark = read_optional_mark(&mut lines, &mut current_line)?;
            let data = read_data(&mut lines, &mut current_line)?;

            let oid = repo.odb().write_raw(ObjectType::Blob, &data)?;

            if let Some(m) = mark {
                marks.insert(m, oid);
            }

            blob_count += 1;
        } else if line.starts_with("commit ") {
            let ref_name = line.strip_prefix("commit ").unwrap().trim().to_string();

            let mut mark_id = None;
            let mut author: Option<Signature> = None;
            let mut committer: Option<Signature> = None;
            let mut message = Vec::new();
            let mut from_oid: Option<ObjectId> = None;
            let mut merge_oids: Vec<ObjectId> = Vec::new();
            let mut tree_entries: Vec<TreeEntry> = Vec::new();

            // Parse commit properties
            while let Some(Ok(l)) = lines.next() {
                let sub_line = l.trim().to_string();

                if sub_line.is_empty() {
                    break;
                }

                if sub_line.starts_with("mark ") {
                    mark_id = Some(sub_line.strip_prefix("mark ").unwrap().to_string());
                } else if sub_line.starts_with("author ") {
                    author = Some(parse_ident(sub_line.strip_prefix("author ").unwrap())?);
                } else if sub_line.starts_with("committer ") {
                    committer = Some(parse_ident(sub_line.strip_prefix("committer ").unwrap())?);
                } else if sub_line.starts_with("data ") {
                    let size: usize = sub_line
                        .strip_prefix("data ")
                        .unwrap()
                        .trim()
                        .parse()?;
                    let mut buf = vec![0u8; size];
                    let mut total_read = 0;
                    while total_read < size {
                        match lines.next() {
                            Some(Ok(l)) => {
                                let bytes = l.as_bytes();
                                let copy_len = (size - total_read).min(bytes.len());
                                buf[total_read..total_read + copy_len]
                                    .copy_from_slice(&bytes[..copy_len]);
                                total_read += copy_len + 1; // +1 for newline
                            }
                            _ => break,
                        }
                    }
                    message = buf[..size.min(buf.len())].to_vec();
                } else if sub_line.starts_with("from ") {
                    let spec = sub_line.strip_prefix("from ").unwrap().trim();
                    from_oid = resolve_mark_or_oid(spec, &marks)?;
                } else if sub_line.starts_with("merge ") {
                    let spec = sub_line.strip_prefix("merge ").unwrap().trim();
                    if let Some(oid) = resolve_mark_or_oid(spec, &marks)? {
                        merge_oids.push(oid);
                    }
                } else if sub_line.starts_with("M ") {
                    // File modify: M <mode> <dataref> <path>
                    let parts: Vec<&str> = sub_line.splitn(4, ' ').collect();
                    if parts.len() >= 4 {
                        let mode = parse_mode(parts[1])?;
                        let data_oid = resolve_mark_or_oid(parts[2], &marks)?
                            .ok_or_else(|| anyhow::anyhow!("unresolved reference: {}", parts[2]))?;
                        let path = parts[3].to_string();
                        tree_entries.push(TreeEntry {
                            mode,
                            name: BString::from(path),
                            oid: data_oid,
                        });
                    }
                } else if sub_line.starts_with("D ") {
                    // File delete: D <path> — handled by not including the entry
                } else {
                    // Unknown command in commit, push back
                    current_line = Some(sub_line);
                    break;
                }
            }

            // Build the tree from entries
            tree_entries.sort_by(|a, b| a.name.cmp(&b.name));
            let tree = Tree {
                entries: tree_entries,
            };
            let tree_oid = repo.odb().write(&Object::Tree(tree))?;

            // Build parents
            let mut parents = Vec::new();
            if let Some(oid) = from_oid {
                parents.push(oid);
            }
            parents.extend(merge_oids);

            let committer_sig = committer.unwrap_or_else(|| Signature {
                name: BString::from("Import"),
                email: BString::from("import@git"),
                date: GitDate::now(),
            });
            let author_sig = author.unwrap_or_else(|| committer_sig.clone());

            let commit = Commit {
                tree: tree_oid,
                parents,
                author: author_sig,
                committer: committer_sig,
                encoding: None,
                gpgsig: None,
                extra_headers: Vec::new(),
                message: BString::from(message),
            };

            let commit_oid = repo.odb().write(&Object::Commit(commit))?;

            // Update ref
            let refname = RefName::new(BString::from(ref_name))?;
            repo.refs().write_ref(&refname, &commit_oid)?;

            if let Some(m) = mark_id {
                marks.insert(m, commit_oid);
            }

            commit_count += 1;
        } else if line.starts_with("tag ") {
            let tag_name = line.strip_prefix("tag ").unwrap().trim().to_string();

            let mut from_oid = None;
            let mut tagger: Option<Signature> = None;
            let mut message = Vec::new();

            while let Some(Ok(l)) = lines.next() {
                let sub_line = l.trim().to_string();

                if sub_line.is_empty() {
                    break;
                }

                if sub_line.starts_with("from ") {
                    let spec = sub_line.strip_prefix("from ").unwrap().trim();
                    from_oid = resolve_mark_or_oid(spec, &marks)?;
                } else if sub_line.starts_with("tagger ") {
                    tagger = Some(parse_ident(sub_line.strip_prefix("tagger ").unwrap())?);
                } else if sub_line.starts_with("data ") {
                    let size: usize = sub_line.strip_prefix("data ").unwrap().trim().parse()?;
                    let mut buf = vec![0u8; size];
                    let mut total_read = 0;
                    while total_read < size {
                        match lines.next() {
                            Some(Ok(l)) => {
                                let bytes = l.as_bytes();
                                let copy_len = (size - total_read).min(bytes.len());
                                buf[total_read..total_read + copy_len]
                                    .copy_from_slice(&bytes[..copy_len]);
                                total_read += copy_len + 1;
                            }
                            _ => break,
                        }
                    }
                    message = buf[..size.min(buf.len())].to_vec();
                } else {
                    current_line = Some(sub_line);
                    break;
                }
            }

            if let Some(target) = from_oid {
                let target_obj = repo.odb().read(&target)?
                    .ok_or_else(|| anyhow::anyhow!("target not found"))?;

                let tag = Tag {
                    target,
                    target_type: target_obj.object_type(),
                    tag_name: BString::from(tag_name.as_str()),
                    tagger,
                    message: BString::from(message),
                    gpgsig: None,
                };

                let tag_oid = repo.odb().write(&Object::Tag(tag))?;
                let refname = RefName::new(BString::from(format!("refs/tags/{}", tag_name)))?;
                repo.refs().write_ref(&refname, &tag_oid)?;

                tag_count += 1;
            }
        } else if line.starts_with("reset ") {
            let ref_name = line.strip_prefix("reset ").unwrap().trim().to_string();
            // Read optional from
            if let Some(Ok(next)) = lines.next() {
                let next = next.trim().to_string();
                if next.starts_with("from ") {
                    let spec = next.strip_prefix("from ").unwrap().trim();
                    if let Some(oid) = resolve_mark_or_oid(spec, &marks)? {
                        let refname = RefName::new(BString::from(ref_name))?;
                        repo.refs().write_ref(&refname, &oid)?;
                    }
                } else {
                    current_line = Some(next);
                }
            }
        } else if line.starts_with("progress ") {
            let msg = line.strip_prefix("progress ").unwrap();
            if !args.quiet {
                writeln!(err, "{}", msg)?;
            }
        } else if line.starts_with("checkpoint") {
            // checkpoint: flush data, no-op for us
        }
    }

    // Export marks
    if let Some(ref marks_path) = args.export_marks {
        let mut content = String::new();
        for (mark, oid) in &marks {
            content.push_str(&format!("{} {}\n", mark, oid.to_hex()));
        }
        std::fs::write(marks_path, &content)?;
    }

    if !args.quiet || args.stats {
        writeln!(err)?;
        writeln!(err, "-----")?;
        writeln!(err, "Blobs:   {}", blob_count)?;
        writeln!(err, "Commits: {}", commit_count)?;
        writeln!(err, "Tags:    {}", tag_count)?;
        writeln!(err, "Marks:   {}", marks.len())?;
        writeln!(err, "-----")?;
    }

    Ok(0)
}

fn resolve_mark_or_oid(
    spec: &str,
    marks: &HashMap<String, ObjectId>,
) -> Result<Option<ObjectId>> {
    if spec.starts_with(':') {
        Ok(marks.get(spec).copied())
    } else {
        Ok(Some(ObjectId::from_hex(spec)?))
    }
}

fn read_optional_mark(
    lines: &mut io::Lines<io::StdinLock<'_>>,
    current_line: &mut Option<String>,
) -> Result<Option<String>> {
    match lines.next() {
        Some(Ok(line)) => {
            let line = line.trim().to_string();
            if line.starts_with("mark ") {
                Ok(Some(line.strip_prefix("mark ").unwrap().to_string()))
            } else {
                *current_line = Some(line);
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn read_data(
    lines: &mut io::Lines<io::StdinLock<'_>>,
    current_line: &mut Option<String>,
) -> Result<Vec<u8>> {
    let line = if let Some(cached) = current_line.take() {
        cached
    } else {
        match lines.next() {
            Some(Ok(l)) => l.trim().to_string(),
            _ => bail!("expected data command"),
        }
    };

    if let Some(size_str) = line.strip_prefix("data ") {
        let size: usize = size_str.trim().parse()?;
        let mut data = Vec::with_capacity(size);
        let mut remaining = size;
        while remaining > 0 {
            match lines.next() {
                Some(Ok(l)) => {
                    let bytes = l.as_bytes();
                    if data.len() + bytes.len() < size {
                        data.extend_from_slice(bytes);
                        data.push(b'\n');
                        remaining = size - data.len();
                    } else {
                        let take = remaining.min(bytes.len());
                        data.extend_from_slice(&bytes[..take]);
                        remaining = 0;
                    }
                }
                _ => break,
            }
        }
        data.truncate(size);
        Ok(data)
    } else {
        bail!("expected 'data <size>', got: {}", line);
    }
}

fn parse_ident(s: &str) -> Result<Signature> {
    // Format: "Name <email> timestamp tz"
    let lt = s
        .find('<')
        .ok_or_else(|| anyhow::anyhow!("invalid identity: {}", s))?;
    let gt = s
        .find('>')
        .ok_or_else(|| anyhow::anyhow!("invalid identity: {}", s))?;

    let name = s[..lt].trim();
    let email = &s[lt + 1..gt];
    let rest = s[gt + 1..].trim();

    let parts: Vec<&str> = rest.split_whitespace().collect();
    let timestamp: i64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let tz_str = parts.get(1).unwrap_or(&"+0000");
    let tz_offset: i32 = parse_tz_offset(tz_str);

    Ok(Signature {
        name: BString::from(name),
        email: BString::from(email),
        date: GitDate::new(timestamp, tz_offset),
    })
}

fn parse_tz_offset(s: &str) -> i32 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let sign = if s.starts_with('-') { -1 } else { 1 };
    let digits = s.trim_start_matches(['+', '-']);
    if digits.len() >= 4 {
        let hours: i32 = digits[..2].parse().unwrap_or(0);
        let minutes: i32 = digits[2..4].parse().unwrap_or(0);
        sign * (hours * 60 + minutes)
    } else {
        0
    }
}

fn parse_mode(s: &str) -> Result<FileMode> {
    let mode: u32 = u32::from_str_radix(s, 8)?;
    Ok(FileMode::from_raw(mode))
}

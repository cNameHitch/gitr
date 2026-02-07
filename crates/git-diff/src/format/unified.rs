//! Unified diff format output.
//!
//! Produces standard unified diff format with `---`/`+++` headers,
//! `@@ ... @@` hunk headers, and context/addition/deletion lines.

use bstr::ByteSlice;

use crate::{DiffLine, DiffOptions, DiffResult, FileDiff, FileStatus, Hunk};

/// Format a DiffResult as a unified diff.
pub fn format(result: &DiffResult, options: &DiffOptions) -> String {
    let mut out = String::new();

    for file in &result.files {
        format_file_diff(&mut out, file, options);
    }

    out
}

/// Format a single file's diff.
fn format_file_diff(out: &mut String, file: &FileDiff, _options: &DiffOptions) {
    let old_path = file
        .old_path
        .as_ref()
        .map(|p| p.to_str_lossy().into_owned())
        .unwrap_or_else(|| "/dev/null".to_string());
    let new_path = file
        .new_path
        .as_ref()
        .map(|p| p.to_str_lossy().into_owned())
        .unwrap_or_else(|| "/dev/null".to_string());

    // diff --git header
    let a_path = file.old_path.as_ref().unwrap_or_else(|| file.new_path.as_ref().unwrap());
    let b_path = file.new_path.as_ref().unwrap_or_else(|| file.old_path.as_ref().unwrap());
    out.push_str(&format!(
        "diff --git a/{} b/{}\n",
        a_path.to_str_lossy(),
        b_path.to_str_lossy()
    ));

    // Mode headers
    match file.status {
        FileStatus::Added => {
            if let Some(mode) = file.new_mode {
                out.push_str(&format!("new file mode {}\n", format_mode(mode)));
            }
        }
        FileStatus::Deleted => {
            if let Some(mode) = file.old_mode {
                out.push_str(&format!("deleted file mode {}\n", format_mode(mode)));
            }
        }
        _ => {
            if file.old_mode != file.new_mode {
                if let (Some(old_m), Some(new_m)) = (file.old_mode, file.new_mode) {
                    out.push_str(&format!(
                        "old mode {}\nnew mode {}\n",
                        format_mode(old_m),
                        format_mode(new_m)
                    ));
                }
            }
        }
    }

    // Similarity header for renames/copies
    if let Some(sim) = file.similarity {
        match file.status {
            FileStatus::Renamed => {
                out.push_str(&format!("similarity index {}%\n", sim));
                out.push_str(&format!(
                    "rename from {}\nrename to {}\n",
                    old_path, new_path
                ));
            }
            FileStatus::Copied => {
                out.push_str(&format!("similarity index {}%\n", sim));
                out.push_str(&format!(
                    "copy from {}\ncopy to {}\n",
                    old_path, new_path
                ));
            }
            _ => {}
        }
    }

    // Index line
    let old_hex_opt = file.old_oid.map(|o| { let h = o.to_hex(); h[..7.min(h.len())].to_string() });
    let new_hex_opt = file.new_oid.map(|o| { let h = o.to_hex(); h[..7.min(h.len())].to_string() });
    let old_hex = old_hex_opt.as_deref().unwrap_or("0000000");
    let new_hex = new_hex_opt.as_deref().unwrap_or("0000000");
    if file.status != FileStatus::Added && file.status != FileStatus::Deleted {
        if file.old_mode == file.new_mode {
            if let Some(mode) = file.old_mode {
                out.push_str(&format!("index {}..{} {}\n", old_hex, new_hex, format_mode(mode)));
            } else {
                out.push_str(&format!("index {}..{}\n", old_hex, new_hex));
            }
        } else {
            out.push_str(&format!("index {}..{}\n", old_hex, new_hex));
        }
    } else if file.status == FileStatus::Added {
        if let Some(new_oid) = file.new_oid {
            let h = new_oid.to_hex();
            let short = &h[..7.min(h.len())];
            out.push_str(&format!("index 0000000..{}\n", short));
        }
    } else if file.status == FileStatus::Deleted {
        if let Some(old_oid) = file.old_oid {
            let h = old_oid.to_hex();
            let short = &h[..7.min(h.len())];
            out.push_str(&format!("index {}..0000000\n", short));
        }
    }

    // Binary notice
    if file.is_binary {
        out.push_str(&format!(
            "Binary files a/{} and b/{} differ\n",
            old_path, new_path
        ));
        return;
    }

    // File content headers
    if !file.hunks.is_empty() {
        if file.status == FileStatus::Added {
            out.push_str("--- /dev/null\n");
        } else {
            out.push_str(&format!("--- a/{}\n", old_path));
        }

        if file.status == FileStatus::Deleted {
            out.push_str("+++ /dev/null\n");
        } else {
            out.push_str(&format!("+++ b/{}\n", new_path));
        }

        // Hunks
        for hunk in &file.hunks {
            format_hunk(out, hunk);
        }
    }
}

/// Format a hunk header and lines.
fn format_hunk(out: &mut String, hunk: &Hunk) {
    // @@ header — omit count when it equals 1 (git convention)
    let old_range = if hunk.old_count == 1 {
        format!("{}", hunk.old_start)
    } else {
        format!("{},{}", hunk.old_start, hunk.old_count)
    };
    let new_range = if hunk.new_count == 1 {
        format!("{}", hunk.new_start)
    } else {
        format!("{},{}", hunk.new_start, hunk.new_count)
    };
    out.push_str(&format!("@@ -{} +{} @@", old_range, new_range));
    if let Some(ref header) = hunk.header {
        out.push(' ');
        out.push_str(&header.to_str_lossy());
    }
    out.push('\n');

    // Lines
    for line in &hunk.lines {
        match line {
            DiffLine::Context(content) => {
                out.push(' ');
                out.push_str(&content.to_str_lossy());
                ensure_newline(out);
            }
            DiffLine::Addition(content) => {
                out.push('+');
                out.push_str(&content.to_str_lossy());
                ensure_newline(out);
            }
            DiffLine::Deletion(content) => {
                out.push('-');
                out.push_str(&content.to_str_lossy());
                ensure_newline(out);
            }
        }
    }
}

/// Ensure the output ends with a newline.
fn ensure_newline(out: &mut String) {
    if !out.ends_with('\n') {
        out.push_str("\n\\ No newline at end of file\n");
    }
}

/// Format a FileMode as an octal string.
fn format_mode(mode: git_object::FileMode) -> String {
    format!("{:06o}", mode.raw())
}

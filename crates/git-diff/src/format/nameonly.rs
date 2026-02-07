//! --name-only, --name-status, and --summary output formats.

use bstr::ByteSlice;

use crate::{DiffResult, FileStatus};

/// Format as --name-only (just file paths).
pub fn format_name_only(result: &DiffResult) -> String {
    let mut out = String::new();
    for file in &result.files {
        out.push_str(&file.path().to_str_lossy());
        out.push('\n');
    }
    out
}

/// Format as --name-status (status letter + file path).
pub fn format_name_status(result: &DiffResult) -> String {
    let mut out = String::new();
    for file in &result.files {
        match file.status {
            FileStatus::Renamed => {
                let old = file
                    .old_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                let new = file
                    .new_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                if let Some(sim) = file.similarity {
                    out.push_str(&format!("R{:03}\t{}\t{}\n", sim, old, new));
                } else {
                    out.push_str(&format!("R100\t{}\t{}\n", old, new));
                }
            }
            FileStatus::Copied => {
                let old = file
                    .old_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                let new = file
                    .new_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                if let Some(sim) = file.similarity {
                    out.push_str(&format!("C{:03}\t{}\t{}\n", sim, old, new));
                } else {
                    out.push_str(&format!("C100\t{}\t{}\n", old, new));
                }
            }
            _ => {
                out.push(file.status.as_char());
                out.push('\t');
                out.push_str(&file.path().to_str_lossy());
                out.push('\n');
            }
        }
    }
    out
}

/// Format as --summary.
pub fn format_summary(result: &DiffResult) -> String {
    let mut out = String::new();
    for file in &result.files {
        match file.status {
            FileStatus::Added => {
                let mode = file
                    .new_mode
                    .map(|m| format!("{:06o}", m.raw()))
                    .unwrap_or_else(|| "100644".to_string());
                out.push_str(&format!(
                    " create mode {} {}\n",
                    mode,
                    file.path().to_str_lossy()
                ));
            }
            FileStatus::Deleted => {
                let mode = file
                    .old_mode
                    .map(|m| format!("{:06o}", m.raw()))
                    .unwrap_or_else(|| "100644".to_string());
                out.push_str(&format!(
                    " delete mode {} {}\n",
                    mode,
                    file.path().to_str_lossy()
                ));
            }
            FileStatus::Renamed => {
                let old = file
                    .old_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                let new = file
                    .new_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                let sim = file.similarity.unwrap_or(100);
                out.push_str(&format!(
                    " rename {} => {} ({}%)\n",
                    old, new, sim
                ));
            }
            FileStatus::Copied => {
                let old = file
                    .old_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                let new = file
                    .new_path
                    .as_ref()
                    .map(|p| p.to_str_lossy().into_owned())
                    .unwrap_or_default();
                let sim = file.similarity.unwrap_or(100);
                out.push_str(&format!(
                    " copy {} => {} ({}%)\n",
                    old, new, sim
                ));
            }
            FileStatus::TypeChanged => {
                out.push_str(&format!(
                    " mode change {} => {} {}\n",
                    file.old_mode
                        .map(|m| format!("{:06o}", m.raw()))
                        .unwrap_or_default(),
                    file.new_mode
                        .map(|m| format!("{:06o}", m.raw()))
                        .unwrap_or_default(),
                    file.path().to_str_lossy()
                ));
            }
            _ => {}
        }
    }
    out
}

//! --raw output format.
//!
//! Produces the raw diff format with mode, OID, and status columns.

use bstr::ByteSlice;
use git_hash::ObjectId;

use crate::{DiffResult, FileDiff, FileStatus};

/// Format a DiffResult in raw format.
pub fn format(result: &DiffResult) -> String {
    let mut out = String::new();
    for file in &result.files {
        format_raw_entry(&mut out, file);
    }
    out
}

fn format_raw_entry(out: &mut String, file: &FileDiff) {
    let old_mode = file
        .old_mode
        .map(|m| format!("{:06o}", m.raw()))
        .unwrap_or_else(|| "000000".to_string());
    let new_mode = file
        .new_mode
        .map(|m| format!("{:06o}", m.raw()))
        .unwrap_or_else(|| "000000".to_string());

    let old_oid = file
        .old_oid
        .map(|o| abbreviate_oid(&o))
        .unwrap_or_else(|| "0000000".to_string());
    let new_oid = file
        .new_oid
        .map(|o| abbreviate_oid(&o))
        .unwrap_or_else(|| "0000000".to_string());

    let status = match file.status {
        FileStatus::Renamed => {
            if let Some(sim) = file.similarity {
                format!("R{:03}", sim)
            } else {
                "R100".to_string()
            }
        }
        FileStatus::Copied => {
            if let Some(sim) = file.similarity {
                format!("C{:03}", sim)
            } else {
                "C100".to_string()
            }
        }
        other => other.as_char().to_string(),
    };

    out.push_str(&format!(":{} {} {} {} {}\t", old_mode, new_mode, old_oid, new_oid, status));

    match file.status {
        FileStatus::Renamed | FileStatus::Copied => {
            let old_path = file
                .old_path
                .as_ref()
                .map(|p| p.to_str_lossy().into_owned())
                .unwrap_or_default();
            let new_path = file
                .new_path
                .as_ref()
                .map(|p| p.to_str_lossy().into_owned())
                .unwrap_or_default();
            out.push_str(&format!("{}\t{}\n", old_path, new_path));
        }
        _ => {
            let path = file.path().to_str_lossy();
            out.push_str(&format!("{}\n", path));
        }
    }
}

/// Abbreviate an OID to 7 hex characters.
fn abbreviate_oid(oid: &ObjectId) -> String {
    let hex = oid.to_hex();
    hex[..7.min(hex.len())].to_string()
}

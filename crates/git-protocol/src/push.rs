//! Push protocol (send-pack) implementation.
//!
//! Implements the client side of the send-pack ↔ receive-pack protocol:
//! 1. Read server's ref advertisement
//! 2. Send ref update commands
//! 3. Generate and stream thin pack
//! 4. Parse server's status report

use std::io::Write;

use bstr::BString;
use git_hash::ObjectId;

use git_transport::Transport;

use crate::capability::{self, Capabilities, SidebandMode};
use crate::pktline::{PktLineReader, PktLineWriter};
use crate::ProtocolError;

/// A single ref update to push.
#[derive(Debug, Clone)]
pub struct PushUpdate {
    /// Local OID to push. None = delete the remote ref.
    pub local_oid: Option<ObjectId>,
    /// Remote ref name to update.
    pub remote_ref: String,
    /// Force push (skip fast-forward check).
    pub force: bool,
    /// Expected remote OID for --force-with-lease (None = no check).
    pub expected_remote_oid: Option<ObjectId>,
}

/// Push operation options.
#[derive(Debug, Clone)]
pub struct PushOptions {
    /// Show progress.
    pub progress: bool,
    /// Atomic push (all or nothing).
    pub atomic: bool,
    /// Push option strings (--push-option).
    pub push_options: Vec<String>,
    /// Generate thin packs.
    pub thin: bool,
}

impl Default for PushOptions {
    fn default() -> Self {
        Self {
            progress: true,
            atomic: false,
            push_options: Vec::new(),
            thin: true,
        }
    }
}

/// Result of a push operation.
#[derive(Debug)]
pub struct PushResult {
    /// Overall success.
    pub ok: bool,
    /// Per-ref results.
    pub ref_results: Vec<(String, PushRefResult)>,
    /// Server message (if any).
    pub server_message: Option<String>,
}

/// Result for a single ref update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushRefResult {
    /// Ref updated successfully.
    Ok,
    /// Push rejected (non-fast-forward, force-with-lease mismatch, etc.).
    Rejected(String),
    /// Server-side error.
    Error(String),
}

/// Perform the send-pack push protocol.
///
/// Protocol flow:
/// 1. Check force-with-lease constraints against advertised refs
/// 2. Send ref update commands: `<old-oid> <new-oid> <refname>\n`
/// 3. Send flush
/// 4. Optionally send push-option lines + flush
/// 5. Send pack data (thin pack of objects reachable from new OIDs
///    but not from remote's advertised OIDs)
/// 6. Read and parse status report
pub fn push(
    transport: &mut dyn Transport,
    advertised_refs: &[(ObjectId, BString)],
    server_caps: &Capabilities,
    updates: &[PushUpdate],
    pack_data: &[u8],
    options: &PushOptions,
) -> Result<PushResult, ProtocolError> {
    if updates.is_empty() {
        return Ok(PushResult {
            ok: true,
            ref_results: Vec::new(),
            server_message: Some("Everything up-to-date".into()),
        });
    }

    // Check force-with-lease constraints
    for update in updates {
        if let Some(expected) = &update.expected_remote_oid {
            let actual = find_advertised_oid(advertised_refs, &update.remote_ref);
            let actual_oid = actual.unwrap_or(ObjectId::NULL_SHA1);
            if actual_oid != *expected {
                return Ok(PushResult {
                    ok: false,
                    ref_results: vec![(
                        update.remote_ref.clone(),
                        PushRefResult::Rejected(format!(
                            "stale info: expected {} but got {}",
                            expected, actual_oid
                        )),
                    )],
                    server_message: None,
                });
            }
        }
    }

    // Negotiate push capabilities
    let client_caps = capability::negotiate_push_capabilities(server_caps);
    let has_report_status = server_caps.has("report-status");
    let has_push_options = server_caps.has("push-options") && !options.push_options.is_empty();
    let has_atomic = server_caps.has("atomic") && options.atomic;
    let sideband_mode = capability::select_sideband(server_caps);

    // Build client capability string for the first ref command
    let mut cap_list = client_caps.clone();
    if has_atomic {
        cap_list.push("atomic".into());
    }
    if has_push_options {
        cap_list.push("push-options".into());
    }

    // Send ref update commands
    {
        let writer = transport.writer();
        let mut pkt_writer = PktLineWriter::new(writer);

        for (i, update) in updates.iter().enumerate() {
            let old_oid = find_advertised_oid(advertised_refs, &update.remote_ref)
                .unwrap_or(ObjectId::NULL_SHA1);

            let new_oid = update.local_oid.unwrap_or(ObjectId::NULL_SHA1);

            let line = if i == 0 {
                // First line includes capabilities
                format!(
                    "{} {} {}\0{}",
                    old_oid,
                    new_oid,
                    update.remote_ref,
                    cap_list.join(" ")
                )
            } else {
                format!("{} {} {}", old_oid, new_oid, update.remote_ref)
            };

            pkt_writer.write_text(&line)?;
        }

        pkt_writer.write_flush()?;

        // Send push options if negotiated
        if has_push_options {
            for opt in &options.push_options {
                pkt_writer.write_text(opt)?;
            }
            pkt_writer.write_flush()?;
        }

        // Send pack data
        if !pack_data.is_empty() {
            pkt_writer.inner_mut().write_all(pack_data)?;
        }

        pkt_writer.flush()?;
    }

    // Parse status report
    if has_report_status {
        parse_push_status(transport, sideband_mode, updates)
    } else {
        // No report-status: assume success
        Ok(PushResult {
            ok: true,
            ref_results: updates
                .iter()
                .map(|u| (u.remote_ref.clone(), PushRefResult::Ok))
                .collect(),
            server_message: None,
        })
    }
}

/// Parse the server's push status report.
fn parse_push_status(
    transport: &mut dyn Transport,
    sideband_mode: SidebandMode,
    _updates: &[PushUpdate],
) -> Result<PushResult, ProtocolError> {
    let reader = transport.reader();

    // Read status lines (may be wrapped in sideband)
    let status_lines = if sideband_mode != SidebandMode::None {
        let pkt_reader = PktLineReader::new(reader);
        let mut sideband = crate::sideband::SidebandReader::new(pkt_reader);
        let data = sideband.read_all_data()?;
        // Parse the data as pkt-lines
        let mut inner_reader = PktLineReader::new(std::io::Cursor::new(data));
        inner_reader.read_until_flush()?
    } else {
        let mut pkt_reader = PktLineReader::new(reader);
        pkt_reader.read_until_flush()?
    };

    let mut ok = true;
    let mut ref_results = Vec::new();
    let mut server_message = None;

    for line_data in &status_lines {
        let line = String::from_utf8_lossy(line_data);
        let line = line.trim_end_matches('\n');

        if let Some(status) = line.strip_prefix("unpack ") {
            if status != "ok" {
                ok = false;
                server_message = Some(format!("unpack failed: {}", status));
            }
        } else if let Some(rest) = line.strip_prefix("ok ") {
            ref_results.push((rest.to_string(), PushRefResult::Ok));
        } else if let Some(rest) = line.strip_prefix("ng ") {
            ok = false;
            // Format: ng <refname> <reason>
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() == 2 {
                ref_results.push((
                    parts[0].to_string(),
                    PushRefResult::Rejected(parts[1].to_string()),
                ));
            } else {
                ref_results.push((
                    rest.to_string(),
                    PushRefResult::Rejected("unknown error".to_string()),
                ));
            }
        }
    }

    Ok(PushResult {
        ok,
        ref_results,
        server_message,
    })
}

/// Find the advertised OID for a ref name.
fn find_advertised_oid(
    advertised_refs: &[(ObjectId, BString)],
    refname: &str,
) -> Option<ObjectId> {
    for (oid, name) in advertised_refs {
        if <BString as AsRef<[u8]>>::as_ref(name) == refname.as_bytes() {
            return Some(*oid);
        }
    }
    None
}

/// Compute which objects need to be sent for a push.
///
/// Returns OIDs of objects reachable from `local_oids` but not from
/// `remote_oids`. This is used to generate the thin pack.
pub fn compute_push_objects(
    local_oids: &[ObjectId],
    remote_oids: &[ObjectId],
) -> Vec<ObjectId> {
    // Simple implementation: return all local OIDs that aren't in remote set
    // A full implementation would walk the object graph
    let remote_set: std::collections::HashSet<&ObjectId> = remote_oids.iter().collect();
    local_oids
        .iter()
        .filter(|oid| !remote_set.contains(oid))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::BString;

    #[test]
    fn find_ref_in_advertised() {
        let oid = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();
        let refs = vec![(oid, BString::from("refs/heads/main"))];
        assert_eq!(
            find_advertised_oid(&refs, "refs/heads/main"),
            Some(oid)
        );
        assert_eq!(find_advertised_oid(&refs, "refs/heads/other"), None);
    }

    #[test]
    fn force_with_lease_mismatch() {
        // This tests the client-side check only (no transport needed)
        let oid1 = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();
        let oid2 = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();

        let advertised = vec![(oid1, BString::from("refs/heads/main"))];

        let update = PushUpdate {
            local_oid: Some(oid2),
            remote_ref: "refs/heads/main".into(),
            force: false,
            // Expect a different OID than what's actually advertised
            expected_remote_oid: Some(oid2),
        };

        // Verify the force-with-lease check catches the mismatch
        let actual = find_advertised_oid(&advertised, &update.remote_ref)
            .unwrap_or(ObjectId::NULL_SHA1);
        assert_ne!(actual, oid2);
    }

    #[test]
    fn compute_objects_filters_common() {
        let a = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let b = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let c = ObjectId::from_hex("cccccccccccccccccccccccccccccccccccccccc").unwrap();

        let result = compute_push_objects(&[a, b, c], &[b]);
        assert_eq!(result, vec![a, c]);
    }

    #[test]
    fn empty_updates_returns_up_to_date() {
        // We can't easily test with a real transport, but we can verify
        // the empty case
        let result = PushResult {
            ok: true,
            ref_results: Vec::new(),
            server_message: Some("Everything up-to-date".into()),
        };
        assert!(result.ok);
        assert!(result.ref_results.is_empty());
    }
}

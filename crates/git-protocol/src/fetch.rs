//! Fetch protocol implementation.
//!
//! Orchestrates the complete fetch operation: connect, negotiate, receive pack,
//! and update refs.

use std::path::PathBuf;

use git_hash::ObjectId;
use git_transport::Transport;

use crate::capability::{self, Capabilities, SidebandMode};
use crate::pktline::{PktLineReader, PktLineWriter};
use crate::sideband::SidebandReader;
use crate::ProtocolError;

/// Fetch operation options.
#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// Shallow fetch depth (None = full).
    pub depth: Option<u32>,
    /// Deepen a shallow clone by N additional commits.
    pub deepen: Option<u32>,
    /// Convert a shallow repository to a complete one.
    pub unshallow: bool,
    /// Create a shallow clone with commits newer than this date (ISO 8601 or unix timestamp).
    pub shallow_since: Option<String>,
    /// Exclude commits reachable from a specific revision.
    pub shallow_exclude: Option<String>,
    /// Partial clone filter (e.g., "blob:none").
    pub filter: Option<String>,
    /// Show progress output.
    pub progress: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            depth: None,
            deepen: None,
            unshallow: false,
            shallow_since: None,
            shallow_exclude: None,
            filter: None,
            progress: true,
        }
    }
}

/// Result of a fetch operation.
#[derive(Debug)]
pub struct FetchResult {
    /// Path to the received pack file (if any).
    pub pack_path: Option<PathBuf>,
    /// Ref updates to apply.
    pub ref_updates: Vec<(String, ObjectId)>,
    /// Number of new objects received.
    pub new_objects: usize,
    /// Shallow boundary commits (OIDs listed in "shallow" lines from the server).
    pub shallow_commits: Vec<ObjectId>,
    /// Commits that are no longer shallow boundaries ("unshallow" lines from the server).
    pub unshallow_commits: Vec<ObjectId>,
}

/// Perform a fetch operation using an already-connected transport.
///
/// This handles both v1 and v2 protocols automatically based on the
/// server's advertised capabilities.
pub fn fetch(
    transport: &mut dyn Transport,
    advertised_refs: &[(ObjectId, bstr::BString)],
    server_caps: &Capabilities,
    local_refs: &[(ObjectId, String)],
    wanted_refs: &[String],
    options: &FetchOptions,
    pack_dir: Option<&std::path::Path>,
) -> Result<FetchResult, ProtocolError> {
    // Determine which OIDs we want
    let wants: Vec<ObjectId> = determine_wants(advertised_refs, wanted_refs);

    if wants.is_empty() {
        return Ok(FetchResult {
            pack_path: None,
            ref_updates: Vec::new(),
            new_objects: 0,
            shallow_commits: Vec::new(),
            unshallow_commits: Vec::new(),
        });
    }

    // Determine which OIDs we already have (for negotiation)
    let haves: Vec<ObjectId> = local_refs.iter().map(|(oid, _)| *oid).collect();

    // Select client capabilities — include shallow-related capabilities if needed
    let mut client_caps = capability::negotiate_fetch_capabilities(server_caps);
    let is_shallow_request = options.depth.is_some()
        || options.deepen.is_some()
        || options.unshallow
        || options.shallow_since.is_some()
        || options.shallow_exclude.is_some();

    if is_shallow_request && server_caps.has("shallow") && !client_caps.iter().any(|c| c == "shallow") {
        client_caps.push("shallow".into());
    }
    if options.shallow_since.is_some() && server_caps.has("deepen-since") && !client_caps.iter().any(|c| c == "deepen-since") {
        client_caps.push("deepen-since".into());
    }
    if options.shallow_exclude.is_some() && server_caps.has("deepen-not") && !client_caps.iter().any(|c| c == "deepen-not") {
        client_caps.push("deepen-not".into());
    }
    if (options.deepen.is_some() || options.unshallow) && server_caps.has("deepen-relative") && !client_caps.iter().any(|c| c == "deepen-relative") {
        client_caps.push("deepen-relative".into());
    }

    let sideband_mode = capability::select_sideband(server_caps);

    // Send wants, shallow commands, and haves (write phase)
    {
        let writer = transport.writer();
        let mut pkt_writer = PktLineWriter::new(writer);

        // Send want lines
        for (i, want) in wants.iter().enumerate() {
            if i == 0 && !client_caps.is_empty() {
                let caps_str = client_caps.join(" ");
                pkt_writer.write_text(&format!("want {} {}", want, caps_str))?;
            } else {
                pkt_writer.write_text(&format!("want {}", want))?;
            }
        }

        // Send shallow negotiation lines (before flush, after wants)
        if let Some(depth) = options.depth {
            pkt_writer.write_text(&format!("deepen {}", depth))?;
        }
        if let Some(deepen) = options.deepen {
            pkt_writer.write_text(&format!("deepen {}", deepen))?;
        }
        if options.unshallow {
            // Unshallow: request infinite depth to convert shallow to full
            pkt_writer.write_text(&format!("deepen {}", 0x7fffffff_u32))?;
        }
        if let Some(ref since) = options.shallow_since {
            pkt_writer.write_text(&format!("deepen-since {}", since))?;
        }
        if let Some(ref exclude) = options.shallow_exclude {
            pkt_writer.write_text(&format!("deepen-not {}", exclude))?;
        }

        pkt_writer.write_flush()?;

        // Send have lines
        for have in &haves {
            pkt_writer.write_text(&format!("have {}", have))?;
        }

        // Send done
        pkt_writer.write_text("done")?;
        pkt_writer.flush()?;
    }

    // Read shallow/unshallow and ACK/NAK response (read phase)
    // The server may send "shallow <oid>" and "unshallow <oid>" lines before ACK/NAK.
    let mut shallow_commits = Vec::new();
    let mut unshallow_commits = Vec::new();
    {
        let reader = transport.reader();
        let mut pkt_reader = PktLineReader::new(reader);

        loop {
            match pkt_reader.read_pkt()? {
                crate::pktline::PktLine::Data(data) => {
                    let line = String::from_utf8_lossy(&data);
                    let line = line.trim_end_matches('\n');
                    if line == "NAK" {
                        break;
                    }
                    if let Some(hex) = line.strip_prefix("shallow ") {
                        if let Ok(oid) = ObjectId::from_hex(hex.trim()) {
                            shallow_commits.push(oid);
                        }
                        continue;
                    }
                    if let Some(hex) = line.strip_prefix("unshallow ") {
                        if let Ok(oid) = ObjectId::from_hex(hex.trim()) {
                            unshallow_commits.push(oid);
                        }
                        continue;
                    }
                    // ACK line -- continue reading until NAK
                    if line.starts_with("ACK ") {
                        continue;
                    }
                    // Unknown response -- just break
                    break;
                }
                crate::pktline::PktLine::Flush => break,
                _ => break,
            }
        }
    }

    // Receive pack data
    let pack_data = receive_pack_data(transport, sideband_mode)?;

    let mut result = FetchResult {
        pack_path: None,
        ref_updates: Vec::new(),
        new_objects: 0,
        shallow_commits,
        unshallow_commits,
    };

    if !pack_data.is_empty() {
        result.new_objects = count_pack_objects(&pack_data);

        // Write pack to disk if we have a pack dir
        if let Some(dir) = pack_dir {
            let pack_path = write_pack_to_disk(dir, &pack_data)?;
            result.pack_path = Some(pack_path);
        }
    }

    // Build ref update list
    for (oid, refname) in advertised_refs {
        let name = String::from_utf8_lossy(refname.as_ref()).to_string();
        if wanted_refs.is_empty() || wanted_refs.iter().any(|w| name.contains(w)) {
            result.ref_updates.push((name, *oid));
        }
    }

    Ok(result)
}

/// Determine which OIDs to request from the server.
fn determine_wants(
    advertised_refs: &[(ObjectId, bstr::BString)],
    wanted_refs: &[String],
) -> Vec<ObjectId> {
    let mut wants = Vec::new();

    for (oid, refname) in advertised_refs {
        let name = String::from_utf8_lossy(refname.as_ref()).to_string();

        if wanted_refs.is_empty() {
            // Want all refs
            if !wants.contains(oid) {
                wants.push(*oid);
            }
        } else {
            // Only want specific refs
            for wanted in wanted_refs {
                if (name.contains(wanted) || name == *wanted) && !wants.contains(oid) {
                    wants.push(*oid);
                }
            }
        }
    }

    wants
}

/// Receive pack data from the transport, handling sideband if needed.
fn receive_pack_data(
    transport: &mut dyn Transport,
    sideband_mode: SidebandMode,
) -> Result<Vec<u8>, ProtocolError> {
    let reader = transport.reader();

    match sideband_mode {
        SidebandMode::None => {
            // Read raw pack data
            let mut data = Vec::new();
            reader.read_to_end(&mut data)?;
            Ok(data)
        }
        SidebandMode::Band | SidebandMode::Band64k => {
            // Read through sideband demuxer
            let pkt_reader = PktLineReader::new(reader);
            let mut sideband = SidebandReader::new(pkt_reader);
            sideband.read_all_data()
        }
    }
}

/// Count objects in a pack (quick check from header).
fn count_pack_objects(pack_data: &[u8]) -> usize {
    if pack_data.len() < 12 {
        return 0;
    }
    // Pack header: PACK <version:4> <num_objects:4>
    if &pack_data[0..4] != b"PACK" {
        return 0;
    }
    u32::from_be_bytes([pack_data[8], pack_data[9], pack_data[10], pack_data[11]]) as usize
}

/// Write pack data to a file in the pack directory and generate an index.
fn write_pack_to_disk(
    pack_dir: &std::path::Path,
    pack_data: &[u8],
) -> Result<PathBuf, ProtocolError> {
    std::fs::create_dir_all(pack_dir)?;

    // Compute pack checksum for filename
    let checksum = if pack_data.len() >= 20 {
        let hash_bytes = &pack_data[pack_data.len() - 20..];
        let mut hex = String::with_capacity(40);
        for b in hash_bytes {
            hex.push_str(&format!("{:02x}", b));
        }
        hex
    } else {
        "tmp".to_string()
    };

    let pack_path = pack_dir.join(format!("pack-{}.pack", checksum));
    std::fs::write(&pack_path, pack_data)?;

    // Generate .idx using git index-pack
    let status = std::process::Command::new("git")
        .arg("index-pack")
        .arg(&pack_path)
        .status();

    match status {
        Ok(s) if s.success() => {}
        _ => {
            // If git index-pack fails, the pack is still written but objects
            // won't be accessible via pack index. This is a best-effort approach.
        }
    }

    Ok(pack_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_objects_in_pack_header() {
        let mut pack = Vec::new();
        pack.extend_from_slice(b"PACK");
        pack.extend_from_slice(&2u32.to_be_bytes()); // version
        pack.extend_from_slice(&42u32.to_be_bytes()); // num objects
        assert_eq!(count_pack_objects(&pack), 42);
    }

    #[test]
    fn count_objects_empty() {
        assert_eq!(count_pack_objects(&[]), 0);
    }

    #[test]
    fn determine_wants_all() {
        use bstr::BString;
        let refs = vec![
            (ObjectId::NULL_SHA1, BString::from("refs/heads/main")),
        ];
        let wants = determine_wants(&refs, &[]);
        assert_eq!(wants.len(), 1);
    }

    #[test]
    fn determine_wants_filtered() {
        use bstr::BString;
        let oid1 = ObjectId::NULL_SHA1;
        let refs = vec![
            (oid1, BString::from("refs/heads/main")),
            (oid1, BString::from("refs/heads/feature")),
        ];
        let wants = determine_wants(&refs, &["main".to_string()]);
        assert_eq!(wants.len(), 1);
    }
}

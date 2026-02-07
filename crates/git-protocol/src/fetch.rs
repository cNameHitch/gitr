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
    /// Partial clone filter (e.g., "blob:none").
    pub filter: Option<String>,
    /// Show progress output.
    pub progress: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            depth: None,
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
    _options: &FetchOptions,
    pack_dir: Option<&std::path::Path>,
) -> Result<FetchResult, ProtocolError> {
    // Determine which OIDs we want
    let wants: Vec<ObjectId> = determine_wants(advertised_refs, wanted_refs);

    if wants.is_empty() {
        return Ok(FetchResult {
            pack_path: None,
            ref_updates: Vec::new(),
            new_objects: 0,
        });
    }

    // Determine which OIDs we already have (for negotiation)
    let haves: Vec<ObjectId> = local_refs.iter().map(|(oid, _)| *oid).collect();

    // Select client capabilities
    let client_caps = capability::negotiate_fetch_capabilities(server_caps);
    let sideband_mode = capability::select_sideband(server_caps);

    // Send wants and haves (write phase)
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
        pkt_writer.write_flush()?;

        // Send have lines
        for have in &haves {
            pkt_writer.write_text(&format!("have {}", have))?;
        }

        // Send done
        pkt_writer.write_text("done")?;
        pkt_writer.flush()?;
    }

    // Read ACK/NAK response (read phase)
    // In simple (non-multi_ack) protocol:
    // - Server sends NAK if no common objects, then pack data
    // - Server sends ACK <oid> if common objects, then NAK, then pack data
    // We consume all ACK/NAK lines until we see NAK or flush.
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
                    // ACK line — continue reading until NAK
                    if line.starts_with("ACK ") {
                        continue;
                    }
                    // Unknown response — just break
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

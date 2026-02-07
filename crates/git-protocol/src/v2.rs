//! Protocol v2 implementation.
//!
//! Protocol v2 uses a command-based architecture:
//! - `ls-refs`: List references with server-side filtering
//! - `fetch`: Fetch objects with incremental negotiation

use bstr::BString;
use git_hash::ObjectId;

use crate::capability::Capabilities;
use crate::pktline::{PktLine, PktLineReader, PktLineWriter};
use crate::ProtocolError;

/// V2 commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V2Command {
    LsRefs,
    Fetch,
}

impl V2Command {
    pub fn as_str(&self) -> &'static str {
        match self {
            V2Command::LsRefs => "ls-refs",
            V2Command::Fetch => "fetch",
        }
    }
}

/// Options for ls-refs command.
#[derive(Debug, Clone, Default)]
pub struct LsRefsOptions {
    /// Ref prefixes to filter by (server-side filtering).
    pub ref_prefixes: Vec<String>,
    /// Whether to include symrefs in output.
    pub symrefs: bool,
    /// Whether to include peeled OIDs.
    pub peel: bool,
}

/// Perform a v2 ls-refs command.
pub fn ls_refs<W: std::io::Write, R: std::io::Read>(
    writer: &mut PktLineWriter<W>,
    reader: &mut PktLineReader<R>,
    options: &LsRefsOptions,
) -> Result<Vec<(ObjectId, BString)>, ProtocolError> {
    // Send command
    writer.write_text("command=ls-refs")?;
    writer.write_delimiter()?;

    // Send arguments
    if options.symrefs {
        writer.write_text("symrefs")?;
    }
    if options.peel {
        writer.write_text("peel")?;
    }
    for prefix in &options.ref_prefixes {
        writer.write_text(&format!("ref-prefix {}", prefix))?;
    }
    writer.write_flush()?;
    writer.flush()?;

    // Read response
    let mut refs = Vec::new();
    loop {
        match reader.read_pkt()? {
            PktLine::Flush | PktLine::ResponseEnd => break,
            PktLine::Delimiter => break,
            PktLine::Data(data) => {
                let line = strip_newline(&data);
                // Format: <oid> <refname> [symref-target:<target>] [peeled:<oid>]
                let line_str = std::str::from_utf8(line).map_err(|_| {
                    ProtocolError::Protocol("invalid UTF-8 in ls-refs response".into())
                })?;

                let parts: Vec<&str> = line_str.splitn(3, ' ').collect();
                if parts.len() < 2 {
                    continue;
                }

                let oid = ObjectId::from_hex(parts[0]).map_err(|e| {
                    ProtocolError::Protocol(format!("invalid OID in ls-refs: {}", e))
                })?;
                let refname = parts[1];
                refs.push((oid, BString::from(refname.as_bytes())));
            }
        }
    }

    Ok(refs)
}

/// Options for v2 fetch command.
#[derive(Debug, Clone, Default)]
pub struct V2FetchOptions {
    /// Object IDs the client wants.
    pub wants: Vec<ObjectId>,
    /// Object IDs the client already has.
    pub haves: Vec<ObjectId>,
    /// Whether to signal "done" (no more haves).
    pub done: bool,
    /// Depth limit for shallow fetch.
    pub depth: Option<u32>,
    /// Filter expression (e.g., "blob:none").
    pub filter: Option<String>,
    /// Whether to include tags.
    pub include_tag: bool,
    /// Thin pack.
    pub thin_pack: bool,
    /// Ofs-delta.
    pub ofs_delta: bool,
}

/// Result of a v2 fetch command.
#[derive(Debug)]
pub struct V2FetchResult {
    /// Acknowledgments from the server.
    pub acks: Vec<ObjectId>,
    /// Whether the server is ready to send the pack.
    pub ready: bool,
    /// Pack data (if ready).
    pub pack_data: Vec<u8>,
    /// Shallow boundary updates.
    pub shallow: Vec<ObjectId>,
    pub unshallow: Vec<ObjectId>,
}

/// Perform a v2 fetch command.
pub fn fetch_v2<W: std::io::Write, R: std::io::Read>(
    writer: &mut PktLineWriter<W>,
    reader: &mut PktLineReader<R>,
    options: &V2FetchOptions,
    server_caps: &Capabilities,
) -> Result<V2FetchResult, ProtocolError> {
    // Send command
    writer.write_text("command=fetch")?;

    // Send capability list
    if server_caps.get("fetch").is_some() {
        writer.write_text("agent=gitr/0.1")?;
    }
    writer.write_delimiter()?;

    // Send arguments
    if options.thin_pack {
        writer.write_text("thin-pack")?;
    }
    if options.ofs_delta {
        writer.write_text("ofs-delta")?;
    }
    if options.include_tag {
        writer.write_text("include-tag")?;
    }

    for want in &options.wants {
        writer.write_text(&format!("want {}", want))?;
    }

    for have in &options.haves {
        writer.write_text(&format!("have {}", have))?;
    }

    if let Some(depth) = options.depth {
        writer.write_text(&format!("deepen {}", depth))?;
    }

    if let Some(ref filter) = options.filter {
        writer.write_text(&format!("filter {}", filter))?;
    }

    if options.done {
        writer.write_text("done")?;
    }

    writer.write_flush()?;
    writer.flush()?;

    // Read response sections
    let mut result = V2FetchResult {
        acks: Vec::new(),
        ready: false,
        pack_data: Vec::new(),
        shallow: Vec::new(),
        unshallow: Vec::new(),
    };

    // Read section by section
    loop {
        match reader.read_pkt()? {
            PktLine::Flush | PktLine::ResponseEnd => break,
            PktLine::Delimiter => continue,
            PktLine::Data(data) => {
                let line = strip_newline(&data);
                let line_str = String::from_utf8_lossy(line);

                if line_str == "acknowledgments" || line_str == "packfile"
                    || line_str == "shallow-info"
                {
                    // Section header — read section contents
                    let section = line_str.to_string();
                    read_v2_section(reader, &section, &mut result)?;
                } else if line_str.starts_with("ACK ") {
                    if let Some(oid_str) = line_str.strip_prefix("ACK ") {
                        if let Ok(oid) = ObjectId::from_hex(oid_str.trim()) {
                            result.acks.push(oid);
                        }
                    }
                } else if line_str == "ready" {
                    result.ready = true;
                } else if line_str == "NAK" {
                    // No common objects
                }
            }
        }
    }

    Ok(result)
}

fn read_v2_section<R: std::io::Read>(
    reader: &mut PktLineReader<R>,
    section: &str,
    result: &mut V2FetchResult,
) -> Result<(), ProtocolError> {
    loop {
        match reader.read_pkt()? {
            PktLine::Flush | PktLine::ResponseEnd => return Ok(()),
            PktLine::Delimiter => return Ok(()),
            PktLine::Data(data) => {
                let line = strip_newline(&data);
                let line_str = String::from_utf8_lossy(line);

                match section {
                    "acknowledgments" => {
                        if line_str.starts_with("ACK ") {
                            if let Some(oid_str) = line_str.strip_prefix("ACK ") {
                                if let Ok(oid) = ObjectId::from_hex(oid_str.trim()) {
                                    result.acks.push(oid);
                                }
                            }
                        } else if line_str == "ready" {
                            result.ready = true;
                        }
                    }
                    "packfile" => {
                        // Packfile data is sideband-multiplexed
                        if !data.is_empty() {
                            let band = data[0];
                            if band == 1 {
                                result.pack_data.extend_from_slice(&data[1..]);
                            }
                            // band 2 = progress, band 3 = error
                        }
                    }
                    "shallow-info" => {
                        if let Some(oid_str) = line_str.strip_prefix("shallow ") {
                            if let Ok(oid) = ObjectId::from_hex(oid_str.trim()) {
                                result.shallow.push(oid);
                            }
                        } else if let Some(oid_str) = line_str.strip_prefix("unshallow ") {
                            if let Ok(oid) = ObjectId::from_hex(oid_str.trim()) {
                                result.unshallow.push(oid);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn strip_newline(data: &[u8]) -> &[u8] {
    if data.last() == Some(&b'\n') {
        &data[..data.len() - 1]
    } else {
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn ls_refs_sends_correct_format() {
        let mut send_buf = Vec::new();
        let mut writer = PktLineWriter::new(&mut send_buf);

        // Build a mock server response
        let mut resp_buf = Vec::new();
        {
            let mut sw = PktLineWriter::new(&mut resp_buf);
            sw.write_text("95d09f2b10159347eece71399a7e2e907ea3df4f refs/heads/main")
                .unwrap();
            sw.write_flush().unwrap();
        }

        let mut reader = PktLineReader::new(Cursor::new(resp_buf));

        let options = LsRefsOptions {
            ref_prefixes: vec!["refs/heads/".into()],
            symrefs: true,
            peel: false,
        };

        let refs = ls_refs(&mut writer, &mut reader, &options).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].1.as_slice(), b"refs/heads/main");
    }
}

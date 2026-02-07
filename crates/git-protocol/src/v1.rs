//! Protocol v1 (and v0) implementation.
//!
//! Handles the v1 reference advertisement parsing and fetch negotiation
//! (want/have/ACK exchange).

use bstr::BString;
use git_hash::ObjectId;
use git_transport::{HandshakeResult, ProtocolVersion, Service, Transport};

use crate::capability::Capabilities;
use crate::pktline::{PktLine, PktLineReader, PktLineWriter};
use crate::ProtocolError;

/// Parse the initial v1 ref advertisement from a transport.
///
/// This reads the server's ref list and capabilities. The first ref line
/// contains capabilities after a NUL byte.
///
/// Returns the parsed handshake result.
pub fn parse_ref_advertisement<R: std::io::Read>(
    reader: &mut PktLineReader<R>,
) -> Result<(Vec<(ObjectId, BString)>, Capabilities), ProtocolError> {
    let mut refs = Vec::new();
    let mut capabilities = Capabilities::default();
    let mut first_line = true;

    loop {
        match reader.read_pkt()? {
            PktLine::Flush | PktLine::Delimiter | PktLine::ResponseEnd => break,
            PktLine::Data(data) => {
                let line = strip_newline(&data);

                if first_line {
                    first_line = false;

                    // Check for "version 2" response
                    if line.starts_with(b"version 2") {
                        // This is actually a v2 handshake — read capability lines
                        let cap_lines = reader.read_until_flush()?;
                        capabilities = Capabilities::parse_v2(&cap_lines);
                        return Ok((refs, capabilities));
                    }

                    // v1: first line has caps after NUL
                    if let Some(nul_pos) = line.iter().position(|&b| b == 0) {
                        let ref_part = &line[..nul_pos];
                        let caps_str = String::from_utf8_lossy(&line[nul_pos + 1..]);
                        capabilities = Capabilities::parse_v1(&caps_str);
                        parse_ref_line(ref_part, &mut refs)?;
                    } else {
                        parse_ref_line(line, &mut refs)?;
                    }
                } else {
                    parse_ref_line(line, &mut refs)?;
                }
            }
        }
    }

    Ok((refs, capabilities))
}

/// Parse a single ref advertisement line: `<40-hex-oid> <refname>`
fn parse_ref_line(line: &[u8], refs: &mut Vec<(ObjectId, BString)>) -> Result<(), ProtocolError> {
    // Skip comment lines (e.g., "# service=git-upload-pack")
    if line.starts_with(b"#") || line.is_empty() {
        return Ok(());
    }

    let space_pos = line.iter().position(|&b| b == b' ').ok_or_else(|| {
        ProtocolError::Protocol(format!(
            "invalid ref line (no space): {}",
            String::from_utf8_lossy(line)
        ))
    })?;

    let oid_hex = &line[..space_pos];
    let refname = &line[space_pos + 1..];

    let oid_str = std::str::from_utf8(oid_hex).map_err(|_| {
        ProtocolError::Protocol("invalid UTF-8 in OID".into())
    })?;

    let oid = ObjectId::from_hex(oid_str).map_err(|e| {
        ProtocolError::Protocol(format!("invalid OID in ref advertisement: {}", e))
    })?;

    refs.push((oid, BString::from(refname)));
    Ok(())
}

/// Perform v1 fetch handshake: read ref advertisement from the transport.
pub fn handshake(
    transport: &mut dyn Transport,
    _service: Service,
) -> Result<HandshakeResult, ProtocolError> {
    let mut pkt_reader = PktLineReader::new(transport.reader());

    // For HTTP, the initial response may start with a service announcement
    // like "# service=git-upload-pack\n" followed by a flush
    let (refs, capabilities) = parse_ref_advertisement(&mut pkt_reader)?;

    // Determine protocol version based on capabilities
    let protocol_version = if capabilities.has("version 2") || capabilities.get("agent").is_none() && refs.is_empty() {
        ProtocolVersion::V2
    } else {
        ProtocolVersion::V1
    };

    Ok(HandshakeResult {
        protocol_version,
        capabilities: capabilities
            .entries()
            .iter()
            .map(|e| {
                if let Some(ref v) = e.value {
                    format!("{}={}", e.name, v)
                } else {
                    e.name.clone()
                }
            })
            .collect(),
        refs,
        extra_lines: Vec::new(),
    })
}

/// V1 fetch negotiation: send wants and haves, receive ACKs.
///
/// Returns true if the server is ready to send the pack.
pub fn negotiate_fetch<W: std::io::Write, R: std::io::Read>(
    writer: &mut PktLineWriter<W>,
    reader: &mut PktLineReader<R>,
    wants: &[ObjectId],
    haves: &[ObjectId],
    client_caps: &[String],
) -> Result<bool, ProtocolError> {
    if wants.is_empty() {
        return Ok(false);
    }

    // Send wants (first want includes capabilities)
    for (i, want) in wants.iter().enumerate() {
        if i == 0 && !client_caps.is_empty() {
            let caps_str = client_caps.join(" ");
            writer.write_text(&format!("want {} {}", want, caps_str))?;
        } else {
            writer.write_text(&format!("want {}", want))?;
        }
    }
    writer.write_flush()?;

    // Send haves
    if !haves.is_empty() {
        for have in haves {
            writer.write_text(&format!("have {}", have))?;
        }
    }

    // Send done
    writer.write_text("done")?;
    writer.flush()?;

    // Read ACKs / NAK
    loop {
        match reader.read_pkt()? {
            PktLine::Data(data) => {
                let line = String::from_utf8_lossy(strip_newline(&data));
                if line == "NAK" {
                    // No common objects, server will send full pack
                    return Ok(true);
                }
                if line.starts_with("ACK ") {
                    // Server acknowledged a common object
                    // Continue reading until we get "ACK <oid> ready" or NAK
                    if line.contains(" ready") {
                        return Ok(true);
                    }
                    // Multi-ack: keep reading
                    continue;
                }
                // Some other response, just continue
            }
            PktLine::Flush => {
                // End of ACK section
                return Ok(true);
            }
            _ => break,
        }
    }

    Ok(true)
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
    use crate::pktline::PktLineWriter;
    use std::io::Cursor;

    fn make_ref_advertisement(refs: &[(&str, &str)], caps: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut writer = PktLineWriter::new(&mut buf);

        for (i, (oid, refname)) in refs.iter().enumerate() {
            if i == 0 && !caps.is_empty() {
                writer
                    .write_text(&format!("{} {}\0{}", oid, refname, caps))
                    .unwrap();
            } else {
                writer.write_text(&format!("{} {}", oid, refname)).unwrap();
            }
        }
        writer.write_flush().unwrap();
        buf
    }

    #[test]
    fn parse_simple_ref_advertisement() {
        let buf = make_ref_advertisement(
            &[
                (
                    "95d09f2b10159347eece71399a7e2e907ea3df4f",
                    "HEAD",
                ),
                (
                    "95d09f2b10159347eece71399a7e2e907ea3df4f",
                    "refs/heads/main",
                ),
            ],
            "multi_ack side-band-64k ofs-delta agent=git/2.39.0",
        );

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let (refs, caps) = parse_ref_advertisement(&mut reader).unwrap();

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].1.as_slice(), b"HEAD");
        assert_eq!(refs[1].1.as_slice(), b"refs/heads/main");
        assert!(caps.has("multi_ack"));
        assert!(caps.has("side-band-64k"));
        assert_eq!(caps.get("agent"), Some("git/2.39.0"));
    }

    #[test]
    fn parse_empty_ref_advertisement() {
        // Just a flush
        let buf = b"0000";
        let mut reader = PktLineReader::new(Cursor::new(&buf[..]));
        let (refs, _caps) = parse_ref_advertisement(&mut reader).unwrap();
        assert!(refs.is_empty());
    }

    #[test]
    fn negotiate_simple_fetch() {
        let want = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();

        // Build what the client will send
        let mut send_buf = Vec::new();
        let mut writer = PktLineWriter::new(&mut send_buf);

        // Build server response: NAK
        let mut server_resp = Vec::new();
        {
            let mut sw = PktLineWriter::new(&mut server_resp);
            sw.write_text("NAK").unwrap();
        }

        let mut reader = PktLineReader::new(Cursor::new(server_resp));
        let result = negotiate_fetch(
            &mut writer,
            &mut reader,
            &[want],
            &[],
            &["side-band-64k".to_string()],
        )
        .unwrap();

        assert!(result);
    }
}

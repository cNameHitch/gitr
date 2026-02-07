//! Integration tests for fetch protocol.

use std::io::Cursor;

use git_hash::ObjectId;
use git_protocol::pktline::{PktLineReader, PktLineWriter};
use git_protocol::v1;

/// Helper to build a v1 ref advertisement.
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
fn parse_ref_advertisement_with_capabilities() {
    let oid = "95d09f2b10159347eece71399a7e2e907ea3df4f";
    let buf = make_ref_advertisement(
        &[
            (oid, "HEAD"),
            (oid, "refs/heads/main"),
            (oid, "refs/heads/develop"),
            (oid, "refs/tags/v1.0"),
        ],
        "multi_ack_detailed thin-pack side-band-64k ofs-delta no-done include-tag symref=HEAD:refs/heads/main agent=git/2.39.0",
    );

    let mut reader = PktLineReader::new(Cursor::new(buf));
    let (refs, caps) = v1::parse_ref_advertisement(&mut reader).unwrap();

    assert_eq!(refs.len(), 4);
    assert!(caps.has("multi_ack_detailed"));
    assert!(caps.has("thin-pack"));
    assert!(caps.has("side-band-64k"));
    assert!(caps.has("ofs-delta"));
    assert!(caps.has("no-done"));
    assert!(caps.has("include-tag"));
    assert_eq!(caps.get("symref"), Some("HEAD:refs/heads/main"));
    assert_eq!(caps.get("agent"), Some("git/2.39.0"));
}

#[test]
fn parse_ref_advertisement_single_ref() {
    let oid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let buf = make_ref_advertisement(
        &[(oid, "HEAD")],
        "report-status delete-refs",
    );

    let mut reader = PktLineReader::new(Cursor::new(buf));
    let (refs, caps) = v1::parse_ref_advertisement(&mut reader).unwrap();

    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].0, ObjectId::from_hex(oid).unwrap());
    assert!(caps.has("report-status"));
    assert!(caps.has("delete-refs"));
}

#[test]
fn negotiate_fetch_with_no_common_objects() {
    let want = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();

    // Server responds with NAK (no common objects)
    let mut server_response = Vec::new();
    {
        let mut sw = PktLineWriter::new(&mut server_response);
        sw.write_text("NAK").unwrap();
    }

    let mut send_buf = Vec::new();
    let mut writer = PktLineWriter::new(&mut send_buf);
    let mut reader = PktLineReader::new(Cursor::new(server_response));

    let result = v1::negotiate_fetch(
        &mut writer,
        &mut reader,
        &[want],
        &[],
        &["side-band-64k".to_string(), "ofs-delta".to_string()],
    )
    .unwrap();

    assert!(result);

    // Verify what was sent
    let mut verify = PktLineReader::new(Cursor::new(send_buf));
    let lines = verify.read_until_flush().unwrap();
    assert_eq!(lines.len(), 1); // one want line
    let want_line = String::from_utf8_lossy(&lines[0]);
    assert!(want_line.starts_with("want 95d09f2b10159347eece71399a7e2e907ea3df4f"));
    assert!(want_line.contains("side-band-64k"));
    assert!(want_line.contains("ofs-delta"));
}

#[test]
fn negotiate_fetch_with_haves() {
    let want = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();
    let have = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();

    // Server responds with ACK then ready
    let mut server_response = Vec::new();
    {
        let mut sw = PktLineWriter::new(&mut server_response);
        sw.write_text(&format!("ACK {} ready", have)).unwrap();
    }

    let mut send_buf = Vec::new();
    let mut writer = PktLineWriter::new(&mut send_buf);
    let mut reader = PktLineReader::new(Cursor::new(server_response));

    let result = v1::negotiate_fetch(
        &mut writer,
        &mut reader,
        &[want],
        &[have],
        &["multi_ack_detailed".to_string()],
    )
    .unwrap();

    assert!(result);
}

#[test]
fn negotiate_fetch_empty_wants() {
    let mut send_buf = Vec::new();
    let mut writer = PktLineWriter::new(&mut send_buf);
    let server_response = Vec::new();
    let mut reader = PktLineReader::new(Cursor::new(server_response));

    let result = v1::negotiate_fetch(
        &mut writer,
        &mut reader,
        &[],  // no wants
        &[],
        &[],
    )
    .unwrap();

    // Should return false (nothing to fetch)
    assert!(!result);
}

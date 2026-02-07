//! Integration tests for pkt-line encoding/decoding.

use std::io::Cursor;

use git_protocol::pktline::{PktLine, PktLineReader, PktLineWriter, MAX_PKT_DATA_LEN};

#[test]
fn pktline_roundtrip_various_sizes() {
    // Test various data sizes from tiny to large
    let sizes = [0, 1, 4, 100, 1000, 65000, MAX_PKT_DATA_LEN];

    for size in sizes {
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_line(&data).unwrap();
        }

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let line = reader.read_line().unwrap().unwrap();
        assert_eq!(line, data, "roundtrip failed for size {}", size);
    }
}

#[test]
fn pktline_too_large_rejected() {
    let data = vec![0u8; MAX_PKT_DATA_LEN + 1];
    let mut buf = Vec::new();
    let mut writer = PktLineWriter::new(&mut buf);
    assert!(writer.write_line(&data).is_err());
}

#[test]
fn pktline_multiple_flush_sections() {
    let mut buf = Vec::new();
    {
        let mut w = PktLineWriter::new(&mut buf);
        w.write_text("section1-line1").unwrap();
        w.write_text("section1-line2").unwrap();
        w.write_flush().unwrap();
        w.write_text("section2-line1").unwrap();
        w.write_flush().unwrap();
        w.write_text("section3-line1").unwrap();
        w.write_text("section3-line2").unwrap();
        w.write_text("section3-line3").unwrap();
        w.write_flush().unwrap();
    }

    let mut reader = PktLineReader::new(Cursor::new(buf));

    let s1 = reader.read_until_flush().unwrap();
    assert_eq!(s1.len(), 2);
    assert_eq!(s1[0], b"section1-line1\n");
    assert_eq!(s1[1], b"section1-line2\n");

    let s2 = reader.read_until_flush().unwrap();
    assert_eq!(s2.len(), 1);

    let s3 = reader.read_until_flush().unwrap();
    assert_eq!(s3.len(), 3);
}

#[test]
fn pktline_v2_delimiter_and_response_end() {
    let mut buf = Vec::new();
    {
        let mut w = PktLineWriter::new(&mut buf);
        w.write_text("command=ls-refs").unwrap();
        w.write_delimiter().unwrap();
        w.write_text("ref-prefix refs/heads/").unwrap();
        w.write_flush().unwrap();
    }

    let mut reader = PktLineReader::new(Cursor::new(buf));

    // First section: command
    let pkt = reader.read_pkt().unwrap();
    assert!(matches!(pkt, PktLine::Data(_)));

    // Delimiter
    let pkt = reader.read_pkt().unwrap();
    assert_eq!(pkt, PktLine::Delimiter);

    // Second section: arguments
    let pkt = reader.read_pkt().unwrap();
    assert!(matches!(pkt, PktLine::Data(_)));

    // Flush
    let pkt = reader.read_pkt().unwrap();
    assert_eq!(pkt, PktLine::Flush);
}

#[test]
fn pktline_binary_data() {
    // Test with binary data containing all byte values
    let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let mut buf = Vec::new();
    {
        let mut writer = PktLineWriter::new(&mut buf);
        writer.write_line(&data).unwrap();
    }

    let mut reader = PktLineReader::new(Cursor::new(buf));
    let line = reader.read_line().unwrap().unwrap();
    assert_eq!(line, data);
}

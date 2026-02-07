//! Pkt-line protocol framing.
//!
//! The pkt-line format is the foundation of the git wire protocol. Each packet
//! is prefixed with a 4-hex-digit length (including the 4 bytes of the length
//! field itself). Special packets:
//! - `0000`: flush packet (end of section)
//! - `0001`: delimiter packet (v2 only)
//! - `0002`: response-end packet (v2 only)

use std::io::{Read, Write};

use crate::ProtocolError;

/// Maximum data per packet (65520 - 4 = 65516).
pub const MAX_PKT_DATA_LEN: usize = 65516;

/// Maximum packet length including the 4-byte header.
pub const MAX_PKT_LEN: usize = 65520;

/// Special packet types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PktLine {
    /// Normal data line.
    Data(Vec<u8>),
    /// Flush packet (0000) — end of section.
    Flush,
    /// Delimiter packet (0001) — v2 section separator.
    Delimiter,
    /// Response-end packet (0002) — v2 response terminator.
    ResponseEnd,
}

/// Pkt-line reader.
pub struct PktLineReader<R> {
    reader: R,
}

impl<R: Read> PktLineReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Get a reference to the inner reader.
    pub fn inner(&self) -> &R {
        &self.reader
    }

    /// Get a mutable reference to the inner reader.
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Consume the reader and return the inner value.
    pub fn into_inner(self) -> R {
        self.reader
    }

    /// Read one pkt-line. Returns the packet type.
    pub fn read_pkt(&mut self) -> Result<PktLine, ProtocolError> {
        // Read the 4-byte hex length
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                ProtocolError::Protocol("unexpected EOF reading pkt-line length".into())
            } else {
                ProtocolError::Io(e)
            }
        })?;

        let len_str = std::str::from_utf8(&len_buf).map_err(|_| {
            ProtocolError::InvalidPktLine(format!(
                "invalid pkt-line length bytes: {:?}",
                len_buf
            ))
        })?;

        let len = u16::from_str_radix(len_str, 16).map_err(|_| {
            ProtocolError::InvalidPktLine(format!(
                "invalid pkt-line length: {:?}",
                len_str
            ))
        })?;

        match len {
            0 => Ok(PktLine::Flush),
            1 => Ok(PktLine::Delimiter),
            2 => Ok(PktLine::ResponseEnd),
            3 => Err(ProtocolError::InvalidPktLine(
                "pkt-line length 3 is invalid (minimum data packet is 4)".into(),
            )),
            _ => {
                let data_len = (len as usize) - 4;
                if data_len > MAX_PKT_DATA_LEN {
                    return Err(ProtocolError::InvalidPktLine(format!(
                        "pkt-line too long: {} bytes",
                        data_len
                    )));
                }
                let mut data = vec![0u8; data_len];
                self.reader.read_exact(&mut data)?;
                Ok(PktLine::Data(data))
            }
        }
    }

    /// Read one data line. Returns None for flush packet.
    /// Returns error for delimiter/response-end (unexpected in v1 context).
    pub fn read_line(&mut self) -> Result<Option<Vec<u8>>, ProtocolError> {
        match self.read_pkt()? {
            PktLine::Data(data) => Ok(Some(data)),
            PktLine::Flush => Ok(None),
            PktLine::Delimiter => Ok(None),
            PktLine::ResponseEnd => Ok(None),
        }
    }

    /// Read all lines until a flush packet.
    pub fn read_until_flush(&mut self) -> Result<Vec<Vec<u8>>, ProtocolError> {
        let mut lines = Vec::new();
        while let PktLine::Data(data) = self.read_pkt()? {
            lines.push(data);
        }
        Ok(lines)
    }
}

/// Pkt-line writer.
pub struct PktLineWriter<W> {
    writer: W,
}

impl<W: Write> PktLineWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Get a reference to the inner writer.
    pub fn inner(&self) -> &W {
        &self.writer
    }

    /// Get a mutable reference to the inner writer.
    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Consume the writer and return the inner value.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Write a data packet.
    pub fn write_line(&mut self, data: &[u8]) -> Result<(), ProtocolError> {
        if data.len() > MAX_PKT_DATA_LEN {
            return Err(ProtocolError::InvalidPktLine(format!(
                "data too long for pkt-line: {} bytes (max {})",
                data.len(),
                MAX_PKT_DATA_LEN
            )));
        }

        let len = (data.len() + 4) as u16;
        write!(self.writer, "{:04x}", len)?;
        self.writer.write_all(data)?;
        Ok(())
    }

    /// Write a text line (appends \n if not present).
    pub fn write_text(&mut self, text: &str) -> Result<(), ProtocolError> {
        if text.ends_with('\n') {
            self.write_line(text.as_bytes())
        } else {
            let mut data = text.as_bytes().to_vec();
            data.push(b'\n');
            self.write_line(&data)
        }
    }

    /// Write a flush packet (0000).
    pub fn write_flush(&mut self) -> Result<(), ProtocolError> {
        self.writer.write_all(b"0000")?;
        Ok(())
    }

    /// Write a delimiter packet (0001, v2 only).
    pub fn write_delimiter(&mut self) -> Result<(), ProtocolError> {
        self.writer.write_all(b"0001")?;
        Ok(())
    }

    /// Write a response-end packet (0002, v2 only).
    pub fn write_response_end(&mut self) -> Result<(), ProtocolError> {
        self.writer.write_all(b"0002")?;
        Ok(())
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> Result<(), ProtocolError> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_and_read_data_line() {
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_line(b"hello").unwrap();
        }
        assert_eq!(&buf, b"0009hello");

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let line = reader.read_line().unwrap().unwrap();
        assert_eq!(line, b"hello");
    }

    #[test]
    fn write_and_read_text_line() {
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_text("hello").unwrap();
        }
        // "hello\n" is 6 bytes, + 4 = 10 = 000a
        assert_eq!(&buf, b"000ahello\n");
    }

    #[test]
    fn write_and_read_flush() {
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_flush().unwrap();
        }
        assert_eq!(&buf, b"0000");

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let pkt = reader.read_pkt().unwrap();
        assert_eq!(pkt, PktLine::Flush);
    }

    #[test]
    fn write_and_read_delimiter() {
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_delimiter().unwrap();
        }
        assert_eq!(&buf, b"0001");

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let pkt = reader.read_pkt().unwrap();
        assert_eq!(pkt, PktLine::Delimiter);
    }

    #[test]
    fn write_and_read_response_end() {
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_response_end().unwrap();
        }
        assert_eq!(&buf, b"0002");

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let pkt = reader.read_pkt().unwrap();
        assert_eq!(pkt, PktLine::ResponseEnd);
    }

    #[test]
    fn read_until_flush() {
        let data = b"000ahello\n000bworld!\n0000";
        let mut reader = PktLineReader::new(Cursor::new(&data[..]));
        let lines = reader.read_until_flush().unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], b"hello\n");
        assert_eq!(lines[1], b"world!\n");
    }

    #[test]
    fn multiple_sections_with_flush() {
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_text("line1").unwrap();
            writer.write_flush().unwrap();
            writer.write_text("line2").unwrap();
            writer.write_flush().unwrap();
        }

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let section1 = reader.read_until_flush().unwrap();
        assert_eq!(section1.len(), 1);
        assert_eq!(section1[0], b"line1\n");

        let section2 = reader.read_until_flush().unwrap();
        assert_eq!(section2.len(), 1);
        assert_eq!(section2[0], b"line2\n");
    }

    #[test]
    fn empty_data_line() {
        // Length 4 = 0004, meaning 0 bytes of data
        let data = b"0004";
        let mut reader = PktLineReader::new(Cursor::new(&data[..]));
        let line = reader.read_line().unwrap().unwrap();
        assert!(line.is_empty());
    }

    #[test]
    fn pkt_line_length_includes_header() {
        // "abc" = 3 bytes data + 4 header = 7 = 0007
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            writer.write_line(b"abc").unwrap();
        }
        assert_eq!(&buf[..4], b"0007");
    }

    #[test]
    fn real_git_ref_advertisement() {
        // Simulate a real git ref advertisement
        let mut buf = Vec::new();
        {
            let mut writer = PktLineWriter::new(&mut buf);
            // First line includes capabilities after NUL
            let first_line = b"95d09f2b10159347eece71399a7e2e907ea3df4f HEAD\0multi_ack thin-pack side-band side-band-64k ofs-delta shallow deepen-since deepen-not deepen-relative no-progress include-tag multi_ack_detailed no-done symref=HEAD:refs/heads/main agent=git/2.39.0\n";
            writer.write_line(first_line).unwrap();
            writer.write_line(b"95d09f2b10159347eece71399a7e2e907ea3df4f refs/heads/main\n").unwrap();
            writer.write_flush().unwrap();
        }

        let mut reader = PktLineReader::new(Cursor::new(buf));
        let lines = reader.read_until_flush().unwrap();
        assert_eq!(lines.len(), 2);
        // First line should contain NUL-separated caps
        assert!(lines[0].contains(&0));
    }
}

//! Sideband multiplexing for git protocol.
//!
//! During fetch/push, the server multiplexes data, progress, and error
//! messages over sideband channels:
//! - Band 1: pack data
//! - Band 2: progress messages (sent to stderr)
//! - Band 3: fatal error messages

use std::io::Read;

use crate::pktline::{PktLine, PktLineReader};
use crate::ProtocolError;

/// Sideband channel identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// Pack data (band 1).
    Data = 1,
    /// Progress messages (band 2).
    Progress = 2,
    /// Fatal error (band 3).
    Error = 3,
}

/// Callback for handling sideband progress/error messages.
pub type SidebandCallback = Box<dyn FnMut(Band, &[u8]) + Send>;

/// Sideband demultiplexer.
///
/// Reads pkt-lines and separates data (band 1) from progress (band 2)
/// and error (band 3) messages.
pub struct SidebandReader<R> {
    reader: PktLineReader<R>,
    callback: Option<SidebandCallback>,
}

impl<R: Read> SidebandReader<R> {
    pub fn new(reader: PktLineReader<R>) -> Self {
        Self {
            reader,
            callback: None,
        }
    }

    /// Set a callback for progress/error messages.
    pub fn with_callback(mut self, callback: SidebandCallback) -> Self {
        self.callback = Some(callback);
        self
    }

    /// Read the next data packet (band 1).
    ///
    /// Progress and error messages are dispatched to the callback.
    /// Returns None on flush packet (end of data).
    /// Returns error on band 3 (fatal error from server).
    pub fn read_data(&mut self) -> Result<Option<Vec<u8>>, ProtocolError> {
        loop {
            match self.reader.read_pkt()? {
                PktLine::Flush | PktLine::Delimiter | PktLine::ResponseEnd => {
                    return Ok(None);
                }
                PktLine::Data(data) => {
                    if data.is_empty() {
                        return Ok(None);
                    }

                    let band = data[0];
                    let payload = &data[1..];

                    match band {
                        1 => {
                            // Data band
                            return Ok(Some(payload.to_vec()));
                        }
                        2 => {
                            // Progress band
                            if let Some(ref mut cb) = self.callback {
                                cb(Band::Progress, payload);
                            } else {
                                // Default: write progress to stderr
                                let msg = String::from_utf8_lossy(payload);
                                eprint!("remote: {}", msg);
                            }
                        }
                        3 => {
                            // Error band
                            let msg = String::from_utf8_lossy(payload).to_string();
                            if let Some(ref mut cb) = self.callback {
                                cb(Band::Error, payload);
                            }
                            return Err(ProtocolError::ServerError(msg));
                        }
                        _ => {
                            return Err(ProtocolError::Protocol(format!(
                                "unknown sideband channel: {}",
                                band
                            )));
                        }
                    }
                }
            }
        }
    }

    /// Read all remaining data from band 1, collecting into a Vec.
    pub fn read_all_data(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let mut result = Vec::new();
        while let Some(chunk) = self.read_data()? {
            result.extend_from_slice(&chunk);
        }
        Ok(result)
    }

    /// Get the underlying pkt-line reader.
    pub fn into_inner(self) -> PktLineReader<R> {
        self.reader
    }
}

/// Write data with sideband framing.
pub fn write_sideband_data<W: std::io::Write>(
    writer: &mut crate::pktline::PktLineWriter<W>,
    band: Band,
    data: &[u8],
) -> Result<(), ProtocolError> {
    // Maximum payload per sideband packet: MAX_PKT_DATA_LEN - 1 (for band byte)
    let max_chunk = crate::pktline::MAX_PKT_DATA_LEN - 1;

    for chunk in data.chunks(max_chunk) {
        let mut pkt = Vec::with_capacity(1 + chunk.len());
        pkt.push(band as u8);
        pkt.extend_from_slice(chunk);
        writer.write_line(&pkt)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pktline::PktLineWriter;
    use std::io::Cursor;

    fn make_sideband_packet(band: u8, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut writer = PktLineWriter::new(&mut buf);
        let mut pkt = vec![band];
        pkt.extend_from_slice(data);
        writer.write_line(&pkt).unwrap();
        buf
    }

    #[test]
    fn read_data_band() {
        let mut buf = make_sideband_packet(1, b"pack data here");
        // Add flush
        buf.extend_from_slice(b"0000");

        let reader = PktLineReader::new(Cursor::new(buf));
        let mut sb = SidebandReader::new(reader);

        let data = sb.read_data().unwrap().unwrap();
        assert_eq!(data, b"pack data here");

        // Next read should return None (flush)
        assert!(sb.read_data().unwrap().is_none());
    }

    #[test]
    fn read_progress_band_skipped() {
        let mut buf = make_sideband_packet(2, b"Counting objects: 5\n");
        buf.extend_from_slice(&make_sideband_packet(1, b"actual data"));
        buf.extend_from_slice(b"0000");

        let reader = PktLineReader::new(Cursor::new(buf));
        let mut progress_msgs = Vec::new();
        let cb: SidebandCallback = Box::new(move |band, data| {
            if band == Band::Progress {
                progress_msgs.push(data.to_vec());
            }
        });
        let mut sb = SidebandReader::new(reader).with_callback(cb);

        // Should skip progress and return data
        let data = sb.read_data().unwrap().unwrap();
        assert_eq!(data, b"actual data");
    }

    #[test]
    fn read_error_band() {
        let mut buf = make_sideband_packet(3, b"repository not found");
        buf.extend_from_slice(b"0000");

        let reader = PktLineReader::new(Cursor::new(buf));
        let mut sb = SidebandReader::new(reader);

        let err = sb.read_data().unwrap_err();
        match err {
            ProtocolError::ServerError(msg) => {
                assert!(msg.contains("repository not found"));
            }
            _ => panic!("expected ServerError, got {:?}", err),
        }
    }

    #[test]
    fn read_all_data() {
        let mut buf = make_sideband_packet(1, b"chunk1");
        buf.extend_from_slice(&make_sideband_packet(1, b"chunk2"));
        buf.extend_from_slice(b"0000");

        let reader = PktLineReader::new(Cursor::new(buf));
        let mut sb = SidebandReader::new(reader);

        let data = sb.read_all_data().unwrap();
        assert_eq!(data, b"chunk1chunk2");
    }
}

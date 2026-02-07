use std::fs;
use std::io::Read;

use flate2::read::ZlibDecoder;
use git_object::header;
use git_object::ObjectType;

use crate::{LooseError, LooseObjectStore};

/// Streaming reader for a loose object.
///
/// Decompresses data on demand as [`Read`] is called.
/// The header has already been parsed; reads yield only the content bytes.
pub struct LooseObjectStream {
    obj_type: ObjectType,
    size: usize,
    decoder: ZlibDecoder<fs::File>,
    bytes_read: usize,
}

impl LooseObjectStream {
    /// The object type.
    pub fn object_type(&self) -> ObjectType {
        self.obj_type
    }

    /// The declared content size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// How many content bytes remain to be read.
    pub fn bytes_remaining(&self) -> usize {
        self.size.saturating_sub(self.bytes_read)
    }
}

impl Read for LooseObjectStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.bytes_remaining();
        if remaining == 0 {
            return Ok(0);
        }
        // Don't read past the declared content size.
        let max_read = remaining.min(buf.len());
        let n = self.decoder.read(&mut buf[..max_read])?;
        self.bytes_read += n;
        Ok(n)
    }
}

impl LooseObjectStore {
    /// Open a streaming reader for a loose object.
    ///
    /// Returns `Ok(None)` if the object does not exist.
    /// The header is parsed immediately; content bytes are decompressed
    /// on demand through the [`Read`] trait.
    pub fn stream(
        &self,
        oid: &git_hash::ObjectId,
    ) -> Result<Option<LooseObjectStream>, LooseError> {
        let path = self.object_path(oid);
        let file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LooseError::Io(e)),
        };

        // First pass: decompress just the header to learn type + size + header length.
        let mut decoder = ZlibDecoder::new(file);
        let mut hdr_buf = [0u8; 64];
        let mut filled = 0;

        loop {
            if filled >= hdr_buf.len() {
                return Err(LooseError::Corrupt {
                    oid: oid.to_hex(),
                    reason: "header exceeds 64 bytes".into(),
                });
            }
            let n = decoder.read(&mut hdr_buf[filled..]).map_err(|e| {
                LooseError::Decompress {
                    oid: oid.to_hex(),
                    source: e,
                }
            })?;
            if n == 0 {
                return Err(LooseError::Corrupt {
                    oid: oid.to_hex(),
                    reason: "unexpected EOF before header null terminator".into(),
                });
            }
            filled += n;
            if hdr_buf[..filled].contains(&0) {
                break;
            }
        }

        let (obj_type, content_size, header_len) =
            header::parse_header(&hdr_buf[..filled])?;

        // Re-open and position the decoder right after the header so that
        // subsequent reads yield only content bytes.
        let file2 = fs::File::open(&path)?;
        let mut decoder2 = ZlibDecoder::new(file2);
        let mut skip_buf = vec![0u8; header_len];
        decoder2.read_exact(&mut skip_buf).map_err(|e| {
            LooseError::Decompress {
                oid: oid.to_hex(),
                source: e,
            }
        })?;

        Ok(Some(LooseObjectStream {
            obj_type,
            size: content_size,
            decoder: decoder2,
            bytes_read: 0,
        }))
    }
}

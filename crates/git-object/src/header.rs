use crate::{ObjectError, ObjectType};

/// Parse an object header from raw bytes.
///
/// The header format is `"<type> <size>\0"`. Returns `(type, content_size, header_length)`
/// where `header_length` includes the null terminator.
pub fn parse_header(data: &[u8]) -> Result<(ObjectType, usize, usize), ObjectError> {
    let null_pos = data
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| ObjectError::InvalidHeader("missing null terminator".into()))?;

    let header = &data[..null_pos];
    let space_pos = header
        .iter()
        .position(|&b| b == b' ')
        .ok_or_else(|| ObjectError::InvalidHeader("missing space in header".into()))?;

    let type_bytes = &header[..space_pos];
    let size_bytes = &header[space_pos + 1..];

    let obj_type = ObjectType::from_bytes(type_bytes)?;

    let size_str = std::str::from_utf8(size_bytes)
        .map_err(|_| ObjectError::InvalidHeader("non-ASCII size".into()))?;
    let content_size: usize = size_str
        .parse()
        .map_err(|_| ObjectError::InvalidHeader(format!("invalid size: {size_str}")))?;

    Ok((obj_type, content_size, null_pos + 1))
}

/// Write an object header: `"<type> <size>\0"`.
pub fn write_header(obj_type: ObjectType, content_size: usize) -> Vec<u8> {
    let s = format!("{} {}\0", obj_type, content_size);
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_blob_header() {
        let data = b"blob 12\0hello world!";
        let (ty, size, hdr_len) = parse_header(data).unwrap();
        assert_eq!(ty, ObjectType::Blob);
        assert_eq!(size, 12);
        assert_eq!(hdr_len, 8);
        assert_eq!(&data[hdr_len..], b"hello world!");
    }

    #[test]
    fn parse_commit_header() {
        let data = b"commit 256\0";
        let (ty, size, _) = parse_header(data).unwrap();
        assert_eq!(ty, ObjectType::Commit);
        assert_eq!(size, 256);
    }

    #[test]
    fn write_and_parse_roundtrip() {
        let hdr = write_header(ObjectType::Tree, 42);
        let (ty, size, len) = parse_header(&hdr).unwrap();
        assert_eq!(ty, ObjectType::Tree);
        assert_eq!(size, 42);
        assert_eq!(len, hdr.len());
    }

    #[test]
    fn missing_null() {
        assert!(parse_header(b"blob 12").is_err());
    }

    #[test]
    fn missing_space() {
        assert!(parse_header(b"blob12\0").is_err());
    }

    #[test]
    fn invalid_type() {
        assert!(parse_header(b"invalid 12\0").is_err());
    }

    #[test]
    fn invalid_size() {
        assert!(parse_header(b"blob abc\0").is_err());
    }
}

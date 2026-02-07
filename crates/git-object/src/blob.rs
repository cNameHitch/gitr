use crate::ObjectError;

/// A git blob — raw file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub data: Vec<u8>,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Parse blob content. A blob is simply its raw bytes.
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError> {
        Ok(Self {
            data: content.to_vec(),
        })
    }

    /// Serialize: blob content is just the raw data.
    pub fn serialize_content(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_blob() {
        let blob = Blob::parse(b"").unwrap();
        assert!(blob.data.is_empty());
    }

    #[test]
    fn blob_with_content() {
        let blob = Blob::parse(b"hello world").unwrap();
        assert_eq!(blob.data, b"hello world");
    }

    #[test]
    fn blob_with_null_bytes() {
        let data = b"hello\0world\0";
        let blob = Blob::parse(data).unwrap();
        assert_eq!(blob.data, data);
    }

    #[test]
    fn serialize_roundtrip() {
        let original = Blob::new(b"test content".to_vec());
        let serialized = original.serialize_content();
        let parsed = Blob::parse(serialized).unwrap();
        assert_eq!(original, parsed);
    }
}

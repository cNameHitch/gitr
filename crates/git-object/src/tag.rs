use bstr::{BStr, BString, ByteSlice};
use git_hash::ObjectId;
use git_utils::date::Signature;

use crate::{ObjectError, ObjectType};

/// A git annotated tag object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// OID of the tagged object.
    pub target: ObjectId,
    /// Type of the tagged object.
    pub target_type: ObjectType,
    /// Tag name.
    pub tag_name: BString,
    /// Tagger identity and timestamp (optional for some old tags).
    pub tagger: Option<Signature>,
    /// Tag message.
    pub message: BString,
    /// Optional GPG signature.
    pub gpgsig: Option<BString>,
}

impl Tag {
    /// Parse tag content from raw bytes (no object header).
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError> {
        let mut target: Option<ObjectId> = None;
        let mut target_type: Option<ObjectType> = None;
        let mut tag_name: Option<BString> = None;
        let mut tagger: Option<Signature> = None;
        let mut gpgsig: Option<BString> = None;

        let mut pos = 0;
        let data = content;

        // Parse headers.
        loop {
            if pos >= data.len() {
                break;
            }

            if data[pos] == b'\n' {
                pos += 1;
                break;
            }

            let line_end = data[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|p| p + pos)
                .unwrap_or(data.len());

            let line = &data[pos..line_end];

            if let Some(space_pos) = line.iter().position(|&b| b == b' ') {
                let key = &line[..space_pos];
                let value = &line[space_pos + 1..];

                match key {
                    b"object" => {
                        let hex = std::str::from_utf8(value).map_err(|_| {
                            ObjectError::InvalidHeader("non-UTF8 target OID".into())
                        })?;
                        target = Some(ObjectId::from_hex(hex)?);
                    }
                    b"type" => {
                        target_type = Some(ObjectType::from_bytes(value)?);
                    }
                    b"tag" => {
                        tag_name = Some(BString::from(value));
                    }
                    b"tagger" => {
                        tagger = Some(
                            Signature::parse(BStr::new(value))
                                .map_err(|e| ObjectError::InvalidSignature(e.to_string()))?,
                        );
                    }
                    _ => {
                        // Skip unknown headers.
                    }
                }
            }

            pos = line_end + 1;
        }

        // Remaining bytes are the message. Check for GPG signature.
        let remaining = &data[pos..];
        let message;

        // GPG signatures in tags appear at the end of the message, starting with
        // "-----BEGIN PGP SIGNATURE-----".
        if let Some(sig_start) = remaining.find(b"-----BEGIN PGP SIGNATURE-----") {
            message = BString::from(&remaining[..sig_start]);
            gpgsig = Some(BString::from(&remaining[sig_start..]));
        } else if let Some(sig_start) = remaining.find(b"-----BEGIN SSH SIGNATURE-----") {
            message = BString::from(&remaining[..sig_start]);
            gpgsig = Some(BString::from(&remaining[sig_start..]));
        } else {
            message = BString::from(remaining);
        }

        let target = target.ok_or(ObjectError::MissingTagField { field: "object" })?;
        let target_type =
            target_type.ok_or(ObjectError::MissingTagField { field: "type" })?;
        let tag_name = tag_name.ok_or(ObjectError::MissingTagField { field: "tag" })?;

        Ok(Self {
            target,
            target_type,
            tag_name,
            tagger,
            message,
            gpgsig,
        })
    }

    /// Serialize tag content to bytes (no object header).
    pub fn serialize_content(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.extend_from_slice(b"object ");
        out.extend_from_slice(self.target.to_hex().as_bytes());
        out.push(b'\n');

        out.extend_from_slice(b"type ");
        out.extend_from_slice(self.target_type.as_bytes());
        out.push(b'\n');

        out.extend_from_slice(b"tag ");
        out.extend_from_slice(&self.tag_name);
        out.push(b'\n');

        if let Some(ref tagger) = self.tagger {
            out.extend_from_slice(b"tagger ");
            out.extend_from_slice(&tagger.to_bytes());
            out.push(b'\n');
        }

        out.push(b'\n');
        out.extend_from_slice(&self.message);

        if let Some(ref sig) = self.gpgsig {
            out.extend_from_slice(sig);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tag_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(
            b"object da39a3ee5e6b4b0d3255bfef95601890afd80709\n",
        );
        out.extend_from_slice(b"type commit\n");
        out.extend_from_slice(b"tag v1.0\n");
        out.extend_from_slice(
            b"tagger John Doe <john@example.com> 1234567890 +0000\n",
        );
        out.extend_from_slice(b"\n");
        out.extend_from_slice(b"Release v1.0\n");
        out
    }

    #[test]
    fn parse_tag() {
        let tag = Tag::parse(&sample_tag_bytes()).unwrap();
        assert_eq!(
            tag.target.to_hex(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(tag.target_type, ObjectType::Commit);
        assert_eq!(tag.tag_name.as_bytes(), b"v1.0");
        assert!(tag.tagger.is_some());
        assert_eq!(tag.message.as_bytes(), b"Release v1.0\n");
        assert!(tag.gpgsig.is_none());
    }

    #[test]
    fn parse_tag_without_tagger() {
        let mut data = Vec::new();
        data.extend_from_slice(
            b"object da39a3ee5e6b4b0d3255bfef95601890afd80709\n",
        );
        data.extend_from_slice(b"type commit\n");
        data.extend_from_slice(b"tag old-tag\n");
        data.extend_from_slice(b"\nOld tag\n");

        let tag = Tag::parse(&data).unwrap();
        assert!(tag.tagger.is_none());
        assert_eq!(tag.tag_name.as_bytes(), b"old-tag");
    }

    #[test]
    fn serialize_roundtrip() {
        let original_bytes = sample_tag_bytes();
        let tag = Tag::parse(&original_bytes).unwrap();
        let serialized = tag.serialize_content();
        assert_eq!(serialized, original_bytes);
    }

    #[test]
    fn tag_pointing_to_tree() {
        let mut data = Vec::new();
        data.extend_from_slice(
            b"object da39a3ee5e6b4b0d3255bfef95601890afd80709\n",
        );
        data.extend_from_slice(b"type tree\n");
        data.extend_from_slice(b"tag tree-tag\n");
        data.extend_from_slice(b"\n");

        let tag = Tag::parse(&data).unwrap();
        assert_eq!(tag.target_type, ObjectType::Tree);
    }

    #[test]
    fn missing_object_errors() {
        let data = b"type commit\ntag v1.0\n\nmessage\n";
        assert!(Tag::parse(data).is_err());
    }
}

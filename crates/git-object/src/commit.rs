use bstr::{BStr, BString, ByteSlice};
use git_hash::ObjectId;
use git_utils::date::Signature;

use crate::ObjectError;

/// A git commit object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// OID of the root tree.
    pub tree: ObjectId,
    /// Parent commit OIDs (empty for root commit).
    pub parents: Vec<ObjectId>,
    /// Author identity and timestamp.
    pub author: Signature,
    /// Committer identity and timestamp.
    pub committer: Signature,
    /// Optional encoding header (e.g., "UTF-8", "ISO-8859-1").
    pub encoding: Option<BString>,
    /// Optional GPG signature (multi-line).
    pub gpgsig: Option<BString>,
    /// Extra headers (mergetag, etc.) preserved for round-trip.
    pub extra_headers: Vec<(BString, BString)>,
    /// Commit message (everything after the blank line separator).
    pub message: BString,
}

impl Commit {
    /// Parse commit content from raw bytes (no object header).
    pub fn parse(content: &[u8]) -> Result<Self, ObjectError> {
        let mut tree: Option<ObjectId> = None;
        let mut parents = Vec::new();
        let mut author: Option<Signature> = None;
        let mut committer: Option<Signature> = None;
        let mut encoding: Option<BString> = None;
        let mut gpgsig: Option<BString> = None;
        let mut extra_headers: Vec<(BString, BString)> = Vec::new();

        let mut pos = 0;
        let data = content;

        // Parse headers (lines before the blank line).
        loop {
            if pos >= data.len() {
                // No message (unusual but possible).
                break;
            }

            // A blank line separates headers from message.
            if data[pos] == b'\n' {
                pos += 1;
                break;
            }

            // Find end of this logical line (handle continuation lines for gpgsig).
            let line_end = data[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|p| p + pos)
                .unwrap_or(data.len());

            let line = &data[pos..line_end];

            // Parse "key value" format.
            if let Some(space_pos) = line.iter().position(|&b| b == b' ') {
                let key = &line[..space_pos];
                let value = &line[space_pos + 1..];

                match key {
                    b"tree" => {
                        let hex = std::str::from_utf8(value)
                            .map_err(|_| ObjectError::InvalidHeader("non-UTF8 tree OID".into()))?;
                        tree = Some(ObjectId::from_hex(hex)?);
                    }
                    b"parent" => {
                        let hex = std::str::from_utf8(value)
                            .map_err(|_| ObjectError::InvalidHeader("non-UTF8 parent OID".into()))?;
                        parents.push(ObjectId::from_hex(hex)?);
                    }
                    b"author" => {
                        author = Some(parse_signature(value)?);
                    }
                    b"committer" => {
                        committer = Some(parse_signature(value)?);
                    }
                    b"encoding" => {
                        encoding = Some(BString::from(value));
                    }
                    b"gpgsig" | b"gpgsig-sha256" => {
                        // GPG signatures are multi-line: continuation lines start with a space.
                        let mut sig = Vec::from(value);
                        let mut next = line_end + 1;
                        while next < data.len() && data[next] == b' ' {
                            sig.push(b'\n');
                            let cont_end = data[next..]
                                .iter()
                                .position(|&b| b == b'\n')
                                .map(|p| p + next)
                                .unwrap_or(data.len());
                            sig.extend_from_slice(&data[next + 1..cont_end]);
                            next = cont_end + 1;
                        }
                        gpgsig = Some(BString::from(sig));
                        pos = next;
                        continue;
                    }
                    _ => {
                        // Multi-line extra headers (e.g., mergetag).
                        let mut val = Vec::from(value);
                        let mut next = line_end + 1;
                        while next < data.len() && data[next] == b' ' {
                            val.push(b'\n');
                            let cont_end = data[next..]
                                .iter()
                                .position(|&b| b == b'\n')
                                .map(|p| p + next)
                                .unwrap_or(data.len());
                            val.extend_from_slice(&data[next + 1..cont_end]);
                            next = cont_end + 1;
                        }
                        extra_headers
                            .push((BString::from(key), BString::from(val)));
                        pos = next;
                        continue;
                    }
                }
            }

            pos = line_end + 1;
        }

        let tree = tree.ok_or(ObjectError::MissingCommitField { field: "tree" })?;
        let author = author.ok_or(ObjectError::MissingCommitField { field: "author" })?;
        let committer =
            committer.ok_or(ObjectError::MissingCommitField { field: "committer" })?;

        let message = BString::from(&data[pos..]);

        Ok(Self {
            tree,
            parents,
            author,
            committer,
            encoding,
            gpgsig,
            extra_headers,
            message,
        })
    }

    /// Serialize commit content to bytes (no object header).
    pub fn serialize_content(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // tree
        out.extend_from_slice(b"tree ");
        out.extend_from_slice(self.tree.to_hex().as_bytes());
        out.push(b'\n');

        // parents
        for parent in &self.parents {
            out.extend_from_slice(b"parent ");
            out.extend_from_slice(parent.to_hex().as_bytes());
            out.push(b'\n');
        }

        // author
        out.extend_from_slice(b"author ");
        out.extend_from_slice(&self.author.to_bytes());
        out.push(b'\n');

        // committer
        out.extend_from_slice(b"committer ");
        out.extend_from_slice(&self.committer.to_bytes());
        out.push(b'\n');

        // encoding (optional)
        if let Some(ref enc) = self.encoding {
            out.extend_from_slice(b"encoding ");
            out.extend_from_slice(enc);
            out.push(b'\n');
        }

        // gpgsig (optional, multi-line with continuation)
        if let Some(ref sig) = self.gpgsig {
            out.extend_from_slice(b"gpgsig ");
            for (i, line) in sig.split(|&b| b == b'\n').enumerate() {
                if i > 0 {
                    out.push(b'\n');
                    out.push(b' ');
                }
                out.extend_from_slice(line);
            }
            out.push(b'\n');
        }

        // extra headers (multi-line with continuation)
        for (key, val) in &self.extra_headers {
            out.extend_from_slice(key);
            out.push(b' ');
            for (i, line) in val.split(|&b| b == b'\n').enumerate() {
                if i > 0 {
                    out.push(b'\n');
                    out.push(b' ');
                }
                out.extend_from_slice(line);
            }
            out.push(b'\n');
        }

        // blank line + message
        out.push(b'\n');
        out.extend_from_slice(&self.message);

        out
    }

    /// Get the first parent (or None for root commits).
    pub fn first_parent(&self) -> Option<&ObjectId> {
        self.parents.first()
    }

    /// Is this a merge commit? (more than one parent)
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// Is this a root commit? (no parents)
    pub fn is_root(&self) -> bool {
        self.parents.is_empty()
    }

    /// Get just the summary (first line) of the message.
    pub fn summary(&self) -> &BStr {
        let msg: &[u8] = self.message.as_ref();
        match msg.find_byte(b'\n') {
            Some(pos) => BStr::new(&msg[..pos]),
            None => BStr::new(msg),
        }
    }

    /// Get the message body (everything after the first blank line in the message).
    pub fn body(&self) -> Option<&BStr> {
        let msg: &[u8] = self.message.as_ref();
        // Body starts after the first blank line (\n\n) in the message.
        msg.find(b"\n\n").map(|pos| BStr::new(&msg[pos + 2..]))
    }
}

fn parse_signature(data: &[u8]) -> Result<Signature, ObjectError> {
    Signature::parse(BStr::new(data))
        .map_err(|e| ObjectError::InvalidSignature(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::ByteSlice;

    fn sample_commit_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"tree da39a3ee5e6b4b0d3255bfef95601890afd80709\n");
        out.extend_from_slice(
            b"parent 0000000000000000000000000000000000000001\n",
        );
        out.extend_from_slice(
            b"author John Doe <john@example.com> 1234567890 +0000\n",
        );
        out.extend_from_slice(
            b"committer Jane Doe <jane@example.com> 1234567890 +0000\n",
        );
        out.extend_from_slice(b"\n");
        out.extend_from_slice(b"Initial commit\n");
        out
    }

    #[test]
    fn parse_commit() {
        let commit = Commit::parse(&sample_commit_bytes()).unwrap();
        assert_eq!(
            commit.tree.to_hex(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        assert_eq!(commit.parents.len(), 1);
        assert_eq!(commit.author.name.as_bytes(), b"John Doe");
        assert_eq!(commit.committer.email.as_bytes(), b"jane@example.com");
        assert_eq!(commit.message.as_bytes(), b"Initial commit\n");
    }

    #[test]
    fn parse_root_commit() {
        let mut data = Vec::new();
        data.extend_from_slice(b"tree da39a3ee5e6b4b0d3255bfef95601890afd80709\n");
        data.extend_from_slice(
            b"author A <a@b.com> 1000000000 +0000\n",
        );
        data.extend_from_slice(
            b"committer A <a@b.com> 1000000000 +0000\n",
        );
        data.extend_from_slice(b"\nroot\n");

        let commit = Commit::parse(&data).unwrap();
        assert!(commit.is_root());
        assert!(!commit.is_merge());
    }

    #[test]
    fn parse_merge_commit() {
        let mut data = Vec::new();
        data.extend_from_slice(b"tree da39a3ee5e6b4b0d3255bfef95601890afd80709\n");
        data.extend_from_slice(
            b"parent 0000000000000000000000000000000000000001\n",
        );
        data.extend_from_slice(
            b"parent 0000000000000000000000000000000000000002\n",
        );
        data.extend_from_slice(
            b"author A <a@b.com> 1000000000 +0000\n",
        );
        data.extend_from_slice(
            b"committer A <a@b.com> 1000000000 +0000\n",
        );
        data.extend_from_slice(b"\nMerge\n");

        let commit = Commit::parse(&data).unwrap();
        assert!(commit.is_merge());
        assert_eq!(commit.parents.len(), 2);
    }

    #[test]
    fn serialize_roundtrip() {
        let original_bytes = sample_commit_bytes();
        let commit = Commit::parse(&original_bytes).unwrap();
        let serialized = commit.serialize_content();
        assert_eq!(serialized, original_bytes);
    }

    #[test]
    fn summary_and_body() {
        let commit = Commit::parse(&sample_commit_bytes()).unwrap();
        assert_eq!(commit.summary().as_bytes(), b"Initial commit");
        assert_eq!(commit.body(), None); // no blank line in message
    }

    #[test]
    fn summary_and_body_multi_paragraph() {
        let mut data = Vec::new();
        data.extend_from_slice(b"tree da39a3ee5e6b4b0d3255bfef95601890afd80709\n");
        data.extend_from_slice(b"author A <a@b.com> 1000000000 +0000\n");
        data.extend_from_slice(b"committer A <a@b.com> 1000000000 +0000\n");
        data.extend_from_slice(b"\nSummary line\n\nBody paragraph.\n");

        let commit = Commit::parse(&data).unwrap();
        assert_eq!(commit.summary().as_bytes(), b"Summary line");
        assert_eq!(
            commit.body().unwrap().as_bytes(),
            b"Body paragraph.\n"
        );
    }

    #[test]
    fn commit_with_encoding() {
        let mut data = Vec::new();
        data.extend_from_slice(b"tree da39a3ee5e6b4b0d3255bfef95601890afd80709\n");
        data.extend_from_slice(b"author A <a@b.com> 1000000000 +0000\n");
        data.extend_from_slice(b"committer A <a@b.com> 1000000000 +0000\n");
        data.extend_from_slice(b"encoding ISO-8859-1\n");
        data.extend_from_slice(b"\nmessage\n");

        let commit = Commit::parse(&data).unwrap();
        assert_eq!(commit.encoding.as_ref().unwrap().as_bytes(), b"ISO-8859-1");

        let serialized = commit.serialize_content();
        assert_eq!(serialized, data);
    }

    #[test]
    fn missing_tree_errors() {
        let data = b"author A <a@b.com> 1000000000 +0000\ncommitter A <a@b.com> 1000000000 +0000\n\nmsg\n";
        assert!(Commit::parse(data).is_err());
    }
}

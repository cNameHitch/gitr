//! Bundle file format support.
//!
//! Git bundles are files that contain a pack plus a list of references.
//! They enable offline transfer of objects.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

use git_hash::ObjectId;

use crate::ProtocolError;

/// Bundle file header signature.
const BUNDLE_V2_SIGNATURE: &str = "# v2 git bundle";
const BUNDLE_V3_SIGNATURE: &str = "# v3 git bundle";

/// Parsed bundle file.
#[derive(Debug)]
pub struct Bundle {
    /// Bundle version (2 or 3).
    pub version: u32,
    /// Prerequisites (OIDs the receiver must already have).
    pub prerequisites: Vec<(ObjectId, Option<String>)>,
    /// References included in the bundle.
    pub refs: Vec<(ObjectId, String)>,
    /// Pack data (everything after the header).
    pub pack_data: Vec<u8>,
}

/// Read a bundle file.
pub fn read_bundle(path: &Path) -> Result<Bundle, ProtocolError> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    parse_bundle(&mut reader)
}

/// Parse a bundle from a reader.
pub fn parse_bundle<R: Read>(reader: &mut BufReader<R>) -> Result<Bundle, ProtocolError> {
    let mut line = String::new();

    // Read signature line
    reader.read_line(&mut line)?;
    let line = line.trim_end();

    let version = if line == BUNDLE_V2_SIGNATURE {
        2
    } else if line == BUNDLE_V3_SIGNATURE {
        3
    } else {
        return Err(ProtocolError::Protocol(format!(
            "invalid bundle signature: {}",
            line
        )));
    };

    let mut prerequisites = Vec::new();
    let mut refs = Vec::new();

    // Read prerequisite and ref lines until blank line
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            break;
        }

        if let Some(rest) = line.strip_prefix('-') {
            // Prerequisite: -<oid> [<comment>]
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            let oid = ObjectId::from_hex(parts[0]).map_err(|e| {
                ProtocolError::Protocol(format!("invalid OID in bundle prerequisite: {}", e))
            })?;
            let comment = parts.get(1).map(|s| s.to_string());
            prerequisites.push((oid, comment));
        } else {
            // Reference: <oid> <refname>
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() != 2 {
                return Err(ProtocolError::Protocol(format!(
                    "invalid bundle ref line: {}",
                    line
                )));
            }
            let oid = ObjectId::from_hex(parts[0]).map_err(|e| {
                ProtocolError::Protocol(format!("invalid OID in bundle ref: {}", e))
            })?;
            refs.push((oid, parts[1].to_string()));
        }
    }

    // Rest is pack data
    let mut pack_data = Vec::new();
    reader.read_to_end(&mut pack_data)?;

    Ok(Bundle {
        version,
        prerequisites,
        refs,
        pack_data,
    })
}

/// Write a bundle file.
pub fn write_bundle<W: Write>(
    writer: &mut W,
    refs: &[(ObjectId, &str)],
    prerequisites: &[(ObjectId, Option<&str>)],
    pack_data: &[u8],
) -> Result<(), ProtocolError> {
    // Write v2 header
    writeln!(writer, "{}", BUNDLE_V2_SIGNATURE)?;

    // Write prerequisites
    for (oid, comment) in prerequisites {
        if let Some(c) = comment {
            writeln!(writer, "-{} {}", oid, c)?;
        } else {
            writeln!(writer, "-{}", oid)?;
        }
    }

    // Write refs
    for (oid, refname) in refs {
        writeln!(writer, "{} {}", oid, refname)?;
    }

    // Blank line separates header from pack data
    writeln!(writer)?;

    // Write pack data
    writer.write_all(pack_data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_bundle() {
        let oid = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();
        let prereq_oid =
            ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();

        let refs = vec![(oid, "refs/heads/main")];
        let prereqs = vec![(prereq_oid, Some("needed commit"))];
        let pack_data = b"PACK\x00\x00\x00\x02\x00\x00\x00\x00";

        let mut buf = Vec::new();
        write_bundle(&mut buf, &refs, &prereqs, pack_data).unwrap();

        let mut reader = BufReader::new(Cursor::new(buf));
        let bundle = parse_bundle(&mut reader).unwrap();

        assert_eq!(bundle.version, 2);
        assert_eq!(bundle.refs.len(), 1);
        assert_eq!(bundle.refs[0].0, oid);
        assert_eq!(bundle.refs[0].1, "refs/heads/main");
        assert_eq!(bundle.prerequisites.len(), 1);
        assert_eq!(bundle.prerequisites[0].0, prereq_oid);
        assert_eq!(bundle.pack_data, pack_data);
    }

    #[test]
    fn parse_v2_bundle_no_prereqs() {
        let oid = ObjectId::from_hex("95d09f2b10159347eece71399a7e2e907ea3df4f").unwrap();

        let mut buf = Vec::new();
        write_bundle(&mut buf, &[(oid, "refs/heads/main")], &[], b"PACK").unwrap();

        let mut reader = BufReader::new(Cursor::new(buf));
        let bundle = parse_bundle(&mut reader).unwrap();

        assert_eq!(bundle.version, 2);
        assert!(bundle.prerequisites.is_empty());
        assert_eq!(bundle.refs.len(), 1);
    }
}

//! OID prefix resolution (short hex -> full OID with ambiguity detection).
//!
//! Searches all backends for objects matching the given hex prefix.
//! Returns an error if the prefix is ambiguous (matches multiple objects).

use git_hash::ObjectId;

use crate::backend::hex_prefix_to_bytes;
use crate::{ObjectDatabase, OdbError};

/// Minimum prefix length (matches C git's MINIMUM_ABBREV).
const MINIMUM_ABBREV: usize = 4;

/// Resolve a hex prefix to a full OID.
///
/// Searches loose objects, pack files, and alternates. Returns an error
/// if the prefix is ambiguous (matches multiple distinct objects) or
/// if no object matches.
pub fn resolve_prefix(odb: &ObjectDatabase, prefix: &str) -> Result<ObjectId, OdbError> {
    if prefix.len() < MINIMUM_ABBREV {
        return Err(OdbError::Ambiguous {
            prefix: prefix.to_string(),
            count: 0,
        });
    }

    // Validate hex characters
    if !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(OdbError::NotFound(ObjectId::NULL_SHA1));
    }

    // If it's a full hex OID, just check existence
    let algo = odb.hash_algo();
    if prefix.len() == algo.hex_len() {
        if let Ok(oid) = ObjectId::from_hex(prefix) {
            if odb.contains(&oid) {
                return Ok(oid);
            }
            return Err(OdbError::NotFound(oid));
        }
    }

    let mut all_matches: Vec<ObjectId> = Vec::new();

    // 1. Search loose objects by iterating and prefix-matching
    if let Ok(iter) = odb.loose.iter() {
        for result in iter {
            let oid = result?;
            if oid.starts_with_hex(prefix) {
                all_matches.push(oid);
            }
        }
    }

    // 2. Search pack files via index prefix lookup
    {
        let prefix_bytes = hex_prefix_to_bytes(prefix);
        let packs = odb.packs.read().unwrap();
        for pack in packs.iter() {
            let results = pack.index().lookup_prefix(&prefix_bytes);
            for (oid, _offset) in results {
                // Double-check that the full hex matches (handles odd-length prefix edge case)
                if oid.starts_with_hex(prefix) {
                    all_matches.push(oid);
                }
            }
        }
    }

    // 3. Search alternates
    for alt in &odb.alternates {
        match resolve_prefix(alt, prefix) {
            Ok(oid) => all_matches.push(oid),
            Err(OdbError::NotFound(_)) => {}
            Err(OdbError::Ambiguous { .. }) => {
                return Err(OdbError::Ambiguous {
                    prefix: prefix.to_string(),
                    count: 2,
                });
            }
            Err(e) => return Err(e),
        }
    }

    // Deduplicate (same object may appear in multiple sources)
    all_matches.sort();
    all_matches.dedup();

    match all_matches.len() {
        0 => Err(OdbError::NotFound(ObjectId::NULL_SHA1)),
        1 => Ok(all_matches[0]),
        n => Err(OdbError::Ambiguous {
            prefix: prefix.to_string(),
            count: n,
        }),
    }
}

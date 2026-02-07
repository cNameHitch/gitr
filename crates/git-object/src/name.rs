//! Object name resolution (rev-parse logic).
//!
//! This module provides the trait and types for resolving revision expressions
//! like `HEAD~3`, `abc1234`, `v1.0^{commit}` to full ObjectIds.
//!
//! The actual lookup implementation requires an object database and ref store
//! (specs 006 and 008), so this module defines the interface and parseable
//! revision syntax. Full resolution is deferred until those specs are available.

use git_hash::ObjectId;

use crate::{ObjectError, ObjectType};

/// A parsed revision suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionSuffix {
    /// `^` or `^N` — Nth parent (default N=1).
    Parent(u32),
    /// `~N` — Nth first-parent ancestor (default N=1).
    Ancestor(u32),
    /// `^{type}` — peel to the given type.
    Peel(ObjectType),
    /// `^{}` — peel to the first non-tag object.
    PeelAny,
    /// `^{/regex}` — search commit messages.
    SearchMessage(String),
}

/// A parsed revision expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionExpr {
    /// The base reference (hex OID, ref name, HEAD, etc.).
    pub base: String,
    /// Chain of suffixes to apply.
    pub suffixes: Vec<RevisionSuffix>,
}

/// Parse a revision expression string into its components.
///
/// Examples:
/// - `"HEAD"` → base="HEAD", no suffixes
/// - `"HEAD~3"` → base="HEAD", suffixes=[Ancestor(3)]
/// - `"abc1234^{commit}"` → base="abc1234", suffixes=[Peel(Commit)]
/// - `"main^^"` → base="main", suffixes=[Parent(1), Parent(1)]
pub fn parse_revision(input: &str) -> Result<RevisionExpr, ObjectError> {
    let mut suffixes = Vec::new();
    let bytes = input.as_bytes();

    // Find where suffixes start: first unescaped '^' or '~'.
    let base_end = find_suffix_start(bytes);
    let base = input[..base_end].to_string();
    let mut pos = base_end;

    while pos < bytes.len() {
        match bytes[pos] {
            b'^' => {
                pos += 1;
                if pos < bytes.len() && bytes[pos] == b'{' {
                    // ^{...} syntax
                    pos += 1;
                    let close = bytes[pos..]
                        .iter()
                        .position(|&b| b == b'}')
                        .ok_or_else(|| {
                            ObjectError::InvalidHeader("unclosed ^{".into())
                        })?
                        + pos;
                    let inner = &input[pos..close];
                    let suffix = if inner.is_empty() {
                        RevisionSuffix::PeelAny
                    } else if let Some(pattern) = inner.strip_prefix('/') {
                        RevisionSuffix::SearchMessage(pattern.to_string())
                    } else {
                        let obj_type: ObjectType = inner.parse().map_err(|_| {
                            ObjectError::InvalidHeader(format!("invalid peel type: {inner}"))
                        })?;
                        RevisionSuffix::Peel(obj_type)
                    };
                    suffixes.push(suffix);
                    pos = close + 1;
                } else {
                    // ^N or just ^
                    let (n, consumed) = parse_number(&bytes[pos..]);
                    suffixes.push(RevisionSuffix::Parent(n.unwrap_or(1)));
                    pos += consumed;
                }
            }
            b'~' => {
                pos += 1;
                let (n, consumed) = parse_number(&bytes[pos..]);
                suffixes.push(RevisionSuffix::Ancestor(n.unwrap_or(1)));
                pos += consumed;
            }
            _ => {
                return Err(ObjectError::InvalidHeader(format!(
                    "unexpected character '{}' in revision",
                    bytes[pos] as char
                )));
            }
        }
    }

    Ok(RevisionExpr { base, suffixes })
}

/// Resolve a full hex string to an ObjectId.
pub fn resolve_hex(hex: &str) -> Result<ObjectId, ObjectError> {
    ObjectId::from_hex(hex).map_err(ObjectError::from)
}

/// Check if a string looks like a hex OID prefix (at least 4 hex chars).
pub fn is_hex_prefix(s: &str) -> bool {
    s.len() >= 4 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn find_suffix_start(bytes: &[u8]) -> usize {
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'^' || b == b'~' {
            return i;
        }
    }
    bytes.len()
}

fn parse_number(bytes: &[u8]) -> (Option<u32>, usize) {
    let mut n: u32 = 0;
    let mut i = 0;
    let mut has_digits = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        n = n * 10 + u32::from(bytes[i] - b'0');
        i += 1;
        has_digits = true;
    }
    if has_digits {
        (Some(n), i)
    } else {
        (None, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_ref() {
        let expr = parse_revision("HEAD").unwrap();
        assert_eq!(expr.base, "HEAD");
        assert!(expr.suffixes.is_empty());
    }

    #[test]
    fn parse_parent() {
        let expr = parse_revision("HEAD^").unwrap();
        assert_eq!(expr.base, "HEAD");
        assert_eq!(expr.suffixes, vec![RevisionSuffix::Parent(1)]);
    }

    #[test]
    fn parse_parent_n() {
        let expr = parse_revision("HEAD^2").unwrap();
        assert_eq!(expr.suffixes, vec![RevisionSuffix::Parent(2)]);
    }

    #[test]
    fn parse_ancestor() {
        let expr = parse_revision("HEAD~3").unwrap();
        assert_eq!(expr.suffixes, vec![RevisionSuffix::Ancestor(3)]);
    }

    #[test]
    fn parse_peel_commit() {
        let expr = parse_revision("v1.0^{commit}").unwrap();
        assert_eq!(expr.base, "v1.0");
        assert_eq!(expr.suffixes, vec![RevisionSuffix::Peel(ObjectType::Commit)]);
    }

    #[test]
    fn parse_peel_any() {
        let expr = parse_revision("v1.0^{}").unwrap();
        assert_eq!(expr.suffixes, vec![RevisionSuffix::PeelAny]);
    }

    #[test]
    fn parse_search_message() {
        let expr = parse_revision("HEAD^{/fix bug}").unwrap();
        assert_eq!(
            expr.suffixes,
            vec![RevisionSuffix::SearchMessage("fix bug".into())]
        );
    }

    #[test]
    fn parse_chained_suffixes() {
        let expr = parse_revision("main^^~3").unwrap();
        assert_eq!(expr.base, "main");
        assert_eq!(
            expr.suffixes,
            vec![
                RevisionSuffix::Parent(1),
                RevisionSuffix::Parent(1),
                RevisionSuffix::Ancestor(3),
            ]
        );
    }

    #[test]
    fn resolve_full_hex() {
        let oid = resolve_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        assert_eq!(
            oid.to_hex(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }

    #[test]
    fn is_hex_prefix_checks() {
        assert!(is_hex_prefix("abcd1234"));
        assert!(is_hex_prefix("ABCD"));
        assert!(!is_hex_prefix("abc")); // too short
        assert!(!is_hex_prefix("HEAD")); // not hex
    }
}

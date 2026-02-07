use std::fmt;
use std::str::FromStr;

use crate::hex::{hex_decode, hex_to_string};
use crate::{HashAlgorithm, HashError};

/// A git object identifier — the hash of an object's content.
///
/// This is an enum with variants for each supported hash algorithm,
/// carrying the raw digest bytes inline.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectId {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

impl ObjectId {
    /// The SHA-1 null OID (all zeros).
    pub const NULL_SHA1: Self = Self::Sha1([0u8; 20]);
    /// The SHA-256 null OID (all zeros).
    pub const NULL_SHA256: Self = Self::Sha256([0u8; 32]);

    /// Create an ObjectId from raw bytes and an algorithm.
    pub fn from_bytes(bytes: &[u8], algo: HashAlgorithm) -> Result<Self, HashError> {
        let expected = algo.digest_len();
        if bytes.len() != expected {
            return Err(HashError::InvalidHashLength {
                expected,
                actual: bytes.len(),
            });
        }
        match algo {
            HashAlgorithm::Sha1 => {
                let mut arr = [0u8; 20];
                arr.copy_from_slice(bytes);
                Ok(Self::Sha1(arr))
            }
            HashAlgorithm::Sha256 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(bytes);
                Ok(Self::Sha256(arr))
            }
        }
    }

    /// Create an ObjectId from a hex string.
    ///
    /// The algorithm is inferred from the length:
    /// - 40 hex chars → SHA-1
    /// - 64 hex chars → SHA-256
    pub fn from_hex(hex: &str) -> Result<Self, HashError> {
        let algo = HashAlgorithm::from_hex_len(hex.len()).ok_or(HashError::InvalidHexLength {
            expected: 40, // suggest SHA-1 as default
            actual: hex.len(),
        })?;
        match algo {
            HashAlgorithm::Sha1 => {
                let mut bytes = [0u8; 20];
                hex_decode(hex, &mut bytes)?;
                Ok(Self::Sha1(bytes))
            }
            HashAlgorithm::Sha256 => {
                let mut bytes = [0u8; 32];
                hex_decode(hex, &mut bytes)?;
                Ok(Self::Sha256(bytes))
            }
        }
    }

    /// Get the raw bytes of the hash.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Sha1(b) => b,
            Self::Sha256(b) => b,
        }
    }

    /// Get the hash algorithm used.
    pub fn algorithm(&self) -> HashAlgorithm {
        match self {
            Self::Sha1(_) => HashAlgorithm::Sha1,
            Self::Sha256(_) => HashAlgorithm::Sha256,
        }
    }

    /// Check if this is the null (all-zeros) OID.
    pub fn is_null(&self) -> bool {
        self.as_bytes().iter().all(|&b| b == 0)
    }

    /// Get the hex string representation (lowercase).
    pub fn to_hex(&self) -> String {
        hex_to_string(self.as_bytes())
    }

    /// Get the first byte of the hash (for fan-out table indexing).
    pub fn first_byte(&self) -> u8 {
        self.as_bytes()[0]
    }

    /// Check if this OID's hex representation starts with the given hex prefix.
    pub fn starts_with_hex(&self, prefix: &str) -> bool {
        let hex = self.to_hex();
        hex.starts_with(&prefix.to_ascii_lowercase())
    }

    /// Get the loose object path component: `"xx/xxxx..."`.
    pub fn loose_path(&self) -> String {
        let hex = self.to_hex();
        format!("{}/{}", &hex[..2], &hex[2..])
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({})", &self.to_hex()[..8])
    }
}

impl FromStr for ObjectId {
    type Err = HashError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const SHA1_HEX: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    const SHA256_HEX: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn from_hex_sha1() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        assert_eq!(oid.algorithm(), HashAlgorithm::Sha1);
        assert_eq!(oid.as_bytes().len(), 20);
    }

    #[test]
    fn from_hex_sha256() {
        let oid = ObjectId::from_hex(SHA256_HEX).unwrap();
        assert_eq!(oid.algorithm(), HashAlgorithm::Sha256);
        assert_eq!(oid.as_bytes().len(), 32);
    }

    #[test]
    fn display_roundtrip() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        let displayed = oid.to_string();
        assert_eq!(displayed, SHA1_HEX);
        let parsed: ObjectId = displayed.parse().unwrap();
        assert_eq!(parsed, oid);
    }

    #[test]
    fn display_roundtrip_sha256() {
        let oid = ObjectId::from_hex(SHA256_HEX).unwrap();
        let displayed = oid.to_string();
        assert_eq!(displayed, SHA256_HEX);
        let parsed: ObjectId = displayed.parse().unwrap();
        assert_eq!(parsed, oid);
    }

    #[test]
    fn debug_shows_short_hash() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        let debug = format!("{:?}", oid);
        assert_eq!(debug, "ObjectId(da39a3ee)");
    }

    #[test]
    fn equality() {
        let a = ObjectId::from_hex(SHA1_HEX).unwrap();
        let b = ObjectId::from_hex(SHA1_HEX).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn ordering() {
        let a = ObjectId::from_hex("0000000000000000000000000000000000000001").unwrap();
        let b = ObjectId::from_hex("0000000000000000000000000000000000000002").unwrap();
        assert!(a < b);
    }

    #[test]
    fn hashmap_key() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        let mut map = HashMap::new();
        map.insert(oid, "value");
        assert_eq!(map.get(&oid), Some(&"value"));
    }

    #[test]
    fn null_oid() {
        assert!(ObjectId::NULL_SHA1.is_null());
        assert!(ObjectId::NULL_SHA256.is_null());
        let non_null = ObjectId::from_hex(SHA1_HEX).unwrap();
        assert!(!non_null.is_null());
    }

    #[test]
    fn from_bytes() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        let bytes = oid.as_bytes();
        let reconstructed = ObjectId::from_bytes(bytes, HashAlgorithm::Sha1).unwrap();
        assert_eq!(oid, reconstructed);
    }

    #[test]
    fn from_bytes_wrong_length() {
        let err = ObjectId::from_bytes(&[0; 10], HashAlgorithm::Sha1).unwrap_err();
        assert!(matches!(err, HashError::InvalidHashLength { expected: 20, actual: 10 }));
    }

    #[test]
    fn invalid_hex_chars() {
        let err = ObjectId::from_hex("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").unwrap_err();
        assert!(matches!(err, HashError::InvalidHex { .. }));
    }

    #[test]
    fn invalid_hex_length() {
        let err = ObjectId::from_hex("abcd").unwrap_err();
        assert!(matches!(err, HashError::InvalidHexLength { .. }));
    }

    #[test]
    fn case_insensitive_hex_decode() {
        let lower = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        let upper = ObjectId::from_hex("DA39A3EE5E6B4B0D3255BFEF95601890AFD80709").unwrap();
        assert_eq!(lower, upper);
    }

    #[test]
    fn first_byte() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        assert_eq!(oid.first_byte(), 0xda);
    }

    #[test]
    fn starts_with_hex_prefix() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        assert!(oid.starts_with_hex("da39"));
        assert!(oid.starts_with_hex("DA39")); // case-insensitive
        assert!(!oid.starts_with_hex("abcd"));
    }

    #[test]
    fn loose_path() {
        let oid = ObjectId::from_hex(SHA1_HEX).unwrap();
        let path = oid.loose_path();
        assert_eq!(path, format!("da/{}", &SHA1_HEX[2..]));
    }

    #[test]
    fn max_oid() {
        let max_sha1 = ObjectId::from_hex("ffffffffffffffffffffffffffffffffffffffff").unwrap();
        assert!(!max_sha1.is_null());
        assert_eq!(max_sha1.first_byte(), 0xff);
    }
}

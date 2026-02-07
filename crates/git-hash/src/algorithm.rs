use crate::ObjectId;

/// Supported git hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HashAlgorithm {
    /// SHA-1 (default, 20 bytes / 160 bits).
    #[default]
    Sha1,
    /// SHA-256 (experimental, 32 bytes / 256 bits).
    Sha256,
}

impl HashAlgorithm {
    /// Length of the hash digest in bytes.
    pub const fn digest_len(&self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    /// Length of the hex representation.
    pub const fn hex_len(&self) -> usize {
        self.digest_len() * 2
    }

    /// The null (all-zeros) OID for this algorithm.
    pub const fn null_oid(&self) -> ObjectId {
        match self {
            Self::Sha1 => ObjectId::NULL_SHA1,
            Self::Sha256 => ObjectId::NULL_SHA256,
        }
    }

    /// The 4-byte format identifier used in pack index files (matching C git).
    pub const fn format_id(&self) -> u32 {
        match self {
            // "sha1" in ASCII
            Self::Sha1 => 0x7368_6131,
            // "s256" in ASCII
            Self::Sha256 => 0x7332_3536,
        }
    }

    /// Look up a hash algorithm by name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sha1" | "sha-1" => Some(Self::Sha1),
            "sha256" | "sha-256" => Some(Self::Sha256),
            _ => None,
        }
    }

    /// Look up a hash algorithm by format id.
    pub fn from_format_id(id: u32) -> Option<Self> {
        match id {
            0x7368_6131 => Some(Self::Sha1),
            0x7332_3536 => Some(Self::Sha256),
            _ => None,
        }
    }

    /// Look up a hash algorithm by raw digest length.
    pub fn from_digest_len(len: usize) -> Option<Self> {
        match len {
            20 => Some(Self::Sha1),
            32 => Some(Self::Sha256),
            _ => None,
        }
    }

    /// Look up a hash algorithm by hex length.
    pub fn from_hex_len(len: usize) -> Option<Self> {
        match len {
            40 => Some(Self::Sha1),
            64 => Some(Self::Sha256),
            _ => None,
        }
    }

    /// The name of this algorithm as used in git configuration.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
}


impl std::fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_lengths() {
        assert_eq!(HashAlgorithm::Sha1.digest_len(), 20);
        assert_eq!(HashAlgorithm::Sha256.digest_len(), 32);
    }

    #[test]
    fn hex_lengths() {
        assert_eq!(HashAlgorithm::Sha1.hex_len(), 40);
        assert_eq!(HashAlgorithm::Sha256.hex_len(), 64);
    }

    #[test]
    fn default_is_sha1() {
        assert_eq!(HashAlgorithm::default(), HashAlgorithm::Sha1);
    }

    #[test]
    fn null_oids() {
        let null1 = HashAlgorithm::Sha1.null_oid();
        assert!(null1.is_null());
        assert_eq!(null1.as_bytes().len(), 20);

        let null256 = HashAlgorithm::Sha256.null_oid();
        assert!(null256.is_null());
        assert_eq!(null256.as_bytes().len(), 32);
    }

    #[test]
    fn from_name() {
        assert_eq!(HashAlgorithm::from_name("sha1"), Some(HashAlgorithm::Sha1));
        assert_eq!(
            HashAlgorithm::from_name("sha256"),
            Some(HashAlgorithm::Sha256)
        );
        assert_eq!(HashAlgorithm::from_name("md5"), None);
    }

    #[test]
    fn format_id_roundtrip() {
        for algo in [HashAlgorithm::Sha1, HashAlgorithm::Sha256] {
            assert_eq!(HashAlgorithm::from_format_id(algo.format_id()), Some(algo));
        }
    }

    #[test]
    fn from_lengths() {
        assert_eq!(
            HashAlgorithm::from_digest_len(20),
            Some(HashAlgorithm::Sha1)
        );
        assert_eq!(
            HashAlgorithm::from_hex_len(64),
            Some(HashAlgorithm::Sha256)
        );
        assert_eq!(HashAlgorithm::from_digest_len(16), None);
    }
}

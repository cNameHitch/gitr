use crate::{HashAlgorithm, HashError, ObjectId};

enum HasherInner {
    Sha1(Box<sha1_checked::Sha1>),
    Sha256(sha2::Sha256),
}

/// Streaming hash computation.
///
/// Wraps SHA-1 (with collision detection) and SHA-256 behind a single
/// interface. Data can be fed incrementally with [`update`](Hasher::update)
/// or through the [`std::io::Write`] implementation, then finalised into an
/// [`ObjectId`].
pub struct Hasher {
    inner: HasherInner,
}

impl Hasher {
    /// Create a new hasher for the given algorithm.
    pub fn new(algo: HashAlgorithm) -> Self {
        use digest::Digest;
        let inner = match algo {
            HashAlgorithm::Sha1 => HasherInner::Sha1(Box::new(sha1_checked::Sha1::new())),
            HashAlgorithm::Sha256 => HasherInner::Sha256(sha2::Sha256::new()),
        };
        Self { inner }
    }

    /// Feed data into the hasher.
    pub fn update(&mut self, data: &[u8]) {
        use digest::Digest;
        match &mut self.inner {
            HasherInner::Sha1(h) => h.update(data),
            HasherInner::Sha256(h) => h.update(data),
        }
    }

    /// Finalize and return the ObjectId.
    ///
    /// Returns an error if SHA-1 collision detection fires.
    pub fn finalize(self) -> Result<ObjectId, HashError> {
        match self.inner {
            HasherInner::Sha1(h) => {
                let result = h.try_finalize();
                if result.has_collision() {
                    return Err(HashError::Sha1Collision);
                }
                let mut bytes = [0u8; 20];
                bytes.copy_from_slice(result.hash().as_slice());
                Ok(ObjectId::Sha1(bytes))
            }
            HasherInner::Sha256(h) => {
                use digest::Digest;
                let result = h.finalize();
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(result.as_slice());
                Ok(ObjectId::Sha256(bytes))
            }
        }
    }

    /// Convenience: hash data in one call.
    pub fn digest(algo: HashAlgorithm, data: &[u8]) -> Result<ObjectId, HashError> {
        let mut h = Self::new(algo);
        h.update(data);
        h.finalize()
    }

    /// Hash a git object: `"{type} {len}\0{content}"`.
    pub fn hash_object(
        algo: HashAlgorithm,
        obj_type: &str,
        data: &[u8],
    ) -> Result<ObjectId, HashError> {
        let header = format!("{} {}\0", obj_type, data.len());
        let mut h = Self::new(algo);
        h.update(header.as_bytes());
        h.update(data);
        h.finalize()
    }
}

impl std::io::Write for Hasher {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

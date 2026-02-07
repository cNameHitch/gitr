//! Unified object database for git.
//!
//! Provides a single interface to read and write objects across loose storage,
//! packfiles, and alternate object databases. This is the primary abstraction
//! that all higher-level git operations use to access objects.

pub mod alternates;
pub mod backend;
pub mod prefix;
mod search;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use git_hash::{HashAlgorithm, ObjectId};
use git_loose::LooseObjectStore;
use git_object::{Object, ObjectType};
use git_object::cache::ObjectCache;
use git_pack::pack::PackFile;

pub use backend::OdbBackend;
pub use error::OdbError;

mod error {
    use std::path::PathBuf;

    use git_hash::ObjectId;

    #[derive(Debug, thiserror::Error)]
    pub enum OdbError {
        #[error("object not found: {0}")]
        NotFound(ObjectId),

        #[error("ambiguous object name: {prefix} matches {count} objects")]
        Ambiguous { prefix: String, count: usize },

        #[error("corrupt object {oid}: {reason}")]
        Corrupt { oid: ObjectId, reason: String },

        #[error("alternates error: {0}")]
        Alternates(String),

        #[error("circular alternates chain detected at {0}")]
        CircularAlternates(PathBuf),

        #[error(transparent)]
        Loose(#[from] git_loose::LooseError),

        #[error(transparent)]
        Pack(#[from] git_pack::PackError),

        #[error(transparent)]
        Io(#[from] std::io::Error),
    }
}

/// Lightweight object info (header only, no content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    pub obj_type: ObjectType,
    pub size: usize,
}

/// Unified object database providing access across all storage backends.
pub struct ObjectDatabase {
    /// Loose object store.
    loose: LooseObjectStore,
    /// Pack files (protected by RwLock for refresh).
    packs: RwLock<Vec<PackFile>>,
    /// Alternate object databases.
    alternates: Vec<ObjectDatabase>,
    /// Object cache.
    cache: Mutex<ObjectCache>,
    /// Path to the objects directory.
    objects_dir: PathBuf,
    /// Hash algorithm in use.
    hash_algo: HashAlgorithm,
}

impl ObjectDatabase {
    /// Open the object database at the given objects directory.
    pub fn open(objects_dir: impl AsRef<Path>) -> Result<Self, OdbError> {
        Self::open_with_algo(objects_dir, HashAlgorithm::Sha1)
    }

    /// Open the object database with a specific hash algorithm.
    pub fn open_with_algo(
        objects_dir: impl AsRef<Path>,
        hash_algo: HashAlgorithm,
    ) -> Result<Self, OdbError> {
        let objects_dir = objects_dir.as_ref().to_path_buf();
        let loose = LooseObjectStore::open(&objects_dir, hash_algo);
        let packs = Self::discover_packs(&objects_dir)?;
        let alternates = alternates::load_alternates(&objects_dir, hash_algo)?;

        Ok(Self {
            loose,
            packs: RwLock::new(packs),
            alternates,
            cache: Mutex::new(ObjectCache::new(1024)),
            objects_dir,
            hash_algo,
        })
    }

    /// Read an object by OID (searches loose -> packs -> alternates).
    pub fn read(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError> {
        search::find_object(self, oid)
    }

    /// Read an object with caching.
    pub fn read_cached(&self, oid: &ObjectId) -> Result<Option<Object>, OdbError> {
        // Check cache first
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(obj) = cache.get(oid) {
                return Ok(Some(obj.clone()));
            }
        }

        // Read from storage
        let obj = self.read(oid)?;

        // Insert into cache
        if let Some(ref obj) = obj {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(*oid, obj.clone());
        }

        Ok(obj)
    }

    /// Read just the header (type + size) without full content.
    pub fn read_header(&self, oid: &ObjectId) -> Result<Option<ObjectInfo>, OdbError> {
        search::find_header(self, oid)
    }

    /// Check if an object exists (fast, no decompression for packed objects).
    pub fn contains(&self, oid: &ObjectId) -> bool {
        search::object_exists(self, oid)
    }

    /// Write a new object (always to loose store).
    pub fn write(&self, obj: &Object) -> Result<ObjectId, OdbError> {
        Ok(self.loose.write(obj)?)
    }

    /// Write raw content with type (always to loose store).
    pub fn write_raw(
        &self,
        obj_type: ObjectType,
        content: &[u8],
    ) -> Result<ObjectId, OdbError> {
        Ok(self.loose.write_raw(obj_type, content)?)
    }

    /// Resolve an OID prefix to a full OID.
    /// Returns error if prefix is ambiguous.
    pub fn resolve_prefix(&self, prefix: &str) -> Result<ObjectId, OdbError> {
        prefix::resolve_prefix(self, prefix)
    }

    /// Refresh the list of pack files (call after gc/repack).
    pub fn refresh(&self) -> Result<(), OdbError> {
        let new_packs = Self::discover_packs(&self.objects_dir)?;
        let mut packs = self.packs.write().unwrap();
        *packs = new_packs;
        Ok(())
    }

    /// Iterate over all known object OIDs (for fsck/gc).
    pub fn iter_all_oids(
        &self,
    ) -> Result<Box<dyn Iterator<Item = Result<ObjectId, OdbError>> + '_>, OdbError> {
        let loose_iter = self.loose.iter()?.map(|r| r.map_err(OdbError::from));

        let packs = self.packs.read().unwrap();
        let mut pack_oids: Vec<Result<ObjectId, OdbError>> = Vec::new();
        for pack in packs.iter() {
            for (oid, _offset) in pack.index().iter() {
                pack_oids.push(Ok(oid));
            }
        }

        let alt_oids: Vec<Result<ObjectId, OdbError>> = self
            .alternates
            .iter()
            .flat_map(|alt| match alt.iter_all_oids() {
                Ok(iter) => iter.collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
            .collect();

        Ok(Box::new(
            loose_iter
                .chain(pack_oids)
                .chain(alt_oids),
        ))
    }

    /// Get the path to the objects directory.
    pub fn objects_dir(&self) -> &Path {
        &self.objects_dir
    }

    /// Get the hash algorithm in use.
    pub fn hash_algo(&self) -> HashAlgorithm {
        self.hash_algo
    }

    /// Discover pack files in the objects/pack directory.
    fn discover_packs(objects_dir: &Path) -> Result<Vec<PackFile>, OdbError> {
        let pack_dir = objects_dir.join("pack");
        if !pack_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut packs = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(&pack_dir)?
            .filter_map(|e| e.ok())
            .collect();

        // Sort by modification time (newest first) to match C git behavior
        entries.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        for entry in entries {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "pack") {
                match PackFile::open(&path) {
                    Ok(pack) => packs.push(pack),
                    Err(_) => {
                        // Skip corrupt packs (fall back to other sources)
                        continue;
                    }
                }
            }
        }

        Ok(packs)
    }
}

pub(crate) mod loose;
pub mod packed;
pub(crate) mod transaction;

use std::path::{Path, PathBuf};

use git_hash::ObjectId;
use git_utils::date::Signature;

use crate::error::RefError;
use crate::name::RefName;
use crate::reflog::{self, ReflogEntry};
use crate::store::{RefStore, RefTransaction};
use crate::Reference;

use self::packed::PackedRefs;

/// Maximum depth for following symbolic ref chains.
const MAX_SYMREF_DEPTH: usize = 10;

/// Files-backend ref store (loose refs + packed-refs).
///
/// This is the default ref backend matching C git's files backend:
/// - Loose refs stored as individual files under `.git/refs/`
/// - Packed refs in `.git/packed-refs` for efficiency
/// - Loose refs take precedence over packed refs
/// - Lock files for atomic updates
pub struct FilesRefStore {
    git_dir: PathBuf,
    committer: Option<Signature>,
}

impl FilesRefStore {
    /// Create a new files-based ref store.
    pub fn new(git_dir: impl AsRef<Path>) -> Self {
        Self {
            git_dir: git_dir.as_ref().to_path_buf(),
            committer: None,
        }
    }

    /// Set the committer identity used for reflog entries.
    pub fn set_committer(&mut self, sig: Signature) {
        self.committer = Some(sig);
    }

    /// Get the git directory path.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// Resolve a ref name, following symbolic ref chains up to MAX_SYMREF_DEPTH.
    fn resolve_inner(
        &self,
        name: &RefName,
        depth: usize,
    ) -> Result<Option<ObjectId>, RefError> {
        if depth > MAX_SYMREF_DEPTH {
            return Err(RefError::SymrefLoop(name.to_string()));
        }

        // Check loose ref first
        match loose::read_loose_ref(&self.git_dir, name)? {
            Some(Reference::Direct { target, .. }) => Ok(Some(target)),
            Some(Reference::Symbolic { target, .. }) => {
                self.resolve_inner(&target, depth + 1)
            }
            None => {
                // Fall back to packed refs
                let packed = PackedRefs::load(&self.git_dir)?;
                Ok(packed.find(name).map(|pr| pr.oid))
            }
        }
    }

    /// Write a single ref directly (non-transactional).
    pub fn write_ref(&self, name: &RefName, oid: &ObjectId) -> Result<(), RefError> {
        loose::write_loose_ref(&self.git_dir, name, oid)
    }

    /// Write a symbolic ref directly (non-transactional).
    pub fn write_symbolic_ref(&self, name: &RefName, target: &RefName) -> Result<(), RefError> {
        loose::write_symbolic_ref(&self.git_dir, name, target)
    }

    /// Delete a ref directly (non-transactional).
    pub fn delete_ref(&self, name: &RefName) -> Result<(), RefError> {
        loose::delete_loose_ref(&self.git_dir, name)
    }

    /// Commit a transaction atomically.
    pub fn commit_transaction(&self, transaction: RefTransaction) -> Result<(), RefError> {
        transaction::commit_transaction(&self.git_dir, transaction, self.committer.as_ref())
    }

    /// Load the packed-refs file.
    pub fn packed_refs(&self) -> Result<PackedRefs, RefError> {
        PackedRefs::load(&self.git_dir)
    }

    /// Pack a loose ref into packed-refs and remove the loose file.
    pub fn pack_ref(&self, name: &RefName) -> Result<(), RefError> {
        // Read the current loose ref value
        let oid = match loose::read_loose_ref(&self.git_dir, name)? {
            Some(Reference::Direct { target, .. }) => target,
            Some(Reference::Symbolic { .. }) => {
                return Err(RefError::PackedRefs(
                    "cannot pack symbolic ref".into(),
                ));
            }
            None => return Err(RefError::NotFound(name.to_string())),
        };

        // Add to packed-refs
        let mut packed = PackedRefs::load(&self.git_dir)?;
        packed.upsert(name.clone(), oid, None);
        packed.write(&self.git_dir)?;

        // Remove the loose file
        loose::delete_loose_ref(&self.git_dir, name)?;

        Ok(())
    }
}

impl RefStore for FilesRefStore {
    fn resolve(&self, name: &RefName) -> Result<Option<Reference>, RefError> {
        // Check loose ref first
        match loose::read_loose_ref(&self.git_dir, name)? {
            Some(r) => Ok(Some(r)),
            None => {
                // Fall back to packed refs
                let packed = PackedRefs::load(&self.git_dir)?;
                Ok(packed.find(name).map(|pr| Reference::Direct {
                    name: pr.name.clone(),
                    target: pr.oid,
                }))
            }
        }
    }

    fn resolve_to_oid(&self, name: &RefName) -> Result<Option<ObjectId>, RefError> {
        self.resolve_inner(name, 0)
    }

    fn iter(
        &self,
        prefix: Option<&str>,
    ) -> Result<Box<dyn Iterator<Item = Result<Reference, RefError>> + '_>, RefError> {
        // Collect all loose refs
        let loose_refs = loose::enumerate_loose_refs(&self.git_dir, prefix)?;
        let mut loose_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        let mut all_refs: Vec<Result<Reference, RefError>> = Vec::new();

        for (name, _path) in &loose_refs {
            loose_names.insert(name.as_str().to_string());
            match loose::read_loose_ref(&self.git_dir, name) {
                Ok(Some(r)) => all_refs.push(Ok(r)),
                Ok(None) => {} // File may have been deleted between enumerate and read
                Err(e) => all_refs.push(Err(e)),
            }
        }

        // Add packed refs that aren't overridden by loose refs
        let packed = PackedRefs::load(&self.git_dir)?;
        for pr in packed.refs() {
            if loose_names.contains(pr.name.as_str()) {
                continue; // Loose ref takes precedence
            }
            if let Some(p) = prefix {
                if !pr.name.as_str().starts_with(p) {
                    continue;
                }
            }
            all_refs.push(Ok(Reference::Direct {
                name: pr.name.clone(),
                target: pr.oid,
            }));
        }

        // Sort by ref name
        all_refs.sort_by(|a, b| {
            let name_a = match a {
                Ok(r) => r.name().clone(),
                Err(_) => RefName::new_unchecked(""),
            };
            let name_b = match b {
                Ok(r) => r.name().clone(),
                Err(_) => RefName::new_unchecked(""),
            };
            name_a.cmp(&name_b)
        });

        Ok(Box::new(all_refs.into_iter()))
    }

    fn reflog(&self, name: &RefName) -> Result<Vec<ReflogEntry>, RefError> {
        reflog::read_reflog(&self.git_dir, name)
    }

    fn append_reflog(&self, name: &RefName, entry: &ReflogEntry) -> Result<(), RefError> {
        reflog::append_reflog_entry(&self.git_dir, name, entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::BString;
    use git_utils::date::GitDate;

    fn test_store(dir: &Path) -> FilesRefStore {
        let mut store = FilesRefStore::new(dir);
        store.set_committer(Signature {
            name: BString::from("Test User"),
            email: BString::from("test@example.com"),
            date: GitDate::new(1234567890, 0),
        });
        store
    }

    #[test]
    fn resolve_direct_ref() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        loose::write_loose_ref(dir.path(), &name, &oid).unwrap();

        let resolved = store.resolve_to_oid(&name).unwrap().unwrap();
        assert_eq!(resolved, oid);
    }

    #[test]
    fn resolve_symbolic_ref_chain() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        // HEAD -> refs/heads/main -> OID
        let main_name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        loose::write_loose_ref(dir.path(), &main_name, &oid).unwrap();

        let head = RefName::new("HEAD").unwrap();
        loose::write_symbolic_ref(dir.path(), &head, &main_name).unwrap();

        let resolved = store.resolve_to_oid(&head).unwrap().unwrap();
        assert_eq!(resolved, oid);
    }

    #[test]
    fn resolve_detached_head() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let head = RefName::new("HEAD").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        loose::write_loose_ref(dir.path(), &head, &oid).unwrap();

        let resolved = store.resolve_to_oid(&head).unwrap().unwrap();
        assert_eq!(resolved, oid);
    }

    #[test]
    fn resolve_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/nonexistent").unwrap();
        assert!(store.resolve_to_oid(&name).unwrap().is_none());
    }

    #[test]
    fn resolve_symref_loop_detected() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        // A -> B -> A (loop)
        let a = RefName::new("refs/heads/a").unwrap();
        let b = RefName::new("refs/heads/b").unwrap();
        loose::write_symbolic_ref(dir.path(), &a, &b).unwrap();
        loose::write_symbolic_ref(dir.path(), &b, &a).unwrap();

        let result = store.resolve_to_oid(&a);
        assert!(matches!(result, Err(RefError::SymrefLoop(_))));
    }

    #[test]
    fn loose_over_packed_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/main").unwrap();
        let packed_oid =
            ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let loose_oid =
            ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();

        // Write to packed-refs
        let mut packed = PackedRefs::load(dir.path()).unwrap();
        packed.upsert(name.clone(), packed_oid, None);
        packed.write(dir.path()).unwrap();

        // Write loose ref (should take precedence)
        loose::write_loose_ref(dir.path(), &name, &loose_oid).unwrap();

        let resolved = store.resolve_to_oid(&name).unwrap().unwrap();
        assert_eq!(resolved, loose_oid);
    }

    #[test]
    fn resolve_from_packed_when_no_loose() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();

        let mut packed = PackedRefs::load(dir.path()).unwrap();
        packed.upsert(name.clone(), oid, None);
        packed.write(dir.path()).unwrap();

        let resolved = store.resolve_to_oid(&name).unwrap().unwrap();
        assert_eq!(resolved, oid);
    }

    #[test]
    fn iterate_all_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        loose::write_loose_ref(
            dir.path(),
            &RefName::new("refs/heads/main").unwrap(),
            &oid,
        )
        .unwrap();
        loose::write_loose_ref(
            dir.path(),
            &RefName::new("refs/heads/feature").unwrap(),
            &oid,
        )
        .unwrap();
        loose::write_loose_ref(
            dir.path(),
            &RefName::new("refs/tags/v1.0").unwrap(),
            &oid,
        )
        .unwrap();

        let refs: Vec<_> = store.iter(None).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(refs.len(), 3);
        // Should be sorted
        assert_eq!(refs[0].name().as_str(), "refs/heads/feature");
        assert_eq!(refs[1].name().as_str(), "refs/heads/main");
        assert_eq!(refs[2].name().as_str(), "refs/tags/v1.0");
    }

    #[test]
    fn iterate_with_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        loose::write_loose_ref(
            dir.path(),
            &RefName::new("refs/heads/main").unwrap(),
            &oid,
        )
        .unwrap();
        loose::write_loose_ref(
            dir.path(),
            &RefName::new("refs/tags/v1.0").unwrap(),
            &oid,
        )
        .unwrap();

        let refs: Vec<_> = store
            .iter(Some("refs/heads/"))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name().as_str(), "refs/heads/main");
    }

    #[test]
    fn iterate_deduplicates_loose_and_packed() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

        // Same ref in both loose and packed
        loose::write_loose_ref(dir.path(), &name, &oid).unwrap();
        let mut packed = PackedRefs::load(dir.path()).unwrap();
        packed.upsert(
            name,
            ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
            None,
        );
        packed.write(dir.path()).unwrap();

        let refs: Vec<_> = store.iter(None).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(refs.len(), 1); // Deduplicated
        // Should use the loose ref value
        match &refs[0] {
            Reference::Direct { target, .. } => assert_eq!(*target, oid),
            _ => panic!("expected Direct ref"),
        }
    }

    #[test]
    fn pack_ref_operation() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        loose::write_loose_ref(dir.path(), &name, &oid).unwrap();

        // Pack the ref
        store.pack_ref(&name).unwrap();

        // Loose file should be gone
        assert!(!loose::loose_ref_path(dir.path(), &name).exists());

        // But ref should still resolve via packed-refs
        let resolved = store.resolve_to_oid(&name).unwrap().unwrap();
        assert_eq!(resolved, oid);
    }

    #[test]
    fn transaction_with_reflog() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

        let mut tx = RefTransaction::new();
        tx.create(name.clone(), oid, "branch: Created from HEAD");
        store.commit_transaction(tx).unwrap();

        // Verify reflog
        let entries = store.reflog(&name).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].old_oid.is_null());
        assert_eq!(entries[0].new_oid, oid);
    }

    #[test]
    fn dangling_symref() {
        let dir = tempfile::tempdir().unwrap();
        let store = test_store(dir.path());

        // HEAD points to unborn branch
        let head = RefName::new("HEAD").unwrap();
        let target = RefName::new("refs/heads/main").unwrap();
        loose::write_symbolic_ref(dir.path(), &head, &target).unwrap();

        // resolve returns the symbolic ref
        let reference = store.resolve(&head).unwrap().unwrap();
        assert!(reference.is_symbolic());

        // resolve_to_oid returns None (unborn branch)
        assert!(store.resolve_to_oid(&head).unwrap().is_none());
    }
}

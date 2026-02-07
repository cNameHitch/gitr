use std::path::Path;

use git_hash::ObjectId;
use git_utils::date::Signature;
use git_utils::lockfile::LockFile;

use crate::error::RefError;
use crate::files::loose;
use crate::files::packed::PackedRefs;
use crate::name::RefName;
use crate::reflog::{self, ReflogEntry};
use crate::store::{RefTransaction, RefUpdateAction};
use crate::Reference;

/// Commit a ref transaction atomically against the files backend.
///
/// The protocol:
/// 1. Acquire locks on all refs being updated
/// 2. Verify all CAS (compare-and-swap) conditions
/// 3. Write new values to lock files
/// 4. Commit all lock files (atomic rename)
/// 5. Append reflog entries
///
/// If any step fails, all locks are released (rollback via Drop).
pub(crate) fn commit_transaction(
    git_dir: &Path,
    transaction: RefTransaction,
    committer: Option<&Signature>,
) -> Result<(), RefError> {
    if transaction.is_empty() {
        return Ok(());
    }

    let packed = PackedRefs::load(git_dir)?;

    // Phase 1: Acquire all locks, verify CAS conditions, and prepare writes.
    // We store locks and the current OID for each update.
    let mut locks: Vec<LockFile> = Vec::new();
    let mut current_oids: Vec<Option<ObjectId>> = Vec::new();

    for update in transaction.updates() {
        let lock_path = loose::loose_ref_path(git_dir, &update.name);

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RefError::IoPath {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let lock = LockFile::acquire(&lock_path)?;

        // Read current value
        let current = loose::read_loose_ref(git_dir, &update.name)?;
        let current_oid = match &current {
            Some(Reference::Direct { target, .. }) => Some(*target),
            Some(Reference::Symbolic { target, .. }) => {
                resolve_symref(git_dir, target, &packed, 10)?
            }
            None => {
                // Check packed refs
                packed.find(&update.name).map(|pr| pr.oid)
            }
        };

        // Verify CAS condition
        verify_cas(&update.name, &update.action, current_oid.as_ref(), &current)?;

        locks.push(lock);
        current_oids.push(current_oid);
    }

    // Phase 2: Write new values to lock files
    for (i, update) in transaction.updates().iter().enumerate() {
        match &update.action {
            RefUpdateAction::Create { new_target }
            | RefUpdateAction::Update { new_target, .. } => {
                use std::io::Write;
                let content = format!("{}\n", new_target.to_hex());
                locks[i]
                    .write_all(content.as_bytes())
                    .map_err(|e| RefError::IoPath {
                        path: locks[i].path().to_path_buf(),
                        source: e,
                    })?;
            }
            RefUpdateAction::Delete { .. } => {
                // No content to write; will handle in commit phase
            }
            RefUpdateAction::SetSymbolic { target } => {
                use std::io::Write;
                let content = format!("ref: {}\n", target);
                locks[i]
                    .write_all(content.as_bytes())
                    .map_err(|e| RefError::IoPath {
                        path: locks[i].path().to_path_buf(),
                        source: e,
                    })?;
            }
        }
    }

    // Phase 3: Commit all locks (atomic rename) or delete.
    // We drain the locks to take ownership.
    let updates = transaction.updates;
    let locks_vec: Vec<LockFile> = std::mem::take(&mut locks);

    for (i, lock) in locks_vec.into_iter().enumerate() {
        match &updates[i].action {
            RefUpdateAction::Delete { .. } => {
                // Rollback the lock (we don't want to commit a delete as a file)
                lock.rollback()?;
                // Delete the loose ref file
                loose::delete_loose_ref(git_dir, &updates[i].name)?;
            }
            _ => {
                lock.commit()?;
            }
        }
    }

    // Phase 4: Write reflog entries
    if let Some(sig) = committer {
        for (i, update) in updates.iter().enumerate() {
            if let Some(msg) = &update.reflog_message {
                let (old_oid, new_oid) = match &update.action {
                    RefUpdateAction::Create { new_target } => {
                        (ObjectId::NULL_SHA1, *new_target)
                    }
                    RefUpdateAction::Update { new_target, .. } => {
                        (current_oids[i].unwrap_or(ObjectId::NULL_SHA1), *new_target)
                    }
                    RefUpdateAction::Delete { .. } => {
                        (
                            current_oids[i].unwrap_or(ObjectId::NULL_SHA1),
                            ObjectId::NULL_SHA1,
                        )
                    }
                    RefUpdateAction::SetSymbolic { .. } => continue,
                };

                let entry = ReflogEntry {
                    old_oid,
                    new_oid,
                    identity: sig.clone(),
                    message: msg.as_str().into(),
                };
                reflog::append_reflog_entry(git_dir, &update.name, &entry)?;
            }
        }
    }

    Ok(())
}

/// Verify the CAS condition for a ref update.
fn verify_cas(
    name: &RefName,
    action: &RefUpdateAction,
    current_oid: Option<&ObjectId>,
    current_ref: &Option<Reference>,
) -> Result<(), RefError> {
    match action {
        RefUpdateAction::Create { .. } => {
            // Fail if the ref already exists (as loose or packed)
            if current_ref.is_some() || current_oid.is_some() {
                return Err(RefError::AlreadyExists(name.to_string()));
            }
        }
        RefUpdateAction::Update {
            old_target,
            new_target: _,
        } => {
            let actual = current_oid.ok_or_else(|| RefError::NotFound(name.to_string()))?;
            if actual != old_target {
                return Err(RefError::CasFailed {
                    name: name.to_string(),
                    expected: *old_target,
                    actual: *actual,
                });
            }
        }
        RefUpdateAction::Delete { old_target } => {
            let actual = current_oid.ok_or_else(|| RefError::NotFound(name.to_string()))?;
            if actual != old_target {
                return Err(RefError::CasFailed {
                    name: name.to_string(),
                    expected: *old_target,
                    actual: *actual,
                });
            }
        }
        RefUpdateAction::SetSymbolic { .. } => {
            // No CAS check for symbolic refs
        }
    }
    Ok(())
}

/// Resolve a symbolic ref chain to an OID, checking packed-refs as fallback.
fn resolve_symref(
    git_dir: &Path,
    name: &RefName,
    packed: &PackedRefs,
    max_depth: usize,
) -> Result<Option<ObjectId>, RefError> {
    if max_depth == 0 {
        return Err(RefError::SymrefLoop(name.to_string()));
    }

    match loose::read_loose_ref(git_dir, name)? {
        Some(Reference::Direct { target, .. }) => Ok(Some(target)),
        Some(Reference::Symbolic { target, .. }) => {
            resolve_symref(git_dir, &target, packed, max_depth - 1)
        }
        None => Ok(packed.find(name).map(|pr| pr.oid)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::BString;
    use crate::store::RefTransaction;
    use git_utils::date::GitDate;

    fn test_sig() -> Signature {
        Signature {
            name: BString::from("Test User"),
            email: BString::from("test@example.com"),
            date: GitDate::new(1234567890, 0),
        }
    }

    #[test]
    fn create_single_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let mut tx = RefTransaction::new();
        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();
        tx.create(name.clone(), oid, "branch: Created from HEAD");

        commit_transaction(git_dir, tx, Some(&test_sig())).unwrap();

        let r = loose::read_loose_ref(git_dir, &name).unwrap().unwrap();
        match r {
            Reference::Direct { target, .. } => assert_eq!(target, oid),
            _ => panic!("expected Direct ref"),
        }
    }

    #[test]
    fn update_ref_with_cas() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let name = RefName::new("refs/heads/main").unwrap();
        let old_oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let new_oid = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();

        loose::write_loose_ref(git_dir, &name, &old_oid).unwrap();

        let mut tx = RefTransaction::new();
        tx.update(name.clone(), old_oid, new_oid, "commit: second commit");
        commit_transaction(git_dir, tx, Some(&test_sig())).unwrap();

        let r = loose::read_loose_ref(git_dir, &name).unwrap().unwrap();
        match r {
            Reference::Direct { target, .. } => assert_eq!(target, new_oid),
            _ => panic!("expected Direct ref"),
        }
    }

    #[test]
    fn update_ref_cas_failure() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let name = RefName::new("refs/heads/main").unwrap();
        let actual_oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let wrong_old = ObjectId::from_hex("cccccccccccccccccccccccccccccccccccccccc").unwrap();
        let new_oid = ObjectId::from_hex("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();

        loose::write_loose_ref(git_dir, &name, &actual_oid).unwrap();

        let mut tx = RefTransaction::new();
        tx.update(name, wrong_old, new_oid, "should fail");

        let result = commit_transaction(git_dir, tx, Some(&test_sig()));
        assert!(matches!(result, Err(RefError::CasFailed { .. })));
    }

    #[test]
    fn delete_ref_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        loose::write_loose_ref(git_dir, &name, &oid).unwrap();

        let mut tx = RefTransaction::new();
        tx.delete(name.clone(), oid, "branch: deleted");
        commit_transaction(git_dir, tx, Some(&test_sig())).unwrap();

        assert!(loose::read_loose_ref(git_dir, &name).unwrap().is_none());
    }

    #[test]
    fn create_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        loose::write_loose_ref(git_dir, &name, &oid).unwrap();

        let mut tx = RefTransaction::new();
        tx.create(name, oid, "should fail");

        let result = commit_transaction(git_dir, tx, Some(&test_sig()));
        assert!(matches!(result, Err(RefError::AlreadyExists(_))));
    }

    #[test]
    fn set_symbolic_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let name = RefName::new("HEAD").unwrap();
        let target = RefName::new("refs/heads/main").unwrap();

        let mut tx = RefTransaction::new();
        tx.set_symbolic(name.clone(), target.clone(), "checkout: moving to main");
        commit_transaction(git_dir, tx, Some(&test_sig())).unwrap();

        let r = loose::read_loose_ref(git_dir, &name).unwrap().unwrap();
        match r {
            Reference::Symbolic {
                target: found, ..
            } => assert_eq!(found, target),
            _ => panic!("expected Symbolic ref"),
        }
    }

    #[test]
    fn transaction_creates_reflog() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

        let mut tx = RefTransaction::new();
        tx.create(name.clone(), oid, "branch: Created");
        commit_transaction(git_dir, tx, Some(&test_sig())).unwrap();

        let entries = reflog::read_reflog(git_dir, &name).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].old_oid.is_null());
        assert_eq!(entries[0].new_oid, oid);
        assert_eq!(entries[0].message, BString::from("branch: Created"));
    }
}

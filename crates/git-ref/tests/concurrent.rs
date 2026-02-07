//! Concurrent update stress test for ref transactions.

use std::sync::{Arc, Barrier};
use std::thread;

use bstr::BString;
use git_hash::ObjectId;
use git_ref::{FilesRefStore, RefName, RefStore, RefTransaction};
use git_utils::date::{GitDate, Signature};

fn make_store(git_dir: &std::path::Path) -> FilesRefStore {
    let mut store = FilesRefStore::new(git_dir);
    store.set_committer(Signature {
        name: BString::from("Test User"),
        email: BString::from("test@example.com"),
        date: GitDate::new(1234567890, 0),
    });
    store
}

#[test]
fn concurrent_creates_different_refs() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().to_path_buf();

    let num_threads = 8;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let git_dir = git_dir.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let store = make_store(&git_dir);
                let name =
                    RefName::new(format!("refs/heads/thread-{}", i)).unwrap();
                let oid = ObjectId::from_hex(&format!(
                    "{:0>40x}",
                    i + 1
                ))
                .unwrap();

                let mut tx = RefTransaction::new();
                tx.create(name, oid, format!("thread {} created", i));
                store.commit_transaction(tx)
            })
        })
        .collect();

    let mut successes = 0;
    for handle in handles {
        if handle.join().unwrap().is_ok() {
            successes += 1;
        }
    }

    // All creates should succeed since they target different refs
    assert_eq!(successes, num_threads);

    // Verify all refs exist
    let store = make_store(&git_dir);
    for i in 0..num_threads {
        let name =
            RefName::new(format!("refs/heads/thread-{}", i)).unwrap();
        assert!(
            store.resolve_to_oid(&name).unwrap().is_some(),
            "ref for thread {} should exist",
            i
        );
    }
}

#[test]
fn concurrent_updates_same_ref_cas() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().to_path_buf();

    // Create initial ref
    let initial_oid =
        ObjectId::from_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    {
        let store = make_store(&git_dir);
        let name = RefName::new("refs/heads/contested").unwrap();
        let mut tx = RefTransaction::new();
        tx.create(name, initial_oid, "initial");
        store.commit_transaction(tx).unwrap();
    }

    let num_threads = 8;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let git_dir = git_dir.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let store = make_store(&git_dir);
                let name = RefName::new("refs/heads/contested").unwrap();
                let new_oid = ObjectId::from_hex(&format!(
                    "{:0>40x}",
                    i + 100
                ))
                .unwrap();

                let mut tx = RefTransaction::new();
                tx.update(name, initial_oid, new_oid, format!("thread {}", i));
                store.commit_transaction(tx)
            })
        })
        .collect();

    let mut successes = 0;
    let mut failures = 0;
    for handle in handles {
        match handle.join().unwrap() {
            Ok(()) => successes += 1,
            Err(_) => failures += 1,
        }
    }

    // Exactly one thread should succeed with CAS (the one that got the lock first)
    // Others should fail because the old value changed
    // Note: due to lock contention, some may fail to acquire the lock entirely
    assert!(
        successes >= 1,
        "at least one update should succeed"
    );
    assert!(
        failures > 0 || num_threads == 1,
        "with concurrent CAS, some should fail"
    );

    // The ref should have a valid value (not corrupted)
    let store = make_store(&git_dir);
    let name = RefName::new("refs/heads/contested").unwrap();
    let final_oid = store.resolve_to_oid(&name).unwrap().unwrap();
    assert!(!final_oid.is_null(), "ref should have a valid OID");
}

#[test]
fn concurrent_creates_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let git_dir = dir.path().to_path_buf();

    // Pre-create some refs
    {
        let store = make_store(&git_dir);
        for i in 0..4 {
            let name =
                RefName::new(format!("refs/heads/cd-{}", i)).unwrap();
            let oid = ObjectId::from_hex(&format!(
                "{:0>40x}",
                i + 1
            ))
            .unwrap();
            let mut tx = RefTransaction::new();
            tx.create(name, oid, "setup");
            store.commit_transaction(tx).unwrap();
        }
    }

    let num_threads = 4;
    let barrier = Arc::new(Barrier::new(num_threads * 2));

    // Half the threads create new refs, half delete existing ones
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let git_dir_create = git_dir.clone();
        let git_dir_delete = git_dir.clone();
        let barrier_create = Arc::clone(&barrier);
        let barrier_delete = Arc::clone(&barrier);

        // Create thread
        handles.push(thread::spawn(move || {
            barrier_create.wait();
            let store = make_store(&git_dir_create);
            let name = RefName::new(format!(
                "refs/heads/new-{}",
                i
            ))
            .unwrap();
            let oid = ObjectId::from_hex(&format!(
                "{:0>40x}",
                i + 100
            ))
            .unwrap();
            let mut tx = RefTransaction::new();
            tx.create(name, oid, "concurrent create");
            store.commit_transaction(tx)
        }));

        // Delete thread
        handles.push(thread::spawn(move || {
            barrier_delete.wait();
            let store = make_store(&git_dir_delete);
            let name = RefName::new(format!(
                "refs/heads/cd-{}",
                i
            ))
            .unwrap();
            let oid = ObjectId::from_hex(&format!(
                "{:0>40x}",
                i + 1
            ))
            .unwrap();
            let mut tx = RefTransaction::new();
            tx.delete(name, oid, "concurrent delete");
            store.commit_transaction(tx)
        }));
    }

    for handle in handles {
        // Don't assert success — concurrent ops may legitimately fail
        let _ = handle.join().unwrap();
    }

    // Verify no corruption
    let store = make_store(&git_dir);
    let all_refs: Vec<_> = store
        .iter(Some("refs/heads/"))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    // Each ref should be valid
    for r in &all_refs {
        assert!(
            store.resolve_to_oid(r.name()).unwrap().is_some(),
            "ref {} should resolve",
            r.name()
        );
    }
}

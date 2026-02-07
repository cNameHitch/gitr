//! Lockfile stress tests to verify concurrent locking behavior.

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

use git_utils::lockfile::LockFile;

#[test]
fn concurrent_lock_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("test.txt");

    // Create the file first
    fs::write(&target, "initial").unwrap();

    let barrier = Arc::new(Barrier::new(10));
    // Second barrier ensures all threads try to acquire before any drop
    let hold_barrier = Arc::new(Barrier::new(10));
    let target_arc = Arc::new(target.clone());
    let mut handles = vec![];

    // 10 threads all try to acquire the same lock simultaneously
    for _ in 0..10 {
        let barrier = Arc::clone(&barrier);
        let hold = Arc::clone(&hold_barrier);
        let target = Arc::clone(&target_arc);
        handles.push(thread::spawn(move || -> bool {
            barrier.wait();
            let result = LockFile::try_acquire(&*target);
            let got_lock = matches!(&result, Ok(Some(_)));
            // Hold the lock until all threads have attempted acquisition
            hold.wait();
            drop(result);
            got_lock
        }));
    }

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let successes: usize = results.iter().filter(|&&r| r).count();

    // Exactly one thread should have acquired the lock
    assert_eq!(
        successes, 1,
        "expected exactly 1 lock acquisition, got {}",
        successes
    );
}

#[test]
fn lock_release_and_reacquire() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("test.txt");
    fs::write(&target, "data").unwrap();

    // Acquire and release multiple times
    for i in 0..10 {
        let lock = LockFile::try_acquire(&target).unwrap();
        assert!(
            lock.is_some(),
            "failed to acquire lock on iteration {}",
            i
        );
        // Lock is dropped here, releasing it
    }
}

#[test]
fn lock_commit_then_reacquire() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("output.txt");

    for i in 0..5 {
        let mut lock = LockFile::acquire(&target).unwrap();
        use std::io::Write;
        write!(lock, "iteration {}", i).unwrap();
        lock.commit().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, format!("iteration {}", i));
    }
}

#[test]
fn lock_rollback_preserves_original() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("data.txt");
    fs::write(&target, "original").unwrap();

    {
        let mut lock = LockFile::acquire(&target).unwrap();
        use std::io::Write;
        write!(lock, "modified").unwrap();
        let _ = lock.rollback();
    }

    let content = fs::read_to_string(&target).unwrap();
    assert_eq!(content, "original");
}

#[test]
fn lock_drop_cleans_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("test.txt");
    fs::write(&target, "data").unwrap();

    let lock_path = target.with_extension("txt.lock");

    {
        let _lock = LockFile::acquire(&target).unwrap();
        assert!(lock_path.exists(), "lock file should exist while held");
    }

    assert!(!lock_path.exists(), "lock file should be removed after drop");
}

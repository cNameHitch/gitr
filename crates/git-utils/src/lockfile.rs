use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::{LockError, UtilError};
use crate::Result;

/// RAII lock file guard. Creates a `.lock` file on construction,
/// atomically renames on commit, removes on drop if not committed.
///
/// This matches C git's lock file protocol:
/// - Create `<path>.lock` with O_CREAT|O_EXCL
/// - Write new contents to the lock file
/// - Atomically rename `.lock` to target on commit
/// - Remove `.lock` on drop if not committed (rollback)
pub struct LockFile {
    /// The target file path (without .lock suffix).
    path: PathBuf,
    /// The lock file path (with .lock suffix).
    lock_path: PathBuf,
    /// The open file handle for writing.
    file: Option<File>,
    /// Whether commit() has been called.
    committed: bool,
}

const LOCK_SUFFIX: &str = ".lock";

impl LockFile {
    /// Acquire a lock on the given path. Creates `path.lock` using O_CREAT|O_EXCL.
    ///
    /// Returns an error if the lock file already exists (another process holds the lock)
    /// or if the file cannot be created.
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let lock_path = PathBuf::from(format!("{}{}", path.display(), LOCK_SUFFIX));

        let file = OpenOptions::new()
            .write(true)
            .create_new(true) // O_CREAT|O_EXCL equivalent
            .open(&lock_path)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    UtilError::Lock(LockError::AlreadyLocked {
                        path: lock_path.clone(),
                    })
                } else {
                    UtilError::Lock(LockError::Create {
                        path: lock_path.clone(),
                        source: e,
                    })
                }
            })?;

        Ok(Self {
            path,
            lock_path,
            file: Some(file),
            committed: false,
        })
    }

    /// Try to acquire without blocking. Returns Ok(None) if already locked,
    /// Ok(Some(lockfile)) on success, or Err on other failures.
    pub fn try_acquire(path: impl AsRef<Path>) -> Result<Option<Self>> {
        match Self::acquire(path) {
            Ok(lk) => Ok(Some(lk)),
            Err(UtilError::Lock(LockError::AlreadyLocked { .. })) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a mutable reference to the underlying file for writing.
    pub fn file_mut(&mut self) -> Option<&mut File> {
        self.file.as_mut()
    }

    /// Get the path of the target file (without .lock).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the path of the lock file (with .lock).
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Commit: close the file and atomically rename .lock to target.
    pub fn commit(mut self) -> Result<()> {
        // Flush and sync the file
        if let Some(ref mut file) = self.file {
            file.flush().map_err(|e| {
                UtilError::Lock(LockError::Commit {
                    path: self.lock_path.clone(),
                    source: e,
                })
            })?;
            file.sync_all().map_err(|e| {
                UtilError::Lock(LockError::Commit {
                    path: self.lock_path.clone(),
                    source: e,
                })
            })?;
        }
        // Drop the file handle before rename
        self.file.take();

        // Atomic rename
        fs::rename(&self.lock_path, &self.path).map_err(|e| {
            UtilError::Lock(LockError::Commit {
                path: self.lock_path.clone(),
                source: e,
            })
        })?;

        self.committed = true;
        Ok(())
    }

    /// Rollback: remove .lock file (also happens on Drop).
    pub fn rollback(mut self) -> Result<()> {
        self.file.take();
        if self.lock_path.exists() {
            fs::remove_file(&self.lock_path)?;
        }
        self.committed = true; // Prevent Drop from trying to clean up again
        Ok(())
    }
}

impl Write for LockFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("lock file already closed"))?
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("lock file already closed"))?
            .flush()
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        if !self.committed {
            self.file.take();
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acquire_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("test.txt");

        // Write initial content
        fs::write(&target, b"old content").unwrap();

        // Acquire lock
        let mut lock = LockFile::acquire(&target).unwrap();
        assert!(lock.lock_path().exists());

        // Write new content
        lock.write_all(b"new content").unwrap();

        // Commit
        lock.commit().unwrap();

        // Verify
        assert!(!dir.path().join("test.txt.lock").exists());
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn acquire_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("test.txt");
        fs::write(&target, b"original").unwrap();

        {
            let mut lock = LockFile::acquire(&target).unwrap();
            lock.write_all(b"should not persist").unwrap();
            lock.rollback().unwrap();
        }

        // Original content should be unchanged
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original");
        assert!(!dir.path().join("test.txt.lock").exists());
    }

    #[test]
    fn drop_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("test.txt");
        fs::write(&target, b"original").unwrap();

        {
            let mut lock = LockFile::acquire(&target).unwrap();
            lock.write_all(b"dropped content").unwrap();
            // Drop without commit
        }

        // Lock file should be cleaned up
        assert!(!dir.path().join("test.txt.lock").exists());
        // Original should be unchanged
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn double_lock_fails() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("test.txt");
        fs::write(&target, b"content").unwrap();

        let _lock1 = LockFile::acquire(&target).unwrap();

        // Second lock should fail
        match LockFile::acquire(&target) {
            Err(UtilError::Lock(LockError::AlreadyLocked { .. })) => {}
            Err(e) => panic!("expected AlreadyLocked, got error: {}", e),
            Ok(_) => panic!("expected AlreadyLocked, got Ok"),
        }
    }

    #[test]
    fn try_acquire_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("test.txt");
        fs::write(&target, b"content").unwrap();

        let _lock1 = LockFile::acquire(&target).unwrap();

        // try_acquire should return None
        let result = LockFile::try_acquire(&target).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn lock_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("new_file.txt");

        let mut lock = LockFile::acquire(&target).unwrap();
        lock.write_all(b"created via lock").unwrap();
        lock.commit().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "created via lock");
    }
}

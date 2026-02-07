use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::Result;

/// A temporary file with RAII cleanup.
///
/// The temporary file is created in the same directory as the target path
/// (to ensure atomic rename is possible). It is automatically deleted
/// when dropped, unless it has been persisted.
pub struct TempFile {
    inner: Option<::tempfile::NamedTempFile>,
    persisted: bool,
}

impl TempFile {
    /// Create a new temporary file in the given directory with a unique name.
    pub fn new_in(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;

        let named = ::tempfile::NamedTempFile::new_in(dir)?;

        Ok(Self {
            inner: Some(named),
            persisted: false,
        })
    }

    /// Create a new temporary file alongside the given target path.
    /// The temp file will be in the same directory as `target`.
    pub fn new_for(target: impl AsRef<Path>) -> Result<Self> {
        let target = target.as_ref();
        let dir = target.parent().unwrap_or(Path::new("."));
        Self::new_in(dir)
    }

    /// Get the path of the temporary file.
    pub fn path(&self) -> &Path {
        self.inner.as_ref().map(|n| n.path()).unwrap_or(Path::new(""))
    }

    /// Get a mutable reference to the file handle.
    pub fn file_mut(&mut self) -> Option<&mut std::fs::File> {
        self.inner.as_mut().map(|n| n.as_file_mut())
    }

    /// Persist the temporary file by renaming it to the target path.
    /// This consumes the TempFile.
    pub fn persist(mut self, target: impl AsRef<Path>) -> Result<()> {
        if let Some(named) = self.inner.take() {
            named
                .persist(target.as_ref())
                .map_err(|e| crate::error::UtilError::Io(e.error))?;
        }
        self.persisted = true;
        Ok(())
    }

    /// Close the file handle without deleting or persisting.
    /// The file remains on disk until this TempFile is dropped.
    pub fn close(&mut self) {
        // Dropping the inner NamedTempFile will delete it,
        // so we don't close here - just mark that we want cleanup on drop
    }
}

impl Write for TempFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner
            .as_mut()
            .ok_or_else(|| io::Error::other("temp file already closed"))?
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner
            .as_mut()
            .ok_or_else(|| io::Error::other("temp file already closed"))?
            .flush()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // NamedTempFile automatically deletes on drop if not persisted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_persist() {
        let dir = ::tempfile::tempdir().unwrap();
        let target = dir.path().join("output.txt");

        let mut tf = TempFile::new_for(&target).unwrap();
        tf.write_all(b"hello world").unwrap();
        tf.persist(&target).unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn drop_cleans_up() {
        let dir = ::tempfile::tempdir().unwrap();
        let temp_path;

        {
            let mut tf = TempFile::new_in(dir.path()).unwrap();
            temp_path = tf.path().to_path_buf();
            tf.write_all(b"temporary").unwrap();
            assert!(temp_path.exists());
        }

        // After drop, the temp file should be gone
        assert!(!temp_path.exists());
    }

    #[test]
    fn new_for_creates_in_same_dir() {
        let dir = ::tempfile::tempdir().unwrap();
        let target = dir.path().join("subdir").join("file.txt");
        fs::create_dir_all(target.parent().unwrap()).unwrap();

        let tf = TempFile::new_for(&target).unwrap();
        assert_eq!(tf.path().parent(), target.parent());
    }
}

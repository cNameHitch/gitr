use std::fs;
use std::path::PathBuf;

use git_hash::ObjectId;

use crate::{LooseError, LooseObjectStore};

/// Iterator over loose object OIDs.
///
/// Walks the fan-out directories (`00`–`ff`) under `.git/objects/` and yields
/// each valid OID found.
pub struct LooseObjectIter {
    /// Sorted list of fan-out directory paths.
    dirs: Vec<PathBuf>,
    dir_index: usize,
    /// Sorted entries in the current fan-out directory.
    current_entries: Vec<fs::DirEntry>,
    entry_index: usize,
    /// Two-char hex prefix of the current fan-out directory.
    current_prefix: String,
}

impl LooseObjectIter {
    fn new(objects_dir: &std::path::Path) -> Result<Self, LooseError> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if objects_dir.is_dir() {
            for entry in fs::read_dir(objects_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Fan-out directories are exactly 2 hex chars.
                if name_str.len() == 2
                    && name_str.chars().all(|c| c.is_ascii_hexdigit())
                    && entry.file_type()?.is_dir()
                {
                    dirs.push(entry.path());
                }
            }
        }
        dirs.sort();

        Ok(Self {
            dirs,
            dir_index: 0,
            current_entries: Vec::new(),
            entry_index: 0,
            current_prefix: String::new(),
        })
    }

    /// Load entries from the next non-empty fan-out directory.
    fn advance_dir(&mut self) -> Result<bool, LooseError> {
        while self.dir_index < self.dirs.len() {
            let dir_path = &self.dirs[self.dir_index];
            self.dir_index += 1;
            self.current_prefix = dir_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();

            let mut entries: Vec<fs::DirEntry> = Vec::new();
            for entry in fs::read_dir(dir_path)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    entries.push(entry);
                }
            }
            entries.sort_by_key(|e| e.file_name());

            if !entries.is_empty() {
                self.current_entries = entries;
                self.entry_index = 0;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Iterator for LooseObjectIter {
    type Item = Result<ObjectId, LooseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.entry_index < self.current_entries.len() {
                let entry = &self.current_entries[self.entry_index];
                self.entry_index += 1;

                let filename = entry.file_name();
                let filename_str = filename.to_string_lossy();

                // Skip non-hex filenames (temp files, etc.).
                if !filename_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    continue;
                }

                let hex = format!("{}{}", self.current_prefix, filename_str);
                match ObjectId::from_hex(&hex) {
                    Ok(oid) => return Some(Ok(oid)),
                    Err(_) => continue,
                }
            }

            // Advance to the next fan-out directory.
            match self.advance_dir() {
                Ok(true) => continue,
                Ok(false) => return None,
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

impl LooseObjectStore {
    /// Iterate over all loose object OIDs.
    pub fn iter(&self) -> Result<LooseObjectIter, LooseError> {
        LooseObjectIter::new(&self.objects_dir)
    }
}

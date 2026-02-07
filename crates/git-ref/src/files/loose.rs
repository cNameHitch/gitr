use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use bstr::ByteSlice;
use git_hash::ObjectId;
use git_utils::lockfile::LockFile;

use crate::error::RefError;
use crate::name::RefName;
use crate::Reference;

/// Read a loose ref file and return the Reference.
///
/// A loose ref is a file under `.git/refs/` (or `.git/HEAD`, etc.) containing
/// either a hex OID or `ref: <target-ref>`.
pub(crate) fn read_loose_ref(git_dir: &Path, name: &RefName) -> Result<Option<Reference>, RefError> {
    let path = loose_ref_path(git_dir, name);
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read(&path).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;

    let trimmed = contents.trim();

    if trimmed.starts_with(b"ref: ") {
        // Symbolic ref
        let target_name = &trimmed[5..];
        let target_str = std::str::from_utf8(target_name.trim())
            .map_err(|_| RefError::Parse("invalid UTF-8 in symbolic ref target".into()))?;
        let target = RefName::new(target_str)?;
        Ok(Some(Reference::Symbolic {
            name: name.clone(),
            target,
        }))
    } else {
        // Direct ref (hex OID)
        let hex = std::str::from_utf8(trimmed)
            .map_err(|_| RefError::Parse("invalid UTF-8 in ref OID".into()))?;
        let oid = ObjectId::from_hex(hex)?;
        Ok(Some(Reference::Direct {
            name: name.clone(),
            target: oid,
        }))
    }
}

/// Write a loose ref file atomically using a lock file.
pub(crate) fn write_loose_ref(
    git_dir: &Path,
    name: &RefName,
    oid: &ObjectId,
) -> Result<(), RefError> {
    let path = loose_ref_path(git_dir, name);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        // Check for directory/file conflicts
        check_dir_file_conflict(git_dir, name)?;
        fs::create_dir_all(parent).map_err(|e| RefError::IoPath {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let mut lock = LockFile::acquire(&path)?;
    let content = format!("{}\n", oid.to_hex());
    lock.write_all(content.as_bytes()).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;
    lock.commit()?;
    Ok(())
}

/// Write a symbolic ref file atomically.
pub(crate) fn write_symbolic_ref(
    git_dir: &Path,
    name: &RefName,
    target: &RefName,
) -> Result<(), RefError> {
    let path = loose_ref_path(git_dir, name);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefError::IoPath {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let mut lock = LockFile::acquire(&path)?;
    let content = format!("ref: {}\n", target);
    lock.write_all(content.as_bytes()).map_err(|e| RefError::IoPath {
        path: path.clone(),
        source: e,
    })?;
    lock.commit()?;
    Ok(())
}

/// Delete a loose ref file.
pub(crate) fn delete_loose_ref(git_dir: &Path, name: &RefName) -> Result<(), RefError> {
    let path = loose_ref_path(git_dir, name);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| RefError::IoPath {
            path: path.clone(),
            source: e,
        })?;

        // Clean up empty parent directories under refs/
        let refs_dir = git_dir.join("refs");
        let mut dir = path.parent().map(|p| p.to_path_buf());
        while let Some(d) = dir {
            if d == refs_dir || d == *git_dir {
                break;
            }
            if d.read_dir().map(|mut e| e.next().is_none()).unwrap_or(false) {
                let _ = fs::remove_dir(&d);
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }
    Ok(())
}

/// Check for directory/file conflicts when creating a ref.
///
/// For example, if `refs/heads/main` exists as a file, we cannot create
/// `refs/heads/main/sub` because `main` would need to be a directory.
/// Conversely, if `refs/heads/main/sub` exists, we cannot create `refs/heads/main`.
fn check_dir_file_conflict(git_dir: &Path, name: &RefName) -> Result<(), RefError> {
    let ref_path = loose_ref_path(git_dir, name);

    // Check if any prefix of the ref name exists as a file
    // e.g., creating refs/heads/a/b when refs/heads/a is already a file
    let mut current = git_dir.to_path_buf();
    for component in name.as_str().split('/') {
        current = current.join(component);
        if current == ref_path {
            break;
        }
        if current.is_file() {
            return Err(RefError::DirectoryConflict {
                name: name.to_string(),
                conflict: current.strip_prefix(git_dir).unwrap_or(&current).display().to_string(),
            });
        }
    }

    // Check if the ref path exists as a directory
    // e.g., creating refs/heads/a when refs/heads/a/b already exists
    if ref_path.is_dir() {
        return Err(RefError::DirectoryConflict {
            name: name.to_string(),
            conflict: format!("{} (is a directory)", ref_path.strip_prefix(git_dir).unwrap_or(&ref_path).display()),
        });
    }

    Ok(())
}

/// Enumerate all loose refs under a given prefix directory.
///
/// Returns pairs of (RefName, file path) sorted by ref name.
pub(crate) fn enumerate_loose_refs(
    git_dir: &Path,
    prefix: Option<&str>,
) -> Result<Vec<(RefName, PathBuf)>, RefError> {
    let refs_base = git_dir.join("refs");
    let search_dir = if let Some(p) = prefix {
        // Strip "refs/" prefix since we're already looking under refs/
        let sub = p.strip_prefix("refs/").unwrap_or(p);
        if sub.is_empty() {
            refs_base.clone()
        } else {
            refs_base.join(sub)
        }
    } else {
        refs_base.clone()
    };

    let mut result = Vec::new();

    if search_dir.is_dir() {
        collect_loose_refs_recursive(git_dir, &search_dir, prefix, &mut result)?;
    }

    // Also check special refs at git_dir root if no prefix or applicable prefix
    if prefix.is_none() || prefix == Some("") {
        for special in &["HEAD", "MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD",
                         "BISECT_HEAD", "ORIG_HEAD", "FETCH_HEAD", "REBASE_HEAD"] {
            let path = git_dir.join(special);
            if path.is_file() {
                if let Ok(name) = RefName::new(*special) {
                    result.push((name, path));
                }
            }
        }
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

fn collect_loose_refs_recursive(
    git_dir: &Path,
    dir: &Path,
    prefix: Option<&str>,
    result: &mut Vec<(RefName, PathBuf)>,
) -> Result<(), RefError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(RefError::IoPath {
                path: dir.to_path_buf(),
                source: e,
            })
        }
    };

    for entry in entries {
        let entry = entry.map_err(|e| RefError::IoPath {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();

        if path.is_dir() {
            collect_loose_refs_recursive(git_dir, &path, prefix, result)?;
        } else if path.is_file() {
            // Build ref name from path relative to git_dir
            let rel_path = path
                .strip_prefix(git_dir)
                .map_err(|_| RefError::Parse("cannot determine ref name from path".into()))?;

            let name_str = rel_path.to_str().ok_or_else(|| {
                RefError::Parse("non-UTF-8 ref path".into())
            })?;

            // Skip .lock files
            if name_str.ends_with(".lock") {
                continue;
            }

            if let Ok(name) = RefName::new(name_str) {
                // Apply prefix filter
                if let Some(p) = prefix {
                    if !name.as_str().starts_with(p) {
                        continue;
                    }
                }
                result.push((name, path));
            }
        }
    }

    Ok(())
}

/// Get the file system path for a loose ref.
pub(crate) fn loose_ref_path(git_dir: &Path, name: &RefName) -> PathBuf {
    git_dir.join(name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_direct_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let refs_dir = git_dir.join("refs/heads");
        fs::create_dir_all(&refs_dir).unwrap();

        let ref_file = refs_dir.join("main");
        fs::write(&ref_file, "da39a3ee5e6b4b0d3255bfef95601890afd80709\n").unwrap();

        let name = RefName::new("refs/heads/main").unwrap();
        let reference = read_loose_ref(git_dir, &name).unwrap().unwrap();

        match reference {
            Reference::Direct { target, .. } => {
                assert_eq!(
                    target,
                    ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap()
                );
            }
            _ => panic!("expected Direct reference"),
        }
    }

    #[test]
    fn read_symbolic_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let head_file = git_dir.join("HEAD");
        fs::write(&head_file, "ref: refs/heads/main\n").unwrap();

        let name = RefName::new("HEAD").unwrap();
        let reference = read_loose_ref(git_dir, &name).unwrap().unwrap();

        match reference {
            Reference::Symbolic { target, .. } => {
                assert_eq!(target.as_str(), "refs/heads/main");
            }
            _ => panic!("expected Symbolic reference"),
        }
    }

    #[test]
    fn read_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let name = RefName::new("refs/heads/nonexistent").unwrap();
        assert!(read_loose_ref(dir.path(), &name).unwrap().is_none());
    }

    #[test]
    fn write_and_read_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

        write_loose_ref(git_dir, &name, &oid).unwrap();

        let reference = read_loose_ref(git_dir, &name).unwrap().unwrap();
        match reference {
            Reference::Direct { target, .. } => assert_eq!(target, oid),
            _ => panic!("expected Direct reference"),
        }
    }

    #[test]
    fn write_and_read_symref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let name = RefName::new("HEAD").unwrap();
        let target = RefName::new("refs/heads/main").unwrap();

        write_symbolic_ref(git_dir, &name, &target).unwrap();

        let reference = read_loose_ref(git_dir, &name).unwrap().unwrap();
        match reference {
            Reference::Symbolic {
                target: found_target,
                ..
            } => {
                assert_eq!(found_target, target);
            }
            _ => panic!("expected Symbolic reference"),
        }
    }

    #[test]
    fn delete_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();
        let name = RefName::new("refs/heads/main").unwrap();
        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

        write_loose_ref(git_dir, &name, &oid).unwrap();
        assert!(read_loose_ref(git_dir, &name).unwrap().is_some());

        delete_loose_ref(git_dir, &name).unwrap();
        assert!(read_loose_ref(git_dir, &name).unwrap().is_none());
    }

    #[test]
    fn enumerate_refs() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path();

        let oid = ObjectId::from_hex("da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap();

        write_loose_ref(git_dir, &RefName::new("refs/heads/main").unwrap(), &oid).unwrap();
        write_loose_ref(git_dir, &RefName::new("refs/heads/feature").unwrap(), &oid).unwrap();
        write_loose_ref(git_dir, &RefName::new("refs/tags/v1.0").unwrap(), &oid).unwrap();

        // All refs
        let all = enumerate_loose_refs(git_dir, Some("refs/")).unwrap();
        assert_eq!(all.len(), 3);

        // Only heads
        let heads = enumerate_loose_refs(git_dir, Some("refs/heads/")).unwrap();
        assert_eq!(heads.len(), 2);

        // Only tags
        let tags = enumerate_loose_refs(git_dir, Some("refs/tags/")).unwrap();
        assert_eq!(tags.len(), 1);
    }
}

//! Alternates file parsing and recursive loading.
//!
//! The file `.git/objects/info/alternates` contains one path per line,
//! pointing to other object directories. Each alternate is itself an
//! object store that may have its own alternates file (forming a chain).
//! Circular chains are detected and rejected.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use git_hash::HashAlgorithm;

use crate::{ObjectDatabase, OdbError};

/// Maximum depth for recursive alternates loading (matches C git).
const MAX_ALTERNATES_DEPTH: usize = 5;

/// Load alternates for the given objects directory.
///
/// Returns a list of ObjectDatabase instances, one per alternate.
pub fn load_alternates(
    objects_dir: &Path,
    hash_algo: HashAlgorithm,
) -> Result<Vec<ObjectDatabase>, OdbError> {
    let mut visited = HashSet::new();
    let canonical = objects_dir
        .canonicalize()
        .unwrap_or_else(|_| objects_dir.to_path_buf());
    visited.insert(canonical);
    load_alternates_recursive(objects_dir, hash_algo, &mut visited, 0)
}

/// Recursively load alternates, tracking visited paths to detect cycles.
fn load_alternates_recursive(
    objects_dir: &Path,
    hash_algo: HashAlgorithm,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<Vec<ObjectDatabase>, OdbError> {
    if depth >= MAX_ALTERNATES_DEPTH {
        return Err(OdbError::Alternates(format!(
            "alternates chain too deep (>{MAX_ALTERNATES_DEPTH} levels)"
        )));
    }

    let alternates_path = objects_dir.join("info").join("alternates");
    if !alternates_path.is_file() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&alternates_path).map_err(|e| {
        OdbError::Alternates(format!(
            "failed to read {}: {}",
            alternates_path.display(),
            e
        ))
    })?;

    let mut result = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Resolve relative paths (relative to the objects directory)
        let alt_path = if Path::new(line).is_absolute() {
            PathBuf::from(line)
        } else {
            objects_dir.join(line)
        };

        // Skip non-existent directories (with warning behavior — just skip)
        if !alt_path.is_dir() {
            continue;
        }

        // Canonicalize for cycle detection
        let canonical = alt_path
            .canonicalize()
            .unwrap_or_else(|_| alt_path.clone());

        // Detect circular alternates
        if !visited.insert(canonical.clone()) {
            return Err(OdbError::CircularAlternates(alt_path));
        }

        // Open the alternate's loose store and packs
        let loose = git_loose::LooseObjectStore::open(&alt_path, hash_algo);
        let packs = ObjectDatabase::discover_packs(&alt_path)?;

        // Recursively load this alternate's alternates
        let nested_alternates =
            load_alternates_recursive(&alt_path, hash_algo, visited, depth + 1)?;

        result.push(ObjectDatabase {
            loose,
            packs: std::sync::RwLock::new(packs),
            alternates: nested_alternates,
            cache: std::sync::Mutex::new(git_object::cache::ObjectCache::new(256)),
            objects_dir: alt_path,
            hash_algo,
        });
    }

    Ok(result)
}

/// Parse an alternates file and return the raw paths (for testing).
pub fn parse_alternates_file(path: &Path) -> Result<Vec<PathBuf>, OdbError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|e| {
        OdbError::Alternates(format!("failed to read {}: {}", path.display(), e))
    })?;

    let base_dir = path.parent().and_then(|p| p.parent()).unwrap_or(path);

    Ok(content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            if Path::new(l).is_absolute() {
                PathBuf::from(l)
            } else {
                base_dir.join(l)
            }
        })
        .collect())
}

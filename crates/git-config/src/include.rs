//! Include and conditional include processing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bstr::{BStr, BString, ByteSlice};

use crate::error::ConfigError;
use crate::file::ConfigFile;
use crate::parse::ConfigEvent;
use crate::types;

/// Maximum include depth to prevent circular includes.
const MAX_INCLUDE_DEPTH: usize = 10;

/// Process include directives in a config set.
///
/// This scans all loaded config files for `include.path` and `includeIf.*.path`
/// directives and loads the referenced files.
pub fn process_includes(
    files: &mut Vec<ConfigFile>,
    git_dir: Option<&Path>,
    current_branch: Option<&str>,
) -> Result<(), ConfigError> {
    let mut include_stack: HashSet<PathBuf> = HashSet::new();
    let mut depth: usize = 0;

    // Process each file's includes.
    // We iterate with an index and newly-inserted files are placed right
    // after the current file, so subsequent iterations will process them
    // (enabling chained includes A -> B -> C).
    let mut i = 0;
    while i < files.len() {
        let includes = collect_includes(&files[i]);
        let file_dir = files[i]
            .path()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        let file_scope = files[i].scope();

        // Track this file in the include stack (use canonical path for
        // reliable circular-include detection on macOS /var -> /private/var).
        if let Some(path) = files[i].path() {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            include_stack.insert(canonical);
        }

        let mut insert_offset = 1;
        for (condition, include_path) in includes {
            if depth >= MAX_INCLUDE_DEPTH {
                return Err(ConfigError::IncludeDepthExceeded(MAX_INCLUDE_DEPTH));
            }

            // Resolve relative paths
            let resolved = resolve_include_path(&include_path, file_dir.as_deref())?;

            // Check condition
            if let Some(cond) = condition {
                if !evaluate_condition(&cond, &resolved, git_dir, current_branch, files)? {
                    continue;
                }
            }

            // Check for circular include
            let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
            if include_stack.contains(&canonical) {
                return Err(ConfigError::CircularInclude(
                    canonical.display().to_string(),
                ));
            }

            // Load the included file
            if resolved.exists() {
                match ConfigFile::load(&resolved, file_scope) {
                    Ok(included) => {
                        depth += 1;
                        files.insert(i + insert_offset, included);
                        insert_offset += 1;
                    }
                    Err(ConfigError::FileNotFound(_)) => {
                        // Silently ignore missing include files
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // Advance past just the current file. Newly-inserted included
        // files sit at i+1..i+insert_offset and will be processed on
        // subsequent iterations, enabling chained includes.
        i += 1;
    }

    Ok(())
}

/// Collect include directives from a config file.
/// Returns (condition, path) tuples. Condition is None for unconditional includes.
fn collect_includes(file: &ConfigFile) -> Vec<(Option<String>, String)> {
    let mut includes = Vec::new();
    let mut current_section = BString::new(Vec::new());
    let mut current_subsection: Option<BString> = None;

    for event in file.events() {
        match event {
            ConfigEvent::SectionHeader {
                section,
                subsection,
                ..
            } => {
                current_section = section.clone();
                current_subsection = subsection.clone();
            }
            ConfigEvent::Entry { key, value, .. } => {
                let key_lower = key.to_str_lossy().to_ascii_lowercase();

                if current_section.to_str_lossy() == "include" && key_lower == "path" {
                    if let Some(ref val) = value {
                        includes.push((None, val.to_str_lossy().to_string()));
                    }
                } else if current_section.to_str_lossy() == "includeif" && key_lower == "path" {
                    if let Some(ref sub) = current_subsection {
                        if let Some(ref val) = value {
                            includes.push((
                                Some(sub.to_str_lossy().to_string()),
                                val.to_str_lossy().to_string(),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    includes
}

/// Resolve an include path, expanding ~ and resolving relative paths.
fn resolve_include_path(path: &str, base_dir: Option<&Path>) -> Result<PathBuf, ConfigError> {
    let expanded = if path.starts_with("~/") || path == "~" {
        types::parse_path(BStr::new(path.as_bytes()))?
    } else if !Path::new(path).is_absolute() {
        // Relative to the including file's directory
        if let Some(base) = base_dir {
            base.join(path)
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    };

    Ok(expanded)
}

/// Evaluate an includeIf condition.
fn evaluate_condition(
    condition: &str,
    _include_path: &Path,
    git_dir: Option<&Path>,
    current_branch: Option<&str>,
    files: &[ConfigFile],
) -> Result<bool, ConfigError> {
    if let Some(pattern) = condition.strip_prefix("gitdir:") {
        return evaluate_gitdir(pattern, git_dir, false);
    }
    if let Some(pattern) = condition.strip_prefix("gitdir/i:") {
        return evaluate_gitdir(pattern, git_dir, true);
    }
    if let Some(pattern) = condition.strip_prefix("onbranch:") {
        return evaluate_onbranch(pattern, current_branch);
    }
    if let Some(rest) = condition.strip_prefix("hasconfig:remote.*.url:") {
        return evaluate_hasconfig_remote_url(rest, files);
    }

    // Unknown condition — don't include
    Ok(false)
}

/// Evaluate a gitdir condition.
fn evaluate_gitdir(
    pattern: &str,
    git_dir: Option<&Path>,
    case_insensitive: bool,
) -> Result<bool, ConfigError> {
    let git_dir = match git_dir {
        Some(d) => d,
        None => return Ok(false),
    };

    let git_dir_str = git_dir.to_str().unwrap_or("");

    // Expand ~ in pattern
    let expanded_pattern = if pattern.starts_with("~/") || pattern == "~" {
        let expanded = types::parse_path(BStr::new(pattern.as_bytes()))?;
        expanded.to_str().unwrap_or(pattern).to_string()
    } else {
        pattern.to_string()
    };

    // Ensure trailing slash for directory matching
    let pattern_with_slash = if expanded_pattern.ends_with('/') {
        format!("{}**", expanded_pattern)
    } else {
        format!("{}/**", expanded_pattern)
    };

    // Use wildmatch for glob matching
    let flags = if case_insensitive {
        git_utils::wildmatch::WildmatchFlags::CASEFOLD
    } else {
        git_utils::wildmatch::WildmatchFlags::empty()
    };

    let git_dir_with_slash = if git_dir_str.ends_with('/') {
        git_dir_str.to_string()
    } else {
        format!("{}/", git_dir_str)
    };

    Ok(git_utils::wildmatch::wildmatch(
        BStr::new(pattern_with_slash.as_bytes()),
        BStr::new(git_dir_with_slash.as_bytes()),
        flags,
    ))
}

/// Evaluate an onbranch condition.
fn evaluate_onbranch(
    pattern: &str,
    current_branch: Option<&str>,
) -> Result<bool, ConfigError> {
    let branch = match current_branch {
        Some(b) => b,
        None => return Ok(false),
    };

    // Strip refs/heads/ prefix if present
    let short_branch = branch
        .strip_prefix("refs/heads/")
        .unwrap_or(branch);

    let flags = git_utils::wildmatch::WildmatchFlags::empty();

    Ok(git_utils::wildmatch::wildmatch(
        BStr::new(pattern.as_bytes()),
        BStr::new(short_branch.as_bytes()),
        flags,
    ))
}

/// Evaluate a hasconfig:remote.*.url condition.
fn evaluate_hasconfig_remote_url(
    url_pattern: &str,
    files: &[ConfigFile],
) -> Result<bool, ConfigError> {
    let flags = git_utils::wildmatch::WildmatchFlags::empty();

    for file in files {
        for entry in file.entries() {
            if entry.key.section.to_str_lossy() == "remote"
                && entry.key.name.to_str_lossy() == "url"
            {
                if let Some(ref val) = entry.value {
                    let url = val.to_str_lossy();
                    if git_utils::wildmatch::wildmatch(
                        BStr::new(url_pattern.as_bytes()),
                        BStr::new(url.as_bytes()),
                        flags,
                    ) {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigScope;

    #[test]
    fn resolve_absolute_path() {
        let result = resolve_include_path("/etc/gitconfig", None).unwrap();
        assert_eq!(result, PathBuf::from("/etc/gitconfig"));
    }

    #[test]
    fn resolve_relative_path() {
        let result =
            resolve_include_path("extra.config", Some(Path::new("/home/user"))).unwrap();
        assert_eq!(result, PathBuf::from("/home/user/extra.config"));
    }

    #[test]
    fn resolve_tilde_path() {
        let result = resolve_include_path("~/extra.config", None).unwrap();
        assert!(!result.to_str().unwrap().starts_with("~"));
    }

    #[test]
    fn evaluate_onbranch_match() {
        assert!(evaluate_onbranch("main", Some("refs/heads/main")).unwrap());
    }

    #[test]
    fn evaluate_onbranch_no_match() {
        assert!(!evaluate_onbranch("main", Some("refs/heads/develop")).unwrap());
    }

    #[test]
    fn evaluate_onbranch_wildcard() {
        assert!(evaluate_onbranch("feature/*", Some("refs/heads/feature/foo")).unwrap());
    }

    #[test]
    fn evaluate_onbranch_no_branch() {
        assert!(!evaluate_onbranch("main", None).unwrap());
    }

    #[test]
    fn max_depth_constant() {
        assert_eq!(MAX_INCLUDE_DEPTH, 10);
    }

    #[test]
    fn collect_includes_from_file() {
        let input = b"[include]\n\tpath = extra.config\n[includeIf \"gitdir:~/work/\"]\n\tpath = work.config\n";
        let file = ConfigFile::parse(input, None, ConfigScope::Local).unwrap();
        let includes = collect_includes(&file);
        assert_eq!(includes.len(), 2);
        assert_eq!(includes[0].0, None);
        assert_eq!(includes[0].1, "extra.config");
        assert_eq!(
            includes[1].0,
            Some("gitdir:~/work/".to_string())
        );
        assert_eq!(includes[1].1, "work.config");
    }
}

//! Remote configuration and refspec parsing.
//!
//! Parses `remote.<name>.url`, `remote.<name>.fetch`, and `remote.<name>.push`
//! from git configuration.

use bstr::BString;
use git_config::ConfigSet;

use crate::ProtocolError;

/// A refspec for fetch or push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSpec {
    /// Source pattern (left side of the colon).
    pub source: String,
    /// Destination pattern (right side of the colon).
    pub destination: String,
    /// Force update (prefixed with +).
    pub force: bool,
}

impl RefSpec {
    /// Parse a refspec string like `+refs/heads/*:refs/remotes/origin/*`.
    pub fn parse(spec: &str) -> Result<Self, ProtocolError> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err(ProtocolError::InvalidRefSpec("empty refspec".into()));
        }

        let (force, rest) = if let Some(s) = spec.strip_prefix('+') {
            (true, s)
        } else {
            (false, spec)
        };

        if let Some(colon_pos) = rest.find(':') {
            let source = &rest[..colon_pos];
            let destination = &rest[colon_pos + 1..];
            Ok(RefSpec {
                source: source.to_string(),
                destination: destination.to_string(),
                force,
            })
        } else {
            // No colon — source only (e.g., "refs/heads/main")
            Ok(RefSpec {
                source: rest.to_string(),
                destination: String::new(),
                force,
            })
        }
    }

    /// Check if a ref name matches this refspec's source pattern.
    pub fn matches_source(&self, refname: &str) -> bool {
        pattern_matches(&self.source, refname)
    }

    /// Map a source ref name to its destination using this refspec.
    ///
    /// Returns None if the source doesn't match.
    pub fn map_to_destination(&self, source_ref: &str) -> Option<String> {
        if self.destination.is_empty() {
            return None;
        }

        if let Some(star_pos) = self.source.find('*') {
            let prefix = &self.source[..star_pos];
            let suffix = &self.source[star_pos + 1..];

            if source_ref.starts_with(prefix) && source_ref.ends_with(suffix) {
                let matched = &source_ref[prefix.len()..source_ref.len() - suffix.len()];

                if let Some(dest_star) = self.destination.find('*') {
                    let dest_prefix = &self.destination[..dest_star];
                    let dest_suffix = &self.destination[dest_star + 1..];
                    return Some(format!("{}{}{}", dest_prefix, matched, dest_suffix));
                }
            }
            None
        } else if self.source == source_ref {
            Some(self.destination.clone())
        } else {
            None
        }
    }
}

/// Check if a pattern (with optional `*` wildcard) matches a string.
fn pattern_matches(pattern: &str, value: &str) -> bool {
    if let Some(star_pos) = pattern.find('*') {
        let prefix = &pattern[..star_pos];
        let suffix = &pattern[star_pos + 1..];
        value.starts_with(prefix) && value.ends_with(suffix)
    } else {
        pattern == value
    }
}

/// Parsed remote configuration.
#[derive(Debug, Clone)]
pub struct RemoteConfig {
    /// Remote name (e.g., "origin").
    pub name: String,
    /// Remote URL.
    pub url: String,
    /// Push URL (if different from url).
    pub push_url: Option<String>,
    /// Fetch refspecs.
    pub fetch_refspecs: Vec<RefSpec>,
    /// Push refspecs.
    pub push_refspecs: Vec<RefSpec>,
}

impl RemoteConfig {
    /// Load remote configuration from a ConfigSet.
    pub fn from_config(config: &ConfigSet, name: &str) -> Result<Option<Self>, ProtocolError> {
        let url_key = format!("remote.{}.url", name);
        let url = match config.get_string(&url_key)? {
            Some(url) => url,
            None => return Ok(None),
        };

        let push_url_key = format!("remote.{}.pushurl", name);
        let push_url = config.get_string(&push_url_key)?;

        let fetch_key = format!("remote.{}.fetch", name);
        let fetch_specs: Vec<RefSpec> = config
            .get_all_strings(&fetch_key)?
            .iter()
            .filter_map(|s| RefSpec::parse(s).ok())
            .collect();

        let push_key = format!("remote.{}.push", name);
        let push_specs: Vec<RefSpec> = config
            .get_all_strings(&push_key)?
            .iter()
            .filter_map(|s| RefSpec::parse(s).ok())
            .collect();

        Ok(Some(RemoteConfig {
            name: name.to_string(),
            url,
            push_url,
            fetch_refspecs: fetch_specs,
            push_refspecs: push_specs,
        }))
    }

    /// Get the URL to use for push (push_url if set, otherwise url).
    pub fn push_url(&self) -> &str {
        self.push_url.as_deref().unwrap_or(&self.url)
    }
}

/// Apply refspecs to map remote refs to local tracking refs.
pub fn map_refs(
    refs: &[(git_hash::ObjectId, BString)],
    refspecs: &[RefSpec],
) -> Vec<(git_hash::ObjectId, String, String)> {
    let mut result = Vec::new();

    for (oid, remote_ref) in refs {
        let remote_name = String::from_utf8_lossy(remote_ref.as_ref()).to_string();
        for spec in refspecs {
            if let Some(local_ref) = spec.map_to_destination(&remote_name) {
                result.push((*oid, remote_name.clone(), local_ref));
                break;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_refspec() {
        let spec = RefSpec::parse("refs/heads/main:refs/remotes/origin/main").unwrap();
        assert_eq!(spec.source, "refs/heads/main");
        assert_eq!(spec.destination, "refs/remotes/origin/main");
        assert!(!spec.force);
    }

    #[test]
    fn parse_force_refspec() {
        let spec = RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap();
        assert_eq!(spec.source, "refs/heads/*");
        assert_eq!(spec.destination, "refs/remotes/origin/*");
        assert!(spec.force);
    }

    #[test]
    fn parse_source_only_refspec() {
        let spec = RefSpec::parse("refs/heads/main").unwrap();
        assert_eq!(spec.source, "refs/heads/main");
        assert!(spec.destination.is_empty());
        assert!(!spec.force);
    }

    #[test]
    fn parse_empty_refspec_fails() {
        assert!(RefSpec::parse("").is_err());
    }

    #[test]
    fn refspec_matches_wildcard() {
        let spec = RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap();
        assert!(spec.matches_source("refs/heads/main"));
        assert!(spec.matches_source("refs/heads/feature/foo"));
        assert!(!spec.matches_source("refs/tags/v1.0"));
    }

    #[test]
    fn refspec_matches_exact() {
        let spec = RefSpec::parse("refs/heads/main:refs/remotes/origin/main").unwrap();
        assert!(spec.matches_source("refs/heads/main"));
        assert!(!spec.matches_source("refs/heads/develop"));
    }

    #[test]
    fn refspec_map_wildcard() {
        let spec = RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap();
        assert_eq!(
            spec.map_to_destination("refs/heads/main"),
            Some("refs/remotes/origin/main".to_string())
        );
        assert_eq!(
            spec.map_to_destination("refs/heads/feature/foo"),
            Some("refs/remotes/origin/feature/foo".to_string())
        );
        assert_eq!(spec.map_to_destination("refs/tags/v1.0"), None);
    }

    #[test]
    fn refspec_map_exact() {
        let spec = RefSpec::parse("refs/heads/main:refs/remotes/origin/main").unwrap();
        assert_eq!(
            spec.map_to_destination("refs/heads/main"),
            Some("refs/remotes/origin/main".to_string())
        );
        assert_eq!(spec.map_to_destination("refs/heads/develop"), None);
    }

    #[test]
    fn map_refs_with_refspecs() {
        use git_hash::ObjectId;

        let oid = ObjectId::NULL_SHA1;
        let refs = vec![
            (oid, BString::from("refs/heads/main")),
            (oid, BString::from("refs/heads/feature")),
            (oid, BString::from("refs/tags/v1.0")),
        ];
        let specs = vec![
            RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap(),
        ];

        let mapped = map_refs(&refs, &specs);
        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].2, "refs/remotes/origin/main");
        assert_eq!(mapped[1].2, "refs/remotes/origin/feature");
    }
}

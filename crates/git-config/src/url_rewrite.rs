//! URL rewriting from url.<base>.insteadOf and url.<base>.pushInsteadOf.

use bstr::ByteSlice;

use crate::ConfigEntry;

/// URL rewriting rules from config.
#[derive(Debug, Clone)]
pub struct UrlRewrite {
    /// The replacement base URL.
    pub base: String,
    /// Prefixes to match for both fetch and push (insteadOf).
    pub instead_of: Vec<String>,
    /// Prefixes to match for push only (pushInsteadOf).
    pub push_instead_of: Vec<String>,
}

/// Collect URL rewrite rules from config entries.
pub fn collect_url_rewrites(entries: &[ConfigEntry]) -> Vec<UrlRewrite> {
    use std::collections::HashMap;

    // Group by base URL (the subsection of url.<base>)
    let mut rewrites: HashMap<String, UrlRewrite> = HashMap::new();

    for entry in entries {
        if entry.key.section.to_str_lossy() != "url" {
            continue;
        }
        let base = match &entry.key.subsection {
            Some(sub) => sub.to_str_lossy().to_string(),
            None => continue,
        };
        let name = entry.key.name.to_str_lossy().to_ascii_lowercase();

        let rewrite = rewrites.entry(base.clone()).or_insert_with(|| UrlRewrite {
            base: base.clone(),
            instead_of: Vec::new(),
            push_instead_of: Vec::new(),
        });

        if let Some(ref val) = entry.value {
            let val_str = val.to_str_lossy().to_string();
            match name.as_str() {
                "insteadof" => rewrite.instead_of.push(val_str),
                "pushinsteadof" => rewrite.push_instead_of.push(val_str),
                _ => {}
            }
        }
    }

    rewrites.into_values().collect()
}

/// Resolve URL rewrites for a given URL.
///
/// For push operations, `pushInsteadOf` rules are checked first, then `insteadOf`.
/// For fetch operations, only `insteadOf` rules apply.
///
/// The longest matching prefix wins.
pub fn rewrite_url(url: &str, rewrites: &[UrlRewrite], is_push: bool) -> String {
    let mut best_match: Option<(&str, &str, usize)> = None; // (prefix, base, prefix_len)

    for rewrite in rewrites {
        if is_push {
            // Check pushInsteadOf first for push operations
            for prefix in &rewrite.push_instead_of {
                if url.starts_with(prefix.as_str()) {
                    let prefix_len = prefix.len();
                    if best_match.map_or(true, |(_, _, best_len)| prefix_len > best_len) {
                        best_match = Some((prefix.as_str(), &rewrite.base, prefix_len));
                    }
                }
            }
        }

        // Check insteadOf
        if best_match.is_none() || !is_push {
            for prefix in &rewrite.instead_of {
                if url.starts_with(prefix.as_str()) {
                    let prefix_len = prefix.len();
                    if best_match.map_or(true, |(_, _, best_len)| prefix_len > best_len) {
                        best_match = Some((prefix.as_str(), &rewrite.base, prefix_len));
                    }
                }
            }
        }
    }

    match best_match {
        Some((prefix, base, _)) => {
            format!("{}{}", base, &url[prefix.len()..])
        }
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_instead_of() {
        let rewrites = vec![UrlRewrite {
            base: "https://github.com/".to_string(),
            instead_of: vec!["gh:".to_string()],
            push_instead_of: Vec::new(),
        }];

        assert_eq!(
            rewrite_url("gh:user/repo", &rewrites, false),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn rewrite_push_instead_of() {
        let rewrites = vec![UrlRewrite {
            base: "git@github.com:".to_string(),
            instead_of: Vec::new(),
            push_instead_of: vec!["https://github.com/".to_string()],
        }];

        // For push, pushInsteadOf should apply
        assert_eq!(
            rewrite_url("https://github.com/user/repo", &rewrites, true),
            "git@github.com:user/repo"
        );

        // For fetch, pushInsteadOf should NOT apply
        assert_eq!(
            rewrite_url("https://github.com/user/repo", &rewrites, false),
            "https://github.com/user/repo"
        );
    }

    #[test]
    fn rewrite_longest_prefix_wins() {
        let rewrites = vec![
            UrlRewrite {
                base: "https://github.com/".to_string(),
                instead_of: vec!["gh:".to_string()],
                push_instead_of: Vec::new(),
            },
            UrlRewrite {
                base: "git@github.com:myorg/".to_string(),
                instead_of: vec!["gh:myorg/".to_string()],
                push_instead_of: Vec::new(),
            },
        ];

        // Longer prefix should match
        assert_eq!(
            rewrite_url("gh:myorg/repo", &rewrites, false),
            "git@github.com:myorg/repo"
        );

        // Shorter prefix should match for other URLs
        assert_eq!(
            rewrite_url("gh:other/repo", &rewrites, false),
            "https://github.com/other/repo"
        );
    }

    #[test]
    fn rewrite_no_match() {
        let rewrites = vec![UrlRewrite {
            base: "https://github.com/".to_string(),
            instead_of: vec!["gh:".to_string()],
            push_instead_of: Vec::new(),
        }];

        assert_eq!(
            rewrite_url("https://gitlab.com/repo", &rewrites, false),
            "https://gitlab.com/repo"
        );
    }

    #[test]
    fn rewrite_empty_rewrites() {
        let rewrites: Vec<UrlRewrite> = Vec::new();
        assert_eq!(
            rewrite_url("https://github.com/repo", &rewrites, false),
            "https://github.com/repo"
        );
    }
}

use std::fmt;

use bstr::{BStr, BString, ByteSlice};

use crate::error::RefError;

/// A validated reference name.
///
/// Enforces all rules from `git-check-ref-format(1)`:
/// - No double dots `..`
/// - No ASCII control characters or space, `~`, `^`, `:`, `?`, `*`, `[`, `\`
/// - Cannot begin or end with `/`, or contain `//`
/// - Cannot end with `.`
/// - Cannot end with `.lock`
/// - Cannot contain `@{`
/// - Cannot be the single character `@`
/// - Cannot contain a NUL byte
/// - Must have at least one `/` (for full refs like `refs/heads/main`),
///   unless it is a special ref like `HEAD`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefName(BString);

/// Characters forbidden anywhere in a ref name.
const FORBIDDEN_CHARS: &[u8] = b" ~^:?*[\\";

/// Special ref names that are valid without a `/`.
const SPECIAL_REFS: &[&str] = &[
    "HEAD",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "BISECT_HEAD",
    "ORIG_HEAD",
    "FETCH_HEAD",
    "AUTO_MERGE",
    "REBASE_HEAD",
];

impl RefName {
    /// Create and validate a ref name according to git-check-ref-format rules.
    pub fn new(name: impl Into<BString>) -> Result<Self, RefError> {
        let name = name.into();
        validate_ref_name(&name)?;
        Ok(Self(name))
    }

    /// Create without validation (for internal use with known-good names).
    pub(crate) fn new_unchecked(name: impl Into<BString>) -> Self {
        Self(name.into())
    }

    /// Get the short name (e.g., `main` from `refs/heads/main`).
    pub fn short_name(&self) -> &BStr {
        let s = self.0.as_bstr();
        if let Some(rest) = s.strip_prefix(b"refs/heads/") {
            rest.as_bstr()
        } else if let Some(rest) = s.strip_prefix(b"refs/tags/") {
            rest.as_bstr()
        } else if let Some(rest) = s.strip_prefix(b"refs/remotes/") {
            rest.as_bstr()
        } else {
            s
        }
    }

    /// Is this under `refs/heads/`?
    pub fn is_branch(&self) -> bool {
        self.0.starts_with(b"refs/heads/")
    }

    /// Is this under `refs/tags/`?
    pub fn is_tag(&self) -> bool {
        self.0.starts_with(b"refs/tags/")
    }

    /// Is this under `refs/remotes/`?
    pub fn is_remote(&self) -> bool {
        self.0.starts_with(b"refs/remotes/")
    }

    /// Is this a special ref (HEAD, MERGE_HEAD, etc.)?
    pub fn is_special(&self) -> bool {
        let s = self.0.to_str_lossy();
        SPECIAL_REFS.contains(&s.as_ref())
    }

    /// Get the raw bytes of this ref name.
    pub fn as_bstr(&self) -> &BStr {
        self.0.as_bstr()
    }

    /// Get as a string slice (ref names are always valid UTF-8 in practice).
    pub fn as_str(&self) -> &str {
        // Ref names validated by git are always ASCII/UTF-8
        std::str::from_utf8(&self.0).unwrap_or("<invalid-utf8>")
    }

    /// Get the inner BString.
    pub fn into_inner(self) -> BString {
        self.0
    }
}

impl AsRef<BStr> for RefName {
    fn as_ref(&self) -> &BStr {
        self.0.as_bstr()
    }
}

impl fmt::Display for RefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Validate a ref name per git-check-ref-format rules.
fn validate_ref_name(name: &[u8]) -> Result<(), RefError> {
    let name_str = || String::from_utf8_lossy(name).into_owned();

    if name.is_empty() {
        return Err(RefError::InvalidName("ref name is empty".into()));
    }

    // Cannot contain NUL
    if name.contains(&0) {
        return Err(RefError::InvalidName(format!(
            "'{}': contains NUL byte",
            name_str()
        )));
    }

    // Cannot be exactly "@"
    if name == b"@" {
        return Err(RefError::InvalidName("'@' is not a valid ref name".into()));
    }

    // Check for forbidden characters and control characters
    for (i, &b) in name.iter().enumerate() {
        if b < 0x20 || b == 0x7f {
            return Err(RefError::InvalidName(format!(
                "'{}': contains control character at position {}",
                name_str(),
                i
            )));
        }
        if FORBIDDEN_CHARS.contains(&b) {
            return Err(RefError::InvalidName(format!(
                "'{}': contains forbidden character '{}' at position {}",
                name_str(),
                b as char,
                i
            )));
        }
    }

    // Cannot start with '.'
    if name.starts_with(b".") {
        return Err(RefError::InvalidName(format!(
            "'{}': starts with '.'",
            name_str()
        )));
    }

    // Cannot end with '/'
    if name.ends_with(b"/") {
        return Err(RefError::InvalidName(format!(
            "'{}': ends with '/'",
            name_str()
        )));
    }

    // Cannot start with '/'
    if name.starts_with(b"/") {
        return Err(RefError::InvalidName(format!(
            "'{}': starts with '/'",
            name_str()
        )));
    }

    // Cannot end with '.'
    if name.ends_with(b".") {
        return Err(RefError::InvalidName(format!(
            "'{}': ends with '.'",
            name_str()
        )));
    }

    // Cannot end with ".lock"
    if name.ends_with(b".lock") {
        return Err(RefError::InvalidName(format!(
            "'{}': ends with '.lock'",
            name_str()
        )));
    }

    // Cannot contain ".."
    if name.find(b"..").is_some() {
        return Err(RefError::InvalidName(format!(
            "'{}': contains '..'",
            name_str()
        )));
    }

    // Cannot contain "//"
    if name.find(b"//").is_some() {
        return Err(RefError::InvalidName(format!(
            "'{}': contains '//'",
            name_str()
        )));
    }

    // Cannot contain "@{"
    if name.find(b"@{").is_some() {
        return Err(RefError::InvalidName(format!(
            "'{}': contains '@{{'",
            name_str()
        )));
    }

    // Check that each component doesn't start with '.'
    for component in name.split_str(b"/") {
        if component.starts_with(b".") {
            return Err(RefError::InvalidName(format!(
                "'{}': component starts with '.'",
                name_str()
            )));
        }
        if component.ends_with(b".lock") {
            return Err(RefError::InvalidName(format!(
                "'{}': component ends with '.lock'",
                name_str()
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ref_names() {
        assert!(RefName::new("refs/heads/main").is_ok());
        assert!(RefName::new("refs/tags/v1.0").is_ok());
        assert!(RefName::new("refs/remotes/origin/main").is_ok());
        assert!(RefName::new("HEAD").is_ok());
        assert!(RefName::new("MERGE_HEAD").is_ok());
        assert!(RefName::new("refs/heads/feature/sub-branch").is_ok());
        assert!(RefName::new("refs/heads/a").is_ok());
    }

    #[test]
    fn invalid_double_dot() {
        assert!(RefName::new("refs/heads/main..branch").is_err());
    }

    #[test]
    fn invalid_control_char() {
        assert!(RefName::new(b"refs/heads/\x01bad".to_vec()).is_err());
    }

    #[test]
    fn invalid_space() {
        assert!(RefName::new("refs/heads/bad name").is_err());
    }

    #[test]
    fn invalid_tilde() {
        assert!(RefName::new("refs/heads/bad~name").is_err());
    }

    #[test]
    fn invalid_caret() {
        assert!(RefName::new("refs/heads/bad^name").is_err());
    }

    #[test]
    fn invalid_colon() {
        assert!(RefName::new("refs/heads/bad:name").is_err());
    }

    #[test]
    fn invalid_question() {
        assert!(RefName::new("refs/heads/bad?name").is_err());
    }

    #[test]
    fn invalid_star() {
        assert!(RefName::new("refs/heads/bad*name").is_err());
    }

    #[test]
    fn invalid_bracket() {
        assert!(RefName::new("refs/heads/bad[name").is_err());
    }

    #[test]
    fn invalid_backslash() {
        assert!(RefName::new("refs/heads/bad\\name").is_err());
    }

    #[test]
    fn invalid_starts_with_dot() {
        assert!(RefName::new(".refs/heads/main").is_err());
    }

    #[test]
    fn invalid_component_starts_with_dot() {
        assert!(RefName::new("refs/heads/.hidden").is_err());
    }

    #[test]
    fn invalid_ends_with_slash() {
        assert!(RefName::new("refs/heads/main/").is_err());
    }

    #[test]
    fn invalid_starts_with_slash() {
        assert!(RefName::new("/refs/heads/main").is_err());
    }

    #[test]
    fn invalid_ends_with_dot() {
        assert!(RefName::new("refs/heads/main.").is_err());
    }

    #[test]
    fn invalid_ends_with_lock() {
        assert!(RefName::new("refs/heads/main.lock").is_err());
    }

    #[test]
    fn invalid_component_ends_with_lock() {
        assert!(RefName::new("refs/heads/bad.lock/sub").is_err());
    }

    #[test]
    fn invalid_double_slash() {
        assert!(RefName::new("refs//heads/main").is_err());
    }

    #[test]
    fn invalid_at_brace() {
        assert!(RefName::new("refs/heads/main@{0}").is_err());
    }

    #[test]
    fn invalid_single_at() {
        assert!(RefName::new("@").is_err());
    }

    #[test]
    fn invalid_empty() {
        assert!(RefName::new("").is_err());
    }

    #[test]
    fn short_name_branch() {
        let r = RefName::new("refs/heads/main").unwrap();
        assert_eq!(r.short_name(), "main");
    }

    #[test]
    fn short_name_tag() {
        let r = RefName::new("refs/tags/v1.0").unwrap();
        assert_eq!(r.short_name(), "v1.0");
    }

    #[test]
    fn short_name_remote() {
        let r = RefName::new("refs/remotes/origin/main").unwrap();
        assert_eq!(r.short_name(), "origin/main");
    }

    #[test]
    fn short_name_head() {
        let r = RefName::new("HEAD").unwrap();
        assert_eq!(r.short_name(), "HEAD");
    }

    #[test]
    fn is_branch() {
        assert!(RefName::new("refs/heads/main").unwrap().is_branch());
        assert!(!RefName::new("refs/tags/v1.0").unwrap().is_branch());
    }

    #[test]
    fn is_tag() {
        assert!(RefName::new("refs/tags/v1.0").unwrap().is_tag());
        assert!(!RefName::new("refs/heads/main").unwrap().is_tag());
    }

    #[test]
    fn is_remote() {
        assert!(RefName::new("refs/remotes/origin/main").unwrap().is_remote());
        assert!(!RefName::new("refs/heads/main").unwrap().is_remote());
    }

    #[test]
    fn is_special() {
        assert!(RefName::new("HEAD").unwrap().is_special());
        assert!(RefName::new("MERGE_HEAD").unwrap().is_special());
        assert!(!RefName::new("refs/heads/main").unwrap().is_special());
    }

    #[test]
    fn display() {
        let r = RefName::new("refs/heads/main").unwrap();
        assert_eq!(r.to_string(), "refs/heads/main");
    }

    #[test]
    fn ordering() {
        let a = RefName::new("refs/heads/alpha").unwrap();
        let b = RefName::new("refs/heads/beta").unwrap();
        assert!(a < b);
    }
}

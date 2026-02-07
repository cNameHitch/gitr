use bstr::{BStr, BString, ByteSlice, ByteVec};

use crate::error::UtilError;
use crate::Result;

/// A git-normalized path (always forward slashes, no trailing slash unless root).
///
/// Git internally represents paths with forward slashes regardless of platform.
/// This type enforces that invariant and provides path manipulation matching
/// C git's behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GitPath(BString);

/// Check if a byte is a directory separator (handles both Unix and Windows).
#[inline]
fn is_dir_sep(c: u8) -> bool {
    c == b'/' || c == b'\\'
}

impl GitPath {
    /// Create from a byte slice, normalizing path separators to forward slashes
    /// and removing trailing slashes (unless the path is just "/").
    pub fn new(path: impl AsRef<[u8]>) -> Self {
        let path = path.as_ref();
        let mut normalized = BString::new(Vec::with_capacity(path.len()));

        for &b in path {
            if is_dir_sep(b) {
                normalized.push_byte(b'/');
            } else {
                normalized.push_byte(b);
            }
        }

        // Remove trailing slashes (but keep a lone "/")
        while normalized.len() > 1 && normalized.last() == Some(&b'/') {
            normalized.pop();
        }

        GitPath(normalized)
    }

    /// Create from an already-normalized byte string (no validation).
    pub fn from_normalized(path: BString) -> Self {
        GitPath(path)
    }

    /// Get the raw bytes of this path.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Get this path as a `BStr`.
    pub fn as_bstr(&self) -> &BStr {
        self.0.as_bstr()
    }

    /// Join two paths with '/'.
    pub fn join(&self, other: impl AsRef<[u8]>) -> GitPath {
        let other = other.as_ref();
        if other.is_empty() {
            return self.clone();
        }
        // If other is absolute, it replaces self
        if !other.is_empty() && is_dir_sep(other[0]) {
            return GitPath::new(other);
        }
        if self.0.is_empty() {
            return GitPath::new(other);
        }

        let mut result = self.0.clone();
        if result.last() != Some(&b'/') {
            result.push_byte(b'/');
        }
        for &b in other {
            if is_dir_sep(b) {
                result.push_byte(b'/');
            } else {
                result.push_byte(b);
            }
        }
        // Remove trailing slash
        while result.len() > 1 && result.last() == Some(&b'/') {
            result.pop();
        }
        GitPath(result)
    }

    /// Get the directory portion (like dirname).
    /// Returns "." if there's no directory component.
    pub fn dirname(&self) -> &BStr {
        let bytes = self.0.as_bytes();
        if bytes.is_empty() {
            return BStr::new(b".");
        }

        // Find the last '/'
        match bytes.iter().rposition(|&b| b == b'/') {
            Some(0) => BStr::new(b"/"),
            Some(pos) => BStr::new(&bytes[..pos]),
            None => BStr::new(b"."),
        }
    }

    /// Get the filename portion (like basename).
    /// Returns the whole path if there's no directory separator.
    pub fn basename(&self) -> &BStr {
        let bytes = self.0.as_bytes();
        if bytes.is_empty() {
            return BStr::new(b"");
        }

        match bytes.iter().rposition(|&b| b == b'/') {
            Some(pos) => BStr::new(&bytes[pos + 1..]),
            None => BStr::new(bytes),
        }
    }

    /// Normalize the path by resolving `.` and `..` components.
    /// Returns an error if `..` tries to go above the root.
    ///
    /// This matches C git's `normalize_path_copy` behavior.
    pub fn normalize(&self) -> Result<GitPath> {
        let bytes = self.0.as_bytes();
        if bytes.is_empty() {
            return Ok(GitPath::new(b"" as &[u8]));
        }

        let mut components: Vec<&[u8]> = Vec::new();
        let is_absolute = !bytes.is_empty() && bytes[0] == b'/';

        for component in bytes.split(|&b| b == b'/') {
            match component {
                b"" | b"." => continue,
                b".." => {
                    if components.is_empty() {
                        if is_absolute {
                            return Err(UtilError::Path(
                                "cannot normalize path above root".into(),
                            ));
                        }
                        // For relative paths, keep the ..
                        components.push(b"..");
                    } else if components.last() == Some(&(b".." as &[u8])) {
                        components.push(b"..");
                    } else {
                        components.pop();
                    }
                }
                other => components.push(other),
            }
        }

        let mut result = BString::new(Vec::new());
        if is_absolute {
            result.push_byte(b'/');
        }

        for (i, component) in components.iter().enumerate() {
            if i > 0 {
                result.push_byte(b'/');
            }
            result.push_str(component);
        }

        if result.is_empty() {
            if is_absolute {
                return Ok(GitPath::new(b"/" as &[u8]));
            }
            return Ok(GitPath::new(b"." as &[u8]));
        }

        Ok(GitPath::from_normalized(result))
    }

    /// Convert to a platform-native OS path for file system operations.
    pub fn to_os_path(&self) -> std::path::PathBuf {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            std::path::PathBuf::from(std::ffi::OsStr::from_bytes(self.0.as_bytes()))
        }
        #[cfg(not(unix))]
        {
            // On Windows, convert forward slashes to backslashes
            let s = self.0.to_str_lossy();
            std::path::PathBuf::from(s.replace('/', "\\"))
        }
    }

    /// Check if the path is absolute.
    pub fn is_absolute(&self) -> bool {
        let bytes = self.0.as_bytes();
        if bytes.is_empty() {
            return false;
        }
        // Unix absolute
        if bytes[0] == b'/' {
            return true;
        }
        // Windows drive letter: C:/
        if bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && is_dir_sep(bytes[2])
        {
            return true;
        }
        false
    }

    /// Make this path relative to a base path.
    ///
    /// Matching C git's `relative_path` behavior.
    pub fn relative_to(&self, base: &GitPath) -> Result<GitPath> {
        let in_bytes = self.0.as_bytes();
        let prefix_bytes = base.0.as_bytes();

        if in_bytes.is_empty() {
            return Ok(GitPath::new(b"." as &[u8]));
        }
        if prefix_bytes.is_empty() {
            return Ok(self.clone());
        }

        // Find common prefix
        let mut i = 0; // index into prefix
        let mut j = 0; // index into self
        let mut prefix_off = 0;
        let mut in_off = 0;

        while i < prefix_bytes.len() && j < in_bytes.len() && prefix_bytes[i] == in_bytes[j] {
            if prefix_bytes[i] == b'/' {
                while i < prefix_bytes.len() && prefix_bytes[i] == b'/' {
                    i += 1;
                }
                while j < in_bytes.len() && in_bytes[j] == b'/' {
                    j += 1;
                }
                prefix_off = i;
                in_off = j;
            } else {
                i += 1;
                j += 1;
            }
        }

        // Check if prefix is actually a prefix of the path
        if i >= prefix_bytes.len() && prefix_off < prefix_bytes.len() {
            if j >= in_bytes.len() {
                // Exact match
                return Ok(GitPath::new(b"." as &[u8]));
            } else if j < in_bytes.len() && in_bytes[j] == b'/' {
                // in="/a/b/c", prefix="/a/b"
                while j < in_bytes.len() && in_bytes[j] == b'/' {
                    j += 1;
                }
                in_off = j;
            } else {
                // in="/a/bbb/c", prefix="/a/b" - not a true prefix
                i = prefix_off;
            }
        } else if j >= in_bytes.len() && in_off < in_bytes.len()
            && i < prefix_bytes.len() && prefix_bytes[i] == b'/' {
            while i < prefix_bytes.len() && prefix_bytes[i] == b'/' {
                i += 1;
            }
            in_off = in_bytes.len();
        }

        let remaining_in = &in_bytes[in_off..];

        if i >= prefix_bytes.len() {
            if remaining_in.is_empty() {
                return Ok(GitPath::new(b"." as &[u8]));
            }
            return Ok(GitPath::new(remaining_in));
        }

        // Count remaining prefix directories to add "../"
        let mut result = BString::new(Vec::new());
        let mut pi = i;
        while pi < prefix_bytes.len() {
            if prefix_bytes[pi] == b'/' {
                result.push_str(b"../");
                while pi < prefix_bytes.len() && prefix_bytes[pi] == b'/' {
                    pi += 1;
                }
                continue;
            }
            pi += 1;
        }
        if prefix_bytes.last() != Some(&b'/') {
            result.push_str(b"../");
        }

        result.push_str(remaining_in);

        // Remove trailing slash
        while result.len() > 1 && result.last() == Some(&b'/') {
            result.pop();
        }

        if result.is_empty() {
            return Ok(GitPath::new(b"." as &[u8]));
        }

        Ok(GitPath::from_normalized(result))
    }

    /// Check if the path is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if this path has a specific extension.
    pub fn has_extension(&self, ext: &[u8]) -> bool {
        let basename = self.basename();
        let basename_bytes = basename.as_bytes();
        if let Some(dot_pos) = basename_bytes.iter().rposition(|&b| b == b'.') {
            &basename_bytes[dot_pos + 1..] == ext
        } else {
            false
        }
    }
}

impl std::fmt::Display for GitPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_bstr())
    }
}

impl AsRef<[u8]> for GitPath {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<&[u8]> for GitPath {
    fn from(bytes: &[u8]) -> Self {
        GitPath::new(bytes)
    }
}

impl From<&str> for GitPath {
    fn from(s: &str) -> Self {
        GitPath::new(s.as_bytes())
    }
}

impl From<BString> for GitPath {
    fn from(s: BString) -> Self {
        GitPath::new(s)
    }
}

/// Quote a path for display, matching C git's `core.quotePath=true` default behavior.
///
/// If any byte is non-printable (< 0x20 or 0x7f) or non-ASCII (> 0x7f), or the path
/// contains a backslash or double-quote, the entire path is wrapped in double quotes
/// with those bytes octal-escaped as `\NNN`. Printable ASCII bytes are passed through.
///
/// Examples:
/// - `café.txt` → `"caf\303\251.txt"`
/// - `hello.txt` → `hello.txt` (no quoting needed)
/// - `a "b"` → `"a \"b\""`
pub fn quote_path(path: &[u8]) -> String {
    let needs_quoting = path.iter().any(|&b| {
        b < 0x20 || b == 0x7f || b > 0x7f || b == b'\\' || b == b'"'
    });

    if !needs_quoting {
        // Safe to convert directly — all bytes are printable ASCII
        return String::from_utf8_lossy(path).into_owned();
    }

    let mut out = String::with_capacity(path.len() + 8);
    out.push('"');
    for &b in path {
        if b == b'\\' {
            out.push_str("\\\\");
        } else if b == b'"' {
            out.push_str("\\\"");
        } else if b == b'\n' {
            out.push_str("\\n");
        } else if b == b'\t' {
            out.push_str("\\t");
        } else if b < 0x20 || b == 0x7f || b > 0x7f {
            // Octal-escape non-printable and non-ASCII bytes
            out.push_str(&format!("\\{:03o}", b));
        } else {
            out.push(b as char);
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_normalizes_separators() {
        let p = GitPath::new(b"a\\b\\c" as &[u8]);
        assert_eq!(p.as_bytes(), b"a/b/c");
    }

    #[test]
    fn new_removes_trailing_slash() {
        let p = GitPath::new(b"a/b/" as &[u8]);
        assert_eq!(p.as_bytes(), b"a/b");
    }

    #[test]
    fn new_preserves_root_slash() {
        let p = GitPath::new(b"/" as &[u8]);
        assert_eq!(p.as_bytes(), b"/");
    }

    #[test]
    fn join_basic() {
        let base = GitPath::new(b"a/b" as &[u8]);
        let joined = base.join(b"c/d" as &[u8]);
        assert_eq!(joined.as_bytes(), b"a/b/c/d");
    }

    #[test]
    fn join_absolute_replaces() {
        let base = GitPath::new(b"a/b" as &[u8]);
        let joined = base.join(b"/c/d" as &[u8]);
        assert_eq!(joined.as_bytes(), b"/c/d");
    }

    #[test]
    fn join_empty_other() {
        let base = GitPath::new(b"a/b" as &[u8]);
        let joined = base.join(b"" as &[u8]);
        assert_eq!(joined.as_bytes(), b"a/b");
    }

    #[test]
    fn dirname_basic() {
        assert_eq!(GitPath::new(b"a/b/c" as &[u8]).dirname(), BStr::new(b"a/b"));
        assert_eq!(GitPath::new(b"a/b" as &[u8]).dirname(), BStr::new(b"a"));
        assert_eq!(GitPath::new(b"abc" as &[u8]).dirname(), BStr::new(b"."));
        assert_eq!(GitPath::new(b"/abc" as &[u8]).dirname(), BStr::new(b"/"));
    }

    #[test]
    fn basename_basic() {
        assert_eq!(GitPath::new(b"a/b/c" as &[u8]).basename(), BStr::new(b"c"));
        assert_eq!(GitPath::new(b"abc" as &[u8]).basename(), BStr::new(b"abc"));
        assert_eq!(GitPath::new(b"/abc" as &[u8]).basename(), BStr::new(b"abc"));
    }

    #[test]
    fn normalize_dots() {
        assert_eq!(
            GitPath::new(b"a/./b/../c" as &[u8]).normalize().unwrap().as_bytes(),
            b"a/c"
        );
    }

    #[test]
    fn normalize_absolute() {
        assert_eq!(
            GitPath::new(b"/a/b/../c" as &[u8]).normalize().unwrap().as_bytes(),
            b"/a/c"
        );
    }

    #[test]
    fn normalize_above_root_errors() {
        assert!(GitPath::new(b"/a/../.." as &[u8]).normalize().is_err());
    }

    #[test]
    fn normalize_relative_dotdot() {
        assert_eq!(
            GitPath::new(b"../a" as &[u8]).normalize().unwrap().as_bytes(),
            b"../a"
        );
    }

    #[test]
    fn normalize_just_dot() {
        assert_eq!(
            GitPath::new(b"." as &[u8]).normalize().unwrap().as_bytes(),
            b"."
        );
    }

    #[test]
    fn to_os_path() {
        let p = GitPath::new(b"a/b/c" as &[u8]);
        let os = p.to_os_path();
        // On Unix, should be a/b/c
        assert!(os.to_str().unwrap().contains("a"));
    }

    #[test]
    fn is_absolute() {
        assert!(GitPath::new(b"/foo" as &[u8]).is_absolute());
        assert!(!GitPath::new(b"foo" as &[u8]).is_absolute());
        assert!(!GitPath::new(b"" as &[u8]).is_absolute());
    }

    #[test]
    fn relative_to_basic() {
        let path = GitPath::new(b"a/b/c" as &[u8]);
        let base = GitPath::new(b"a/b" as &[u8]);
        let rel = path.relative_to(&base).unwrap();
        assert_eq!(rel.as_bytes(), b"c");
    }

    #[test]
    fn relative_to_same() {
        let path = GitPath::new(b"a/b" as &[u8]);
        let base = GitPath::new(b"a/b" as &[u8]);
        let rel = path.relative_to(&base).unwrap();
        assert_eq!(rel.as_bytes(), b".");
    }

    #[test]
    fn relative_to_sibling() {
        let path = GitPath::new(b"a/c" as &[u8]);
        let base = GitPath::new(b"a/b" as &[u8]);
        let rel = path.relative_to(&base).unwrap();
        assert_eq!(rel.as_bytes(), b"../c");
    }

    #[test]
    fn has_extension_test() {
        assert!(GitPath::new(b"foo.c" as &[u8]).has_extension(b"c"));
        assert!(!GitPath::new(b"foo.c" as &[u8]).has_extension(b"h"));
        assert!(GitPath::new(b"dir/foo.rs" as &[u8]).has_extension(b"rs"));
    }

    #[test]
    fn display() {
        let p = GitPath::new(b"a/b/c" as &[u8]);
        assert_eq!(format!("{}", p), "a/b/c");
    }
}

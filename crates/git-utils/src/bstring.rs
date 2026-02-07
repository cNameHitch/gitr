use bstr::{BStr, BString, ByteSlice, ByteVec};

/// Extension trait for git-specific byte string operations.
pub trait GitBStringExt {
    /// Shell-quote a byte string for safe display.
    ///
    /// Wraps the string in single quotes, escaping any single quotes as `'\''`
    /// and exclamation points as `'\!'`, matching C git's `sq_quote_buf`.
    fn shell_quote(&self) -> BString;

    /// C-style quote (backslash escaping).
    ///
    /// Produces a double-quoted string with backslash escapes for control
    /// characters, non-ASCII bytes (as octal), `"`, and `\`, matching C git's
    /// `quote_c_style`. Returns the original string unchanged if no quoting
    /// is needed.
    fn c_quote(&self) -> BString;

    /// Check if the string needs quoting for git output.
    ///
    /// Returns true if the byte string contains characters that require
    /// C-style quoting: control chars, `"`, `\`, or bytes >= 0x80.
    fn needs_quoting(&self) -> bool;

    /// Trim trailing newlines (like strbuf_rtrim).
    fn rtrim_newlines(&self) -> &BStr;
}

/// Returns true if a byte needs shell-quoting (single quote or exclamation).
fn need_bs_quote(c: u8) -> bool {
    c == b'\'' || c == b'!'
}

/// C-quote lookup table matching C git's `cq_lookup`.
/// Positive: quote as octal always.
/// Zero: quote as octal if quote_path_fully.
/// Negative: never quote.
/// Char value: quote as `\<char>`.
fn cq_lookup(c: u8) -> i8 {
    match c {
        0x00..=0x06 => 1,
        0x07 => b'a' as i8,
        0x08 => b'b' as i8,
        0x09 => b't' as i8,
        0x0a => b'n' as i8,
        0x0b => b'v' as i8,
        0x0c => b'f' as i8,
        0x0d => b'r' as i8,
        0x0e..=0x1f => 1,
        0x22 => b'"' as i8,  // "
        0x5c => b'\\' as i8, // backslash
        0x7f => 1,
        0x80..=0xff => 0, // high bytes: quote if quote_path_fully
        _ => -1,          // printable ASCII (except " and \)
    }
}

/// Returns true if a byte must be C-quoted (always quoting high bytes).
fn cq_must_quote(c: u8) -> bool {
    // We always quote path fully (matching git default behavior)
    cq_lookup(c) + 1 > 0
}

impl GitBStringExt for BStr {
    fn shell_quote(&self) -> BString {
        let mut out = BString::from("'");
        let bytes = self.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            // Find the next byte that needs escaping
            let start = i;
            while i < bytes.len() && !need_bs_quote(bytes[i]) {
                i += 1;
            }
            out.push_str(&bytes[start..i]);
            // Escape any single quotes or exclamation marks
            while i < bytes.len() && need_bs_quote(bytes[i]) {
                out.push_str(b"'\\");
                out.push_byte(bytes[i]);
                out.push_byte(b'\'');
                i += 1;
            }
        }
        out.push_byte(b'\'');
        out
    }

    fn c_quote(&self) -> BString {
        if !self.needs_quoting() {
            return BString::from(self.as_bytes());
        }

        let mut out = BString::from("\"");
        for &b in self.as_bytes() {
            if !cq_must_quote(b) {
                out.push_byte(b);
            } else {
                let lookup = cq_lookup(b);
                out.push_byte(b'\\');
                if lookup >= b' ' as i8 {
                    out.push_byte(lookup as u8);
                } else {
                    // Octal escape
                    out.push_byte(((b >> 6) & 0o3) + b'0');
                    out.push_byte(((b >> 3) & 0o7) + b'0');
                    out.push_byte((b & 0o7) + b'0');
                }
            }
        }
        out.push_byte(b'"');
        out
    }

    fn needs_quoting(&self) -> bool {
        self.as_bytes().iter().any(|&b| cq_must_quote(b))
    }

    fn rtrim_newlines(&self) -> &BStr {
        let bytes = self.as_bytes();
        let end = bytes
            .iter()
            .rposition(|&b| b != b'\n' && b != b'\r')
            .map(|i| i + 1)
            .unwrap_or(0);
        BStr::new(&bytes[..end])
    }
}

impl GitBStringExt for BString {
    fn shell_quote(&self) -> BString {
        self.as_bstr().shell_quote()
    }

    fn c_quote(&self) -> BString {
        self.as_bstr().c_quote()
    }

    fn needs_quoting(&self) -> bool {
        self.as_bstr().needs_quoting()
    }

    fn rtrim_newlines(&self) -> &BStr {
        self.as_bstr().rtrim_newlines()
    }
}

impl GitBStringExt for [u8] {
    fn shell_quote(&self) -> BString {
        BStr::new(self).shell_quote()
    }

    fn c_quote(&self) -> BString {
        BStr::new(self).c_quote()
    }

    fn needs_quoting(&self) -> bool {
        BStr::new(self).needs_quoting()
    }

    fn rtrim_newlines(&self) -> &BStr {
        BStr::new(self).rtrim_newlines()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_simple() {
        assert_eq!(b"name".shell_quote(), BString::from("'name'"));
    }

    #[test]
    fn shell_quote_with_spaces() {
        assert_eq!(b"a b".shell_quote(), BString::from("'a b'"));
    }

    #[test]
    fn shell_quote_with_single_quote() {
        assert_eq!(b"a'b".shell_quote(), BString::from("'a'\\''b'"));
    }

    #[test]
    fn shell_quote_with_exclamation() {
        assert_eq!(b"a!b".shell_quote(), BString::from("'a'\\!'b'"));
    }

    #[test]
    fn shell_quote_empty() {
        assert_eq!(b"".shell_quote(), BString::from("''"));
    }

    #[test]
    fn c_quote_simple() {
        // No quoting needed for simple strings
        assert_eq!(b"hello".c_quote(), BString::from("hello"));
    }

    #[test]
    fn c_quote_with_newline() {
        assert_eq!(b"a\nb".c_quote(), BString::from("\"a\\nb\""));
    }

    #[test]
    fn c_quote_with_tab() {
        assert_eq!(b"a\tb".c_quote(), BString::from("\"a\\tb\""));
    }

    #[test]
    fn c_quote_with_double_quote() {
        assert_eq!(b"a\"b".c_quote(), BString::from("\"a\\\"b\""));
    }

    #[test]
    fn c_quote_with_backslash() {
        assert_eq!(b"a\\b".c_quote(), BString::from("\"a\\\\b\""));
    }

    #[test]
    fn c_quote_with_high_bytes() {
        let input: &[u8] = &[0x80, 0xff];
        let quoted = input.c_quote();
        assert_eq!(quoted, BString::from("\"\\200\\377\""));
    }

    #[test]
    fn needs_quoting_simple() {
        assert!(!b"hello".needs_quoting());
        assert!(!b"foo/bar".needs_quoting());
    }

    #[test]
    fn needs_quoting_control_chars() {
        assert!(b"a\nb".needs_quoting());
        assert!(b"a\tb".needs_quoting());
        assert!(b"\x00".needs_quoting());
    }

    #[test]
    fn needs_quoting_high_bytes() {
        assert!([0x80u8].needs_quoting());
    }

    #[test]
    fn rtrim_newlines_basic() {
        assert_eq!(b"hello\n".rtrim_newlines(), BStr::new(b"hello"));
        assert_eq!(b"hello\n\n".rtrim_newlines(), BStr::new(b"hello"));
        assert_eq!(b"hello\r\n".rtrim_newlines(), BStr::new(b"hello"));
        assert_eq!(b"hello".rtrim_newlines(), BStr::new(b"hello"));
        assert_eq!(b"\n".rtrim_newlines(), BStr::new(b""));
        assert_eq!(b"".rtrim_newlines(), BStr::new(b""));
    }
}

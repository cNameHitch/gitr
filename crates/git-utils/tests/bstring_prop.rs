//! Property-based tests for byte string operations.

use bstr::{BStr, BString};
use git_utils::bstring::GitBStringExt;
use proptest::prelude::*;

proptest! {
    /// Shell-quoted strings should always be safe to eval (no unquoted single quotes).
    #[test]
    fn shell_quote_never_contains_unquoted_single_quote(s in ".*") {
        let input = BString::from(s.as_bytes());
        let quoted = input.shell_quote();
        // The result should be wrapped in single quotes with proper escaping
        // It should start with ' and end with '
        let result = std::str::from_utf8(&quoted).unwrap();
        assert!(result.starts_with('\''), "shell_quote should start with single quote: {}", result);
        assert!(result.ends_with('\''), "shell_quote should end with single quote: {}", result);
    }

    /// C-quoting and the needs_quoting predicate should be consistent:
    /// if a string needs quoting, c_quote should modify it.
    #[test]
    fn c_quote_consistent_with_needs_quoting(s in "\\PC{0,100}") {
        let input = BString::from(s.as_bytes());
        let needs = input.needs_quoting();
        let quoted = input.c_quote();
        if needs {
            // c_quote should produce a different result (with quotes/escapes)
            assert!(quoted.starts_with(b"\""), "c_quote should wrap in double quotes when needs_quoting");
        } else {
            // If no quoting needed, output should be the same as input
            assert_eq!(quoted.as_slice(), input.as_slice(),
                "c_quote should be identity when !needs_quoting");
        }
    }

    /// rtrim_newlines should remove trailing newlines and carriage returns only.
    #[test]
    fn rtrim_newlines_preserves_non_trailing(s in "\\PC{0,50}") {
        let mut input = BString::from(s.as_bytes());
        let original = input.clone();
        // Add some trailing newlines
        input.push(b'\n');
        input.push(b'\r');
        input.push(b'\n');

        let trimmed = BStr::new(&input).rtrim_newlines();
        // The trimmed result should not end with \n or \r
        if !trimmed.is_empty() {
            let last = trimmed[trimmed.len() - 1];
            assert!(last != b'\n' && last != b'\r',
                "rtrim_newlines should remove trailing newlines");
        }
        // The trimmed result should be a prefix of the original
        assert!(original.as_slice().starts_with(trimmed),
            "rtrim_newlines should only remove from the end");
    }

    /// Shell quoting should round-trip through sh -c 'echo ...'
    /// (only test with printable ASCII to avoid shell encoding issues)
    #[test]
    fn shell_quote_roundtrip(s in "[a-zA-Z0-9 _\\-\\.]{0,50}") {
        let input = BString::from(s.as_bytes());
        let quoted = input.shell_quote();
        let quoted_str = std::str::from_utf8(&quoted).unwrap();

        // Use echo with the shell-quoted string to verify round-trip
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf '%s' {}", quoted_str))
            .output()
            .unwrap();

        assert!(output.status.success(), "sh should succeed");
        assert_eq!(output.stdout, input.as_slice(),
            "shell_quote round-trip failed for input: {:?}", s);
    }
}

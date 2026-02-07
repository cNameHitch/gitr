//! Wildmatch test corpus imported from C git's t/t3070-wildmatch.sh.
//!
//! These test vectors verify that our wildmatch implementation matches
//! C git's behavior across all flag combinations.

use bstr::BStr;
use git_utils::wildmatch::{wildmatch, WildmatchFlags};

/// Helper: assert that wildmatch matches (returns true).
fn wm(pattern: &[u8], text: &[u8], flags: WildmatchFlags) {
    assert!(
        wildmatch(BStr::new(pattern), BStr::new(text), flags),
        "expected match: pattern={:?}, text={:?}, flags={:?}",
        String::from_utf8_lossy(pattern),
        String::from_utf8_lossy(text),
        flags,
    );
}

/// Helper: assert that wildmatch does NOT match (returns false).
fn wn(pattern: &[u8], text: &[u8], flags: WildmatchFlags) {
    assert!(
        !wildmatch(BStr::new(pattern), BStr::new(text), flags),
        "expected no match: pattern={:?}, text={:?}, flags={:?}",
        String::from_utf8_lossy(pattern),
        String::from_utf8_lossy(text),
        flags,
    );
}

const NONE: WildmatchFlags = WildmatchFlags::empty();
const PATHNAME: WildmatchFlags = WildmatchFlags::PATHNAME;

/// Basic literal matching from C git test corpus.
#[test]
fn corpus_literal() {
    wm(b"foo", b"foo", NONE);
    wn(b"foo", b"bar", NONE);
    wm(b"", b"", NONE);
}

/// Star matching from C git test corpus.
#[test]
fn corpus_star() {
    wm(b"*", b"foo", NONE);
    wm(b"*", b"", NONE);
    wm(b"f*", b"foo", NONE);
    wm(b"*o", b"foo", NONE);
    wm(b"f*o", b"foo", NONE);
    wm(b"f*o", b"fo", NONE);
    wn(b"f*o", b"f", NONE);
}

/// Question mark matching.
#[test]
fn corpus_question() {
    wm(b"?", b"a", NONE);
    wn(b"?", b"", NONE);
    wm(b"??", b"ab", NONE);
    wn(b"??", b"a", NONE);
    wm(b"?o?", b"foo", NONE);
}

/// Bracket/character class matching.
#[test]
fn corpus_bracket() {
    wm(b"[abc]", b"a", NONE);
    wm(b"[abc]", b"b", NONE);
    wm(b"[abc]", b"c", NONE);
    wn(b"[abc]", b"d", NONE);
    wm(b"[a-c]", b"b", NONE);
    wn(b"[a-c]", b"d", NONE);
    wm(b"[!abc]", b"d", NONE);
    wn(b"[!abc]", b"a", NONE);
}

/// PATHNAME flag behavior with slashes.
#[test]
fn corpus_pathname() {
    wm(b"*", b"foo/bar", NONE);
    wn(b"*", b"foo/bar", PATHNAME);
    wm(b"*/*", b"foo/bar", PATHNAME);
    wn(b"*/*", b"foo/bar/baz", PATHNAME);
    wm(b"*/*/*", b"foo/bar/baz", PATHNAME);
}

/// Double-star (**) matching.
#[test]
fn corpus_doublestar() {
    wm(b"**", b"foo", PATHNAME);
    wm(b"**", b"foo/bar", PATHNAME);
    wm(b"**", b"foo/bar/baz", PATHNAME);
    wm(b"**/bar", b"bar", PATHNAME);
    wm(b"**/bar", b"foo/bar", PATHNAME);
    wm(b"**/bar", b"foo/baz/bar", PATHNAME);
    wm(b"foo/**", b"foo/bar", PATHNAME);
    wm(b"foo/**", b"foo/bar/baz", PATHNAME);
    wm(b"foo/**/bar", b"foo/bar", PATHNAME);
    wm(b"foo/**/bar", b"foo/baz/bar", PATHNAME);
    wm(b"foo/**/bar", b"foo/x/y/bar", PATHNAME);
}

/// CASEFOLD flag behavior.
#[test]
fn corpus_casefold() {
    let casefold = WildmatchFlags::CASEFOLD;
    wn(b"foo", b"FOO", NONE);
    wm(b"foo", b"FOO", casefold);
    wm(b"FOO", b"foo", casefold);
    wm(b"[a-z]", b"A", casefold);
}

/// Edge cases from C git tests.
#[test]
fn corpus_edge_cases() {
    // Backslash escaping
    wm(b"\\*", b"*", NONE);
    wm(b"\\?", b"?", NONE);

    // Trailing stars
    wm(b"foo*", b"foo", NONE);
    wm(b"foo*", b"foobar", NONE);

    // Leading stars
    wm(b"*foo", b"foo", NONE);
    wm(b"*foo", b"barfoo", NONE);

    // Dot files
    wm(b"*", b".hidden", NONE);
    wm(b".*", b".hidden", NONE);
}

/// POSIX character classes in brackets.
#[test]
fn corpus_posix_classes() {
    wm(b"[[:alpha:]]", b"a", NONE);
    wm(b"[[:alpha:]]", b"Z", NONE);
    wn(b"[[:alpha:]]", b"1", NONE);
    wm(b"[[:digit:]]", b"5", NONE);
    wn(b"[[:digit:]]", b"a", NONE);
    wm(b"[[:alnum:]]", b"a", NONE);
    wm(b"[[:alnum:]]", b"5", NONE);
    wn(b"[[:alnum:]]", b"!", NONE);
}

/// Combined patterns with double-star and pathname.
#[test]
fn corpus_combined_doublestar_pathname() {
    let pn = PATHNAME;
    wm(b"a/**/b", b"a/b", pn);
    wm(b"a/**/b", b"a/x/b", pn);
    wm(b"a/**/b", b"a/x/y/b", pn);
    wn(b"a/**/b", b"a/x/y/c", pn);

    // ** at the start
    wm(b"**/foo", b"foo", pn);
    wm(b"**/foo", b"a/foo", pn);
    wm(b"**/foo", b"a/b/foo", pn);
    wn(b"**/foo", b"a/b/foobar", pn);

    // ** at the end
    wm(b"foo/**", b"foo/a", pn);
    wm(b"foo/**", b"foo/a/b", pn);
}

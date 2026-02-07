use bstr::{BStr, BString, ByteSlice};

bitflags::bitflags! {
    /// Flags controlling wildmatch behavior.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WildmatchFlags: u32 {
        /// Case-insensitive matching.
        const CASEFOLD = 0x01;
        /// Don't match '/' with wildcards (pathname mode).
        const PATHNAME = 0x02;
    }
}

/// Internal return values matching C git.
const WM_MATCH: i32 = 0;
const WM_NOMATCH: i32 = 1;
const WM_ABORT_ALL: i32 = -1;
const WM_ABORT_TO_STARSTAR: i32 = -2;

/// Check if a character is a glob special character.
fn is_glob_special(c: u8) -> bool {
    matches!(c, b'*' | b'?' | b'[' | b'\\')
}

/// Core wildmatch algorithm, faithful port of C git's `dowild`.
fn dowild(pattern: &[u8], text: &[u8], flags: WildmatchFlags) -> i32 {
    let mut pi = 0; // pattern index
    let mut ti = 0; // text index
    let pattern_start = 0;

    while pi < pattern.len() {
        let mut p_ch = pattern[pi];
        let t_ch = if ti < text.len() {
            text[ti]
        } else if p_ch != b'*' {
            return WM_ABORT_ALL;
        } else {
            0 // will be handled by '*' case
        };

        let t_ch_cmp = if flags.contains(WildmatchFlags::CASEFOLD) && t_ch.is_ascii_uppercase() {
            t_ch.to_ascii_lowercase()
        } else {
            t_ch
        };
        let p_ch_cmp = if flags.contains(WildmatchFlags::CASEFOLD) && p_ch.is_ascii_uppercase() {
            p_ch.to_ascii_lowercase()
        } else {
            p_ch
        };

        match p_ch {
            b'\\' => {
                // Literal match with following character
                pi += 1;
                if pi >= pattern.len() {
                    return WM_ABORT_ALL;
                }
                p_ch = pattern[pi];
                let p_ch_esc = if flags.contains(WildmatchFlags::CASEFOLD)
                    && p_ch.is_ascii_uppercase()
                {
                    p_ch.to_ascii_lowercase()
                } else {
                    p_ch
                };
                if t_ch_cmp != p_ch_esc {
                    return WM_NOMATCH;
                }
                ti += 1;
                pi += 1;
            }
            b'?' => {
                if flags.contains(WildmatchFlags::PATHNAME) && t_ch == b'/' {
                    return WM_NOMATCH;
                }
                ti += 1;
                pi += 1;
            }
            b'*' => {
                return handle_star(pattern, pi, text, ti, flags, pattern_start);
            }
            b'[' => {
                let result = handle_bracket(pattern, &mut pi, t_ch, t_ch_cmp, flags);
                if result != WM_MATCH {
                    return result;
                }
                ti += 1;
                pi += 1; // past the ']'
            }
            _ => {
                if t_ch_cmp != p_ch_cmp {
                    return WM_NOMATCH;
                }
                ti += 1;
                pi += 1;
            }
        }
    }

    if ti < text.len() {
        WM_NOMATCH
    } else {
        WM_MATCH
    }
}

/// Handle the '*' and '**' pattern cases.
fn handle_star(
    pattern: &[u8],
    mut pi: usize,
    text: &[u8],
    mut ti: usize,
    flags: WildmatchFlags,
    pattern_start: usize,
) -> i32 {
    pi += 1; // skip first *
    let match_slash;

    if pi < pattern.len() && pattern[pi] == b'*' {
        // '**' case
        let prev_p = pi;
        while pi < pattern.len() && pattern[pi] == b'*' {
            pi += 1;
        }

        if !flags.contains(WildmatchFlags::PATHNAME) {
            // Without PATHNAME, '*' == '**'
            match_slash = true;
        } else if (prev_p.wrapping_sub(pattern_start) < 2
            || pattern[prev_p.wrapping_sub(2)] == b'/')
            && (pi >= pattern.len()
                || pattern[pi] == b'/'
                || (pi + 1 < pattern.len() && pattern[pi] == b'\\' && pattern[pi + 1] == b'/'))
        {
            // Valid '**/' or trailing '**'
            if pi < pattern.len() && pattern[pi] == b'/'
                && dowild(&pattern[pi + 1..], &text[ti..], flags) == WM_MATCH {
                return WM_MATCH;
            }
            match_slash = true;
        } else {
            // '**' in the middle without proper separators
            match_slash = false;
        }
    } else {
        // Single '*'
        match_slash = !flags.contains(WildmatchFlags::PATHNAME);
    }

    // Trailing star(s)
    if pi >= pattern.len() {
        if !match_slash && text[ti..].contains(&b'/') {
            return WM_ABORT_TO_STARSTAR;
        }
        return WM_MATCH;
    }

    // Single '*' followed by '/' in PATHNAME mode
    if !match_slash && pi < pattern.len() && pattern[pi] == b'/' {
        if let Some(pos) = text[ti..].iter().position(|&b| b == b'/') {
            ti += pos;
            // The slash is consumed by continuing; advance past '/'
            ti += 1;
            pi += 1;
            return dowild(&pattern[pi..], &text[ti..], flags);
        } else {
            return WM_ABORT_ALL;
        }
    }

    // General star matching
    while ti < text.len() {
        let _t_ch = text[ti];

        // Optimization: skip ahead when star is followed by a literal
        if pi < pattern.len() && !is_glob_special(pattern[pi]) {
            let mut p_ch = pattern[pi];
            if flags.contains(WildmatchFlags::CASEFOLD) && p_ch.is_ascii_uppercase() {
                p_ch = p_ch.to_ascii_lowercase();
            }
            while ti < text.len() && (match_slash || text[ti] != b'/') {
                let mut tc = text[ti];
                if flags.contains(WildmatchFlags::CASEFOLD) && tc.is_ascii_uppercase() {
                    tc = tc.to_ascii_lowercase();
                }
                if tc == p_ch {
                    break;
                }
                ti += 1;
            }
            if ti >= text.len() || {
                let mut tc = text[ti];
                if flags.contains(WildmatchFlags::CASEFOLD) && tc.is_ascii_uppercase() {
                    tc = tc.to_ascii_lowercase();
                }
                tc != p_ch
            } {
                if match_slash {
                    return WM_ABORT_ALL;
                } else {
                    return WM_ABORT_TO_STARSTAR;
                }
            }
        }

        let matched = dowild(&pattern[pi..], &text[ti..], flags);
        if matched != WM_NOMATCH {
            if !match_slash || matched != WM_ABORT_TO_STARSTAR {
                return matched;
            }
        } else if !match_slash && text[ti] == b'/' {
            return WM_ABORT_TO_STARSTAR;
        }
        ti += 1;
    }

    WM_ABORT_ALL
}

/// Handle the '[...]' bracket expression.
fn handle_bracket(
    pattern: &[u8],
    pi: &mut usize,
    t_ch: u8,
    t_ch_cmp: u8,
    flags: WildmatchFlags,
) -> i32 {
    *pi += 1; // skip '['
    if *pi >= pattern.len() {
        return WM_ABORT_ALL;
    }

    let mut p_ch = pattern[*pi];
    // Handle negation
    if p_ch == b'^' {
        p_ch = b'!';
    }
    let negated = p_ch == b'!';
    if negated {
        *pi += 1;
        if *pi >= pattern.len() {
            return WM_ABORT_ALL;
        }
        p_ch = pattern[*pi];
    }

    let mut matched = false;
    let mut prev_ch: u8 = 0;

    loop {
        if *pi >= pattern.len() || p_ch == 0 {
            return WM_ABORT_ALL;
        }

        if p_ch == b'\\' {
            *pi += 1;
            if *pi >= pattern.len() {
                return WM_ABORT_ALL;
            }
            p_ch = pattern[*pi];
            if t_ch_cmp == p_ch
                || (flags.contains(WildmatchFlags::CASEFOLD)
                    && t_ch_cmp == p_ch.to_ascii_lowercase())
            {
                matched = true;
            }
        } else if p_ch == b'-'
            && prev_ch != 0
            && *pi + 1 < pattern.len()
            && pattern[*pi + 1] != b']'
        {
            // Range expression
            *pi += 1;
            p_ch = pattern[*pi];
            if p_ch == b'\\' {
                *pi += 1;
                if *pi >= pattern.len() {
                    return WM_ABORT_ALL;
                }
                p_ch = pattern[*pi];
            }
            if t_ch_cmp <= p_ch && t_ch_cmp >= prev_ch {
                matched = true;
            } else if flags.contains(WildmatchFlags::CASEFOLD) && t_ch.is_ascii_lowercase() {
                let t_upper = t_ch.to_ascii_uppercase();
                if t_upper <= p_ch && t_upper >= prev_ch {
                    matched = true;
                }
            }
            p_ch = 0; // reset prev_ch
        } else if p_ch == b'[' && *pi + 1 < pattern.len() && pattern[*pi + 1] == b':' {
            // POSIX character class
            let class_start = *pi + 2;
            // Find the closing ":]"
            let mut end = class_start;
            while end < pattern.len() && pattern[end] != b']' {
                end += 1;
            }
            if end >= pattern.len() {
                return WM_ABORT_ALL;
            }
            let class_len = end - class_start - 1;
            if end == 0 || pattern[end - 1] != b':' {
                // Didn't find ":]", treat '[' as literal
                if t_ch_cmp == b'[' {
                    matched = true;
                }
            } else {
                let class_name = &pattern[class_start..class_start + class_len];
                if !match_char_class(class_name, t_ch_cmp, flags) {
                    // Check if class matched (already handled in match_char_class)
                } else {
                    matched = true;
                }
                *pi = end; // advance past ']'
                p_ch = 0; // reset prev_ch
            }
        } else if t_ch_cmp == p_ch
            || (flags.contains(WildmatchFlags::CASEFOLD)
                && t_ch_cmp == p_ch.to_ascii_lowercase())
        {
            matched = true;
        }

        prev_ch = p_ch;
        *pi += 1;
        if *pi >= pattern.len() {
            return WM_ABORT_ALL;
        }
        p_ch = pattern[*pi];

        if p_ch == b']' {
            break;
        }
    }

    if matched == negated || (flags.contains(WildmatchFlags::PATHNAME) && t_ch == b'/') {
        return WM_NOMATCH;
    }

    WM_MATCH
}

/// Match a character against a POSIX character class.
fn match_char_class(class: &[u8], ch: u8, flags: WildmatchFlags) -> bool {
    match class {
        b"alnum" => ch.is_ascii_alphanumeric(),
        b"alpha" => ch.is_ascii_alphabetic(),
        b"blank" => ch == b' ' || ch == b'\t',
        b"cntrl" => ch.is_ascii_control(),
        b"digit" => ch.is_ascii_digit(),
        b"graph" => ch.is_ascii_graphic(),
        b"lower" => ch.is_ascii_lowercase(),
        b"print" => ch.is_ascii_graphic() || ch == b' ',
        b"punct" => ch.is_ascii_punctuation(),
        b"space" => ch.is_ascii_whitespace(),
        b"upper" => {
            ch.is_ascii_uppercase()
                || (flags.contains(WildmatchFlags::CASEFOLD) && ch.is_ascii_lowercase())
        }
        b"xdigit" => ch.is_ascii_hexdigit(),
        _ => false, // Unknown class
    }
}

/// Compiled wildmatch pattern for efficient repeated matching.
#[derive(Debug, Clone)]
pub struct WildmatchPattern {
    pattern: BString,
    flags: WildmatchFlags,
}

impl WildmatchPattern {
    /// Create a new pattern.
    pub fn new(pattern: &BStr, flags: WildmatchFlags) -> Self {
        Self {
            pattern: pattern.into(),
            flags,
        }
    }

    /// Match against text. Returns true if the pattern matches.
    pub fn matches(&self, text: &BStr) -> bool {
        wildmatch(self.pattern.as_ref(), text, self.flags)
    }
}

/// Standalone match function matching C git's `wildmatch()`.
pub fn wildmatch(pattern: &BStr, text: &BStr, flags: WildmatchFlags) -> bool {
    let res = dowild(pattern.as_bytes(), text.as_bytes(), flags);
    res == WM_MATCH
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to test wildmatch with various flag combinations.
    /// Arguments mirror the test script: (glob, iglob, pathmatch, ipathmatch, text, pattern)
    fn match_test(
        glob: bool,
        iglob: bool,
        pathmatch: bool,
        ipathmatch: bool,
        text: &[u8],
        pattern: &[u8],
    ) {
        let text = BStr::new(text);
        let pat = BStr::new(pattern);

        // glob = WM_PATHNAME (no casefold)
        assert_eq!(
            wildmatch(pat, text, WildmatchFlags::PATHNAME),
            glob,
            "glob: pattern={:?} text={:?}",
            pat,
            text
        );

        // iglob = WM_PATHNAME | WM_CASEFOLD
        assert_eq!(
            wildmatch(pat, text, WildmatchFlags::PATHNAME | WildmatchFlags::CASEFOLD),
            iglob,
            "iglob: pattern={:?} text={:?}",
            pat,
            text
        );

        // pathmatch = no flags (neither PATHNAME nor CASEFOLD)
        assert_eq!(
            wildmatch(pat, text, WildmatchFlags::empty()),
            pathmatch,
            "pathmatch: pattern={:?} text={:?}",
            pat,
            text
        );

        // ipathmatch = WM_CASEFOLD
        assert_eq!(
            wildmatch(pat, text, WildmatchFlags::CASEFOLD),
            ipathmatch,
            "ipathmatch: pattern={:?} text={:?}",
            pat,
            text
        );
    }

    #[test]
    fn basic_literal() {
        match_test(true, true, true, true, b"foo", b"foo");
        match_test(false, false, false, false, b"foo", b"bar");
    }

    #[test]
    fn empty_strings() {
        match_test(true, true, true, true, b"", b"");
    }

    #[test]
    fn question_mark() {
        match_test(true, true, true, true, b"foo", b"???");
        match_test(false, false, false, false, b"foo", b"??");
    }

    #[test]
    fn single_star() {
        match_test(true, true, true, true, b"foo", b"*");
        match_test(true, true, true, true, b"foo", b"f*");
        match_test(false, false, false, false, b"foo", b"*f");
        match_test(true, true, true, true, b"foo", b"*foo*");
        match_test(true, true, true, true, b"foobar", b"*ob*a*r*");
        match_test(true, true, true, true, b"aaaaaaabababab", b"*ab");
    }

    #[test]
    fn backslash_escape() {
        match_test(true, true, true, true, b"foo*", b"foo\\*");
        match_test(false, false, false, false, b"foobar", b"foo\\*bar");
        match_test(true, true, true, true, b"f\\oo", b"f\\\\oo");
    }

    #[test]
    fn character_class() {
        match_test(true, true, true, true, b"ball", b"*[al]?");
        match_test(false, false, false, false, b"ten", b"[ten]");
        match_test(true, true, true, true, b"ten", b"**[!te]");
        match_test(false, false, false, false, b"ten", b"**[!ten]");
        match_test(true, true, true, true, b"ten", b"t[a-g]n");
        match_test(false, false, false, false, b"ten", b"t[!a-g]n");
        match_test(true, true, true, true, b"ton", b"t[!a-g]n");
        match_test(true, true, true, true, b"ton", b"t[^a-g]n");
    }

    #[test]
    fn bracket_special_chars() {
        match_test(true, true, true, true, b"a]b", b"a[]]b");
        match_test(true, true, true, true, b"a-b", b"a[]-]b");
        match_test(true, true, true, true, b"a]b", b"a[]-]b");
        match_test(false, false, false, false, b"aab", b"a[]-]b");
        match_test(true, true, true, true, b"aab", b"a[]a-]b");
        match_test(true, true, true, true, b"]", b"]");
    }

    #[test]
    fn slash_matching() {
        match_test(false, false, true, true, b"foo/baz/bar", b"foo*bar");
        match_test(false, false, true, true, b"foo/baz/bar", b"foo**bar");
        match_test(true, true, true, true, b"foobazbar", b"foo**bar");
        match_test(true, true, true, true, b"foo/baz/bar", b"foo/**/bar");
        match_test(true, true, true, true, b"foo/b/a/z/bar", b"foo/**/bar");
    }

    #[test]
    fn double_star_matching() {
        match_test(true, true, false, false, b"foo", b"**/foo");
        match_test(true, true, true, true, b"XXX/foo", b"**/foo");
        match_test(true, true, true, true, b"bar/baz/foo", b"**/foo");
        match_test(false, false, true, true, b"bar/baz/foo", b"*/foo");
    }

    #[test]
    fn path_separator() {
        match_test(false, false, true, true, b"foo/bar", b"foo?bar");
        match_test(false, false, true, true, b"foo/bar", b"foo[/]bar");
        match_test(false, false, true, true, b"foo/bar", b"foo[^a-z]bar");
    }

    #[test]
    fn posix_char_classes() {
        match_test(true, true, true, true, b"a1B", b"[[:alpha:]][[:digit:]][[:upper:]]");
        match_test(false, true, false, true, b"a", b"[[:digit:][:upper:][:space:]]");
        match_test(true, true, true, true, b"A", b"[[:digit:][:upper:][:space:]]");
        match_test(true, true, true, true, b"1", b"[[:digit:][:upper:][:space:]]");
        match_test(true, true, true, true, b" ", b"[[:digit:][:upper:][:space:]]");
        match_test(false, false, false, false, b".", b"[[:digit:][:upper:][:space:]]");
        match_test(true, true, true, true, b"5", b"[[:xdigit:]]");
        match_test(true, true, true, true, b"f", b"[[:xdigit:]]");
        match_test(true, true, true, true, b"D", b"[[:xdigit:]]");
    }

    #[test]
    fn case_sensitivity() {
        match_test(false, true, false, true, b"a", b"[A-Z]");
        match_test(true, true, true, true, b"A", b"[A-Z]");
        match_test(false, true, false, true, b"A", b"[a-z]");
        match_test(true, true, true, true, b"a", b"[a-z]");
        match_test(false, true, false, true, b"a", b"[[:upper:]]");
        match_test(true, true, true, true, b"A", b"[[:upper:]]");
    }

    #[test]
    fn recursion() {
        match_test(
            true,
            true,
            true,
            true,
            b"-adobe-courier-bold-o-normal--12-120-75-75-m-70-iso8859-1",
            b"-*-*-*-*-*-*-12-*-*-*-m-*-*-*",
        );
        match_test(
            true,
            true,
            true,
            true,
            b"abcd/abcdefg/abcdefghijk/abcdefghijklmnop.txt",
            b"**/*a*b*g*n*t",
        );
    }

    #[test]
    fn path_wildcard_combos() {
        match_test(false, false, false, false, b"foo", b"*/*/*");
        match_test(false, false, false, false, b"foo/bar", b"*/*/*");
        match_test(true, true, true, true, b"foo/bba/arr", b"*/*/*");
        match_test(false, false, true, true, b"foo/bb/aa/rr", b"*/*/*");
        match_test(true, true, true, true, b"foo/bb/aa/rr", b"**/**/**");
    }

    #[test]
    fn extra_pathmatch() {
        match_test(false, false, false, false, b"foo", b"fo");
        match_test(true, true, true, true, b"foo/bar", b"foo/bar");
        match_test(true, true, true, true, b"foo/bar", b"foo/*");
        match_test(false, false, true, true, b"foo/bba/arr", b"foo/*");
        match_test(true, true, true, true, b"foo/bba/arr", b"foo/**");
        match_test(false, false, true, true, b"foo/bba/arr", b"foo*");
    }

    #[test]
    fn misc_additional() {
        match_test(true, true, true, true, b"[ab]", b"\\[ab]");
        match_test(true, true, true, true, b"?a?b", b"\\??\\?b");
        match_test(true, true, true, true, b"abc", b"\\a\\b\\c");
    }

    #[test]
    fn double_star_bar() {
        match_test(true, true, true, true, b"deep/foo/bar/baz", b"**/bar/*");
        match_test(false, false, false, false, b"deep/foo/bar", b"**/bar/*");
        match_test(true, true, true, true, b"deep/foo/bar/", b"**/bar/**");
        match_test(true, true, true, true, b"foo/bar/baz/x", b"*/bar/**");
        match_test(false, false, true, true, b"deep/foo/bar/baz/x", b"*/bar/**");
        match_test(true, true, true, true, b"deep/foo/bar/baz/x", b"**/bar/*/*");
    }

    #[test]
    fn additional_range_tests() {
        match_test(true, true, true, true, b"-", b"[-]");
        match_test(true, true, true, true, b"-", b"[--A]");
        match_test(true, true, true, true, b"5", b"[--A]");
        match_test(true, true, true, true, b",", b"[,]");
        match_test(true, true, true, true, b"-", b"[,-.]");
        match_test(false, false, false, false, b"+", b"[,-.]");
    }

    #[test]
    fn compiled_pattern() {
        let pat = WildmatchPattern::new(BStr::new(b"foo*bar"), WildmatchFlags::PATHNAME);
        assert!(!pat.matches(BStr::new(b"foo/baz/bar")));
        assert!(pat.matches(BStr::new(b"foobazbar")));
    }
}

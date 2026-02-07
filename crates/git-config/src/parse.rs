//! Config file parser — handles git's INI-like format.

use bstr::{BString, ByteVec};
use crate::error::ConfigError;

/// UTF-8 BOM bytes.
const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

/// A parsed event from the config file, preserving raw text.
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// A section header: `[section]` or `[section "subsection"]`
    SectionHeader {
        /// The raw header line including brackets.
        raw: BString,
        /// Lowercased section name.
        section: BString,
        /// Case-preserved subsection (optional).
        subsection: Option<BString>,
    },
    /// A key-value entry.
    Entry {
        /// The raw line(s) as they appeared in the file.
        raw: BString,
        /// Lowercased key name.
        key: BString,
        /// Parsed value (None for boolean keys with no `=`).
        value: Option<BString>,
        /// Line number where this entry starts.
        line_number: usize,
    },
    /// A comment line (`#` or `;`).
    Comment(BString),
    /// A blank line.
    Blank(BString),
}

/// Parse a config file's bytes into a sequence of events.
pub fn parse_config(input: &[u8], filename: &str) -> Result<Vec<ConfigEvent>, ConfigError> {
    let mut events = Vec::new();
    let mut pos = 0;
    let mut line_number: usize = 1;

    // Skip UTF-8 BOM if present
    if input.starts_with(UTF8_BOM) {
        pos = UTF8_BOM.len();
    }

    while pos < input.len() {
        // Skip leading whitespace (but not newlines)
        let line_start = pos;
        while pos < input.len() && (input[pos] == b' ' || input[pos] == b'\t') {
            pos += 1;
        }

        if pos >= input.len() {
            // Trailing whitespace only
            let raw = BString::from(&input[line_start..pos]);
            if !raw.is_empty() {
                events.push(ConfigEvent::Blank(raw));
            }
            break;
        }

        match input[pos] {
            b'\n' => {
                let raw = BString::from(&input[line_start..=pos]);
                events.push(ConfigEvent::Blank(raw));
                pos += 1;
                line_number += 1;
            }
            b'\r' => {
                // Handle CR or CRLF
                if pos + 1 < input.len() && input[pos + 1] == b'\n' {
                    let raw = BString::from(&input[line_start..pos + 2]);
                    events.push(ConfigEvent::Blank(raw));
                    pos += 2;
                } else {
                    let raw = BString::from(&input[line_start..=pos]);
                    events.push(ConfigEvent::Blank(raw));
                    pos += 1;
                }
                line_number += 1;
            }
            b'#' | b';' => {
                let end = find_line_end(input, pos);
                line_number += count_newlines(&input[pos..end]);
                let newline_end = skip_newline(input, end);
                if newline_end > end {
                    line_number += 1;
                }
                let raw = BString::from(&input[line_start..newline_end]);
                events.push(ConfigEvent::Comment(raw));
                pos = newline_end;
            }
            b'[' => {
                let start_line = line_number;
                let (section, subsection, end) = parse_section_header(input, pos, filename, start_line)?;
                let newline_end = skip_newline(input, end);
                let raw = BString::from(&input[line_start..newline_end]);
                line_number += count_newlines(&input[pos..newline_end]);
                events.push(ConfigEvent::SectionHeader {
                    raw,
                    section,
                    subsection,
                });
                pos = newline_end;
            }
            _ => {
                // Key-value entry
                let start_line = line_number;
                let (key, value, end, newlines) =
                    parse_key_value(input, line_start, filename, start_line)?;
                let raw = BString::from(&input[line_start..end]);
                events.push(ConfigEvent::Entry {
                    raw,
                    key,
                    value,
                    line_number: start_line,
                });
                line_number += newlines;
                pos = end;
            }
        }
    }

    Ok(events)
}

/// Parse a section header starting at `[`.
/// Returns (section_name, subsection_name, end_position).
fn parse_section_header(
    input: &[u8],
    start: usize,
    filename: &str,
    line: usize,
) -> Result<(BString, Option<BString>, usize), ConfigError> {
    let mut pos = start + 1; // skip '['

    // Parse section name (alphanumeric and hyphen, case-insensitive)
    let section_start = pos;
    while pos < input.len() && is_section_char(input[pos]) {
        pos += 1;
    }

    if pos == section_start {
        return Err(ConfigError::Parse {
            file: filename.to_string(),
            line,
            message: "empty section name".into(),
        });
    }

    let section = BString::from(
        input[section_start..pos]
            .iter()
            .map(|b| b.to_ascii_lowercase())
            .collect::<Vec<u8>>(),
    );

    // Check for subsection
    let subsection = if pos < input.len() && (input[pos] == b' ' || input[pos] == b'\t') {
        // Skip whitespace before quote
        while pos < input.len() && (input[pos] == b' ' || input[pos] == b'\t') {
            pos += 1;
        }

        if pos >= input.len() || input[pos] != b'"' {
            return Err(ConfigError::Parse {
                file: filename.to_string(),
                line,
                message: "expected '\"' for subsection".into(),
            });
        }
        pos += 1; // skip opening quote

        let mut subsection = BString::new(Vec::new());
        while pos < input.len() && input[pos] != b'"' {
            if input[pos] == b'\\' {
                pos += 1;
                if pos >= input.len() {
                    return Err(ConfigError::Parse {
                        file: filename.to_string(),
                        line,
                        message: "unterminated escape in subsection".into(),
                    });
                }
                // In subsection, backslash escapes any character
                subsection.push_byte(input[pos]);
            } else if input[pos] == b'\n' {
                return Err(ConfigError::Parse {
                    file: filename.to_string(),
                    line,
                    message: "newline in subsection name".into(),
                });
            } else {
                subsection.push_byte(input[pos]);
            }
            pos += 1;
        }

        if pos >= input.len() || input[pos] != b'"' {
            return Err(ConfigError::Parse {
                file: filename.to_string(),
                line,
                message: "unterminated subsection quote".into(),
            });
        }
        pos += 1; // skip closing quote

        Some(subsection)
    } else {
        None
    };

    // Expect ']'
    if pos >= input.len() || input[pos] != b']' {
        return Err(ConfigError::Parse {
            file: filename.to_string(),
            line,
            message: "expected ']' to close section header".into(),
        });
    }
    pos += 1; // skip ']'

    // Skip to end of line (ignore trailing whitespace/comments)
    while pos < input.len() && input[pos] != b'\n' && input[pos] != b'\r' {
        if input[pos] == b'#' || input[pos] == b';' {
            // comment to end of line
            while pos < input.len() && input[pos] != b'\n' && input[pos] != b'\r' {
                pos += 1;
            }
            break;
        }
        if input[pos] != b' ' && input[pos] != b'\t' {
            return Err(ConfigError::Parse {
                file: filename.to_string(),
                line,
                message: format!(
                    "unexpected character after section header: {:?}",
                    input[pos] as char
                ),
            });
        }
        pos += 1;
    }

    Ok((section, subsection, pos))
}

/// Parse a key-value line.
/// Returns (key, value, end_position, newline_count).
fn parse_key_value(
    input: &[u8],
    start: usize,
    filename: &str,
    line: usize,
) -> Result<(BString, Option<BString>, usize, usize), ConfigError> {
    let mut pos = start;
    let mut newlines: usize = 0;

    // Skip leading whitespace
    while pos < input.len() && (input[pos] == b' ' || input[pos] == b'\t') {
        pos += 1;
    }

    // Parse key name (alphanumeric and hyphen)
    let key_start = pos;
    while pos < input.len() && is_key_char(input[pos]) {
        pos += 1;
    }

    if pos == key_start {
        return Err(ConfigError::Parse {
            file: filename.to_string(),
            line,
            message: "empty key name".into(),
        });
    }

    let key = BString::from(
        input[key_start..pos]
            .iter()
            .map(|b| b.to_ascii_lowercase())
            .collect::<Vec<u8>>(),
    );

    // Skip whitespace after key
    while pos < input.len() && (input[pos] == b' ' || input[pos] == b'\t') {
        pos += 1;
    }

    // Check for = sign
    if pos >= input.len() || input[pos] == b'\n' || input[pos] == b'\r' || input[pos] == b'#' || input[pos] == b';' {
        // Boolean key with no value
        let end = skip_to_line_end_and_newline(input, pos, &mut newlines);
        return Ok((key, None, end, newlines));
    }

    if input[pos] != b'=' {
        return Err(ConfigError::Parse {
            file: filename.to_string(),
            line,
            message: format!("expected '=' after key, got {:?}", input[pos] as char),
        });
    }
    pos += 1; // skip '='

    // Skip whitespace after =
    while pos < input.len() && (input[pos] == b' ' || input[pos] == b'\t') {
        pos += 1;
    }

    // Parse value
    let (value, end, value_newlines) = parse_value(input, pos, filename, line)?;
    newlines += value_newlines;

    Ok((key, Some(value), end, newlines))
}

/// Parse a config value, handling quoting, escapes, and line continuations.
/// Returns (parsed_value, end_position, newline_count).
fn parse_value(
    input: &[u8],
    start: usize,
    filename: &str,
    line: usize,
) -> Result<(BString, usize, usize), ConfigError> {
    let mut pos = start;
    let mut value = BString::new(Vec::new());
    let mut in_quote = false;
    let mut newlines: usize = 0;

    while pos < input.len() {
        let ch = input[pos];

        match ch {
            b'\n' => {
                if in_quote {
                    return Err(ConfigError::Parse {
                        file: filename.to_string(),
                        line: line + newlines,
                        message: "newline inside quoted string".into(),
                    });
                }
                // End of value - consume the newline
                pos += 1;
                newlines += 1;
                break;
            }
            b'\r' => {
                if in_quote {
                    return Err(ConfigError::Parse {
                        file: filename.to_string(),
                        line: line + newlines,
                        message: "newline inside quoted string".into(),
                    });
                }
                // CRLF or lone CR
                pos += 1;
                if pos < input.len() && input[pos] == b'\n' {
                    pos += 1;
                }
                newlines += 1;
                break;
            }
            b'\\' => {
                pos += 1;
                if pos >= input.len() {
                    return Err(ConfigError::Parse {
                        file: filename.to_string(),
                        line: line + newlines,
                        message: "backslash at end of file".into(),
                    });
                }
                match input[pos] {
                    b'\n' => {
                        // Line continuation
                        pos += 1;
                        newlines += 1;
                    }
                    b'\r' => {
                        // Line continuation (CRLF)
                        pos += 1;
                        if pos < input.len() && input[pos] == b'\n' {
                            pos += 1;
                        }
                        newlines += 1;
                    }
                    b'n' => {
                        value.push_byte(b'\n');
                        pos += 1;
                    }
                    b't' => {
                        value.push_byte(b'\t');
                        pos += 1;
                    }
                    b'b' => {
                        value.push_byte(b'\x08');
                        pos += 1;
                    }
                    b'\\' => {
                        value.push_byte(b'\\');
                        pos += 1;
                    }
                    b'"' => {
                        value.push_byte(b'"');
                        pos += 1;
                    }
                    other => {
                        return Err(ConfigError::Parse {
                            file: filename.to_string(),
                            line: line + newlines,
                            message: format!("invalid escape sequence: \\{}", other as char),
                        });
                    }
                }
            }
            b'"' => {
                in_quote = !in_quote;
                pos += 1;
            }
            b'#' | b';' => {
                if in_quote {
                    value.push_byte(ch);
                    pos += 1;
                } else {
                    // Comment — skip to end of line
                    while pos < input.len() && input[pos] != b'\n' && input[pos] != b'\r' {
                        pos += 1;
                    }
                    // Consume the newline
                    if pos < input.len() {
                        if input[pos] == b'\r' {
                            pos += 1;
                            if pos < input.len() && input[pos] == b'\n' {
                                pos += 1;
                            }
                        } else {
                            pos += 1;
                        }
                        newlines += 1;
                    }
                    break;
                }
            }
            _ => {
                value.push_byte(ch);
                pos += 1;
            }
        }
    }

    // Trim trailing whitespace from unquoted portions.
    // We do this at the end: the value has already consumed everything,
    // so trailing spaces before a comment or newline should be stripped.
    let trimmed = value.as_ref() as &[u8];
    let trimmed = trimmed
        .iter()
        .rposition(|b| *b != b' ' && *b != b'\t')
        .map(|p| &trimmed[..=p])
        .unwrap_or(b"");
    let value = BString::from(trimmed);

    Ok((value, pos, newlines))
}

/// Check if a byte is valid in a section name (alphanumeric, hyphen, dot).
fn is_section_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'.'
}

/// Check if a byte is valid in a key name (alphanumeric, hyphen).
fn is_key_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
}

/// Find the end of the current line (position of \n or \r or end of input).
fn find_line_end(input: &[u8], start: usize) -> usize {
    let mut pos = start;
    while pos < input.len() && input[pos] != b'\n' && input[pos] != b'\r' {
        pos += 1;
    }
    pos
}

/// Skip past newline characters (LF, CR, CRLF).
fn skip_newline(input: &[u8], pos: usize) -> usize {
    if pos >= input.len() {
        return pos;
    }
    if input[pos] == b'\r' {
        if pos + 1 < input.len() && input[pos + 1] == b'\n' {
            pos + 2
        } else {
            pos + 1
        }
    } else if input[pos] == b'\n' {
        pos + 1
    } else {
        pos
    }
}

/// Skip to end of line and past the newline, counting newlines.
fn skip_to_line_end_and_newline(input: &[u8], start: usize, newlines: &mut usize) -> usize {
    let end = find_line_end(input, start);
    let pos = skip_newline(input, end);
    if pos > end {
        *newlines += 1;
    }
    pos
}

/// Count newlines in a byte slice.
fn count_newlines(data: &[u8]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\r' {
            count += 1;
            if i + 1 < data.len() && data[i + 1] == b'\n' {
                i += 1;
            }
        } else if data[i] == b'\n' {
            count += 1;
        }
        i += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use bstr::{BStr, ByteSlice};

    #[test]
    fn parse_empty() {
        let events = parse_config(b"", "<test>").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn parse_simple_section() {
        let input = b"[core]\n\tbare = false\n";
        let events = parse_config(input, "<test>").unwrap();
        assert_eq!(events.len(), 2);
        match &events[0] {
            ConfigEvent::SectionHeader { section, subsection, .. } => {
                assert_eq!(&**section, b"core");
                assert!(subsection.is_none());
            }
            _ => panic!("expected SectionHeader"),
        }
        match &events[1] {
            ConfigEvent::Entry { key, value, .. } => {
                assert_eq!(&**key, b"bare");
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("false")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_subsection() {
        let input = b"[remote \"origin\"]\n\turl = https://example.com\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[0] {
            ConfigEvent::SectionHeader { section, subsection, .. } => {
                assert_eq!(&**section, b"remote");
                assert_eq!(subsection.as_deref().map(|v| v.as_bstr()), Some(BStr::new("origin")));
            }
            _ => panic!("expected SectionHeader"),
        }
    }

    #[test]
    fn parse_subsection_with_escape() {
        let input = b"[url \"https://github.com/\"]\n\tinsteadOf = gh:\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[0] {
            ConfigEvent::SectionHeader { section, subsection, .. } => {
                assert_eq!(&**section, b"url");
                assert_eq!(
                    subsection.as_deref().map(|v| v.as_bstr()),
                    Some(BStr::new("https://github.com/"))
                );
            }
            _ => panic!("expected SectionHeader"),
        }
    }

    #[test]
    fn parse_boolean_key_no_value() {
        let input = b"[core]\n\tbare\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { key, value, .. } => {
                assert_eq!(&**key, b"bare");
                assert!(value.is_none());
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_comments() {
        let input = b"# A comment\n; Another comment\n[core]\n";
        let events = parse_config(input, "<test>").unwrap();
        assert!(matches!(&events[0], ConfigEvent::Comment(_)));
        assert!(matches!(&events[1], ConfigEvent::Comment(_)));
        assert!(matches!(&events[2], ConfigEvent::SectionHeader { .. }));
    }

    #[test]
    fn parse_line_continuation() {
        let input = b"[core]\n\tkey = hello \\\n\t\tworld\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("hello \t\tworld")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_quoted_value() {
        let input = b"[core]\n\tkey = \"hello world\"\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("hello world")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_escape_sequences() {
        let input = b"[core]\n\tkey = \"hello\\nworld\\t!\"\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("hello\nworld\t!")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_inline_comment() {
        let input = b"[core]\n\tkey = value # inline comment\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("value")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_inline_comment_semicolon() {
        let input = b"[core]\n\tkey = value ; inline comment\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("value")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_comment_in_quotes_preserved() {
        let input = b"[core]\n\tkey = \"value # not a comment\"\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("value # not a comment")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_bom() {
        let mut input = Vec::from(UTF8_BOM);
        input.extend_from_slice(b"[core]\n\tbare = true\n");
        let events = parse_config(&input, "<test>").unwrap();
        match &events[0] {
            ConfigEvent::SectionHeader { section, .. } => {
                assert_eq!(&**section, b"core");
            }
            _ => panic!("expected SectionHeader"),
        }
    }

    #[test]
    fn parse_crlf() {
        let input = b"[core]\r\n\tbare = false\r\n";
        let events = parse_config(input, "<test>").unwrap();
        assert_eq!(events.len(), 2);
        match &events[1] {
            ConfigEvent::Entry { key, value, .. } => {
                assert_eq!(&**key, b"bare");
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("false")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_mixed_line_endings() {
        let input = b"[core]\n\ta = 1\r\n\tb = 2\r\tc = 3\n";
        let events = parse_config(input, "<test>").unwrap();
        let entry_count = events
            .iter()
            .filter(|e| matches!(e, ConfigEvent::Entry { .. }))
            .count();
        assert_eq!(entry_count, 3);
    }

    #[test]
    fn parse_case_insensitive_section() {
        let input = b"[CoRe]\n\tBaRe = false\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[0] {
            ConfigEvent::SectionHeader { section, .. } => {
                assert_eq!(&**section, b"core");
            }
            _ => panic!("expected SectionHeader"),
        }
        match &events[1] {
            ConfigEvent::Entry { key, .. } => {
                assert_eq!(&**key, b"bare");
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_empty_value() {
        let input = b"[core]\n\tkey =\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_value_with_trailing_whitespace() {
        let input = b"[core]\n\tkey = value   \n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[1] {
            ConfigEvent::Entry { value, .. } => {
                assert_eq!(value.as_deref().map(|v| v.as_bstr()), Some(BStr::new("value")));
            }
            _ => panic!("expected Entry"),
        }
    }

    #[test]
    fn parse_blank_lines() {
        let input = b"\n[core]\n\n\tbare = true\n\n";
        let events = parse_config(input, "<test>").unwrap();
        let blank_count = events
            .iter()
            .filter(|e| matches!(e, ConfigEvent::Blank(_)))
            .count();
        assert!(blank_count >= 2);
    }

    #[test]
    fn parse_section_with_trailing_comment() {
        let input = b"[core] # comment\n\tbare = true\n";
        let events = parse_config(input, "<test>").unwrap();
        match &events[0] {
            ConfigEvent::SectionHeader { section, .. } => {
                assert_eq!(&**section, b"core");
            }
            _ => panic!("expected SectionHeader"),
        }
    }

    #[test]
    fn parse_invalid_escape() {
        let input = b"[core]\n\tkey = \"\\x\"\n";
        assert!(parse_config(input, "<test>").is_err());
    }

    #[test]
    fn parse_multiple_sections() {
        let input = b"[user]\n\tname = Alice\n[core]\n\tbare = false\n";
        let events = parse_config(input, "<test>").unwrap();
        let sections: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                ConfigEvent::SectionHeader { section, .. } => Some(section.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(sections.len(), 2);
        assert_eq!(&*sections[0], b"user");
        assert_eq!(&*sections[1], b"core");
    }
}

//! Date compatibility tests comparing our parsing/formatting with C git behavior.

use bstr::BStr;
use git_utils::date::{DateFormat, GitDate, Signature};

/// Verify raw format round-trips correctly.
#[test]
fn raw_format_roundtrip() {
    let inputs = [
        "1234567890 +0000",
        "1234567890 -0500",
        "1234567890 +0530",
        "0 +0000",
        "1700000000 +1200",
        "1700000000 -1100",
    ];

    for input in inputs {
        let date = GitDate::parse_raw(input).unwrap();
        let formatted = date.format(DateFormat::Raw);
        let reparsed = GitDate::parse_raw(&formatted).unwrap();

        assert_eq!(
            date.timestamp, reparsed.timestamp,
            "timestamp mismatch for input: {}",
            input
        );
        assert_eq!(
            date.tz_offset, reparsed.tz_offset,
            "tz_offset mismatch for input: {}",
            input
        );
    }
}

/// Verify signature round-trips through parse -> to_bytes -> parse.
#[test]
fn signature_roundtrip() {
    let inputs = [
        "John Doe <john@example.com> 1234567890 +0000",
        "Jane Smith <jane@test.org> 1700000000 -0500",
        "A B C <abc@d.e> 0 +0000",
    ];

    for input in inputs {
        let sig = Signature::parse(BStr::new(input.as_bytes())).unwrap();
        let bytes = sig.to_bytes();
        let reparsed = Signature::parse(BStr::new(&bytes)).unwrap();

        assert_eq!(sig.name, reparsed.name, "name mismatch for: {}", input);
        assert_eq!(sig.email, reparsed.email, "email mismatch for: {}", input);
        assert_eq!(
            sig.date.timestamp, reparsed.date.timestamp,
            "timestamp mismatch for: {}",
            input
        );
        assert_eq!(
            sig.date.tz_offset, reparsed.date.tz_offset,
            "tz_offset mismatch for: {}",
            input
        );
    }
}

/// Verify that ISO format produces valid output.
#[test]
fn iso_format_structure() {
    let date = GitDate::parse_raw("1234567890 +0000").unwrap();
    let formatted = date.format(DateFormat::Iso);

    // ISO format should be: YYYY-MM-DD HH:MM:SS +ZZZZ
    assert!(
        formatted.len() >= 25,
        "ISO format too short: {}",
        formatted
    );
    assert_eq!(&formatted[4..5], "-", "ISO missing first dash");
    assert_eq!(&formatted[7..8], "-", "ISO missing second dash");
    assert_eq!(&formatted[10..11], " ", "ISO missing space");
    assert_eq!(&formatted[13..14], ":", "ISO missing first colon");
    assert_eq!(&formatted[16..17], ":", "ISO missing second colon");
}

/// Verify that IsoStrict format is valid ISO 8601.
#[test]
fn iso_strict_format_structure() {
    let date = GitDate::parse_raw("1234567890 +0000").unwrap();
    let formatted = date.format(DateFormat::IsoStrict);

    // Should contain a T separator
    assert!(
        formatted.contains('T'),
        "IsoStrict should contain T: {}",
        formatted
    );
}

/// Verify that Short format is YYYY-MM-DD.
#[test]
fn short_format_structure() {
    let date = GitDate::parse_raw("1234567890 +0000").unwrap();
    let formatted = date.format(DateFormat::Short);

    assert_eq!(formatted.len(), 10, "Short format should be 10 chars: {}", formatted);
    assert_eq!(&formatted[4..5], "-");
    assert_eq!(&formatted[7..8], "-");
}

/// Verify that Unix format is just the timestamp.
#[test]
fn unix_format_is_timestamp() {
    let date = GitDate::parse_raw("1234567890 +0000").unwrap();
    let formatted = date.format(DateFormat::Unix);
    assert_eq!(formatted, "1234567890");
}

/// Verify that @timestamp parsing works.
#[test]
fn at_timestamp_parse() {
    let date = GitDate::parse("@1234567890").unwrap();
    assert_eq!(date.timestamp, 1234567890);
}

/// Verify various timezone offsets.
#[test]
fn timezone_offsets() {
    let cases = [
        ("+0000", 0),
        ("-0500", -300),
        ("+0530", 330),
        ("+1200", 720),
        ("-1100", -660),
        ("+0100", 60),
        ("-0800", -480),
    ];

    for (tz_str, expected_minutes) in cases {
        let input = format!("1234567890 {}", tz_str);
        let date = GitDate::parse_raw(&input).unwrap();
        assert_eq!(
            date.tz_offset, expected_minutes,
            "tz_offset mismatch for {}",
            tz_str
        );
    }
}

/// Verify parse handles edge case timestamps.
#[test]
fn edge_timestamps() {
    // Epoch
    let date = GitDate::parse_raw("0 +0000").unwrap();
    assert_eq!(date.timestamp, 0);

    // Very large timestamp (year ~2106)
    let date = GitDate::parse_raw("4294967295 +0000").unwrap();
    assert_eq!(date.timestamp, 4294967295);
}

/// Verify RFC2822 format output.
#[test]
fn rfc2822_format() {
    let date = GitDate::parse_raw("1234567890 +0000").unwrap();
    let formatted = date.format(DateFormat::Rfc2822);

    // RFC2822 should contain day-of-week abbreviation and month abbreviation
    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let has_weekday = weekdays.iter().any(|d| formatted.contains(d));
    let has_month = months.iter().any(|m| formatted.contains(m));

    assert!(has_weekday, "RFC2822 should contain weekday: {}", formatted);
    assert!(has_month, "RFC2822 should contain month: {}", formatted);
}

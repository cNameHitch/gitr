use bstr::{BStr, BString, ByteSlice, ByteVec};
use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, TimeZone, Utc};

use crate::error::UtilError;
use crate::Result;

/// A parsed git date with timezone information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitDate {
    /// Seconds since Unix epoch.
    pub timestamp: i64,
    /// Timezone offset in minutes from UTC (e.g., -300 for EST).
    pub tz_offset: i32,
}

/// Supported date output formats matching C git's `date_mode_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// "2 hours ago"
    Relative,
    /// Locale-dependent local time
    Local,
    /// ISO 8601 like: "2025-01-15 12:00:00 +0000"
    Iso,
    /// ISO 8601 strict: "2025-01-15T12:00:00+00:00"
    IsoStrict,
    /// RFC 2822: "Wed, 15 Jan 2025 12:00:00 +0000"
    Rfc2822,
    /// Short: "2025-01-15"
    Short,
    /// Raw: "1736942400 +0000"
    Raw,
    /// Human-readable (relative for recent, absolute for old)
    Human,
    /// Unix timestamp only
    Unix,
    /// C git default: "Thu Feb 13 23:31:30 2009 +0000" using commit's stored tz_offset
    Default,
}

/// Git timezone offset stored as integer (e.g. -0500 for EST = -500 integer).
/// This is the same format C git uses: the "decimal parse" where -0100 => -100.
fn tz_offset_to_minutes(tz: i32) -> i32 {
    let sign = if tz < 0 { -1 } else { 1 };
    let abs = tz.unsigned_abs() as i32;
    let hours = abs / 100;
    let mins = abs % 100;
    sign * (hours * 60 + mins)
}

/// Convert minutes offset to the git-style decimal representation.
fn minutes_to_tz_offset(minutes: i32) -> i32 {
    let sign = if minutes < 0 { -1 } else { 1 };
    let abs = minutes.unsigned_abs() as i32;
    let hours = abs / 60;
    let mins = abs % 60;
    sign * (hours * 100 + mins)
}

impl GitDate {
    /// Create a GitDate from a Unix timestamp and timezone offset in minutes.
    pub fn new(timestamp: i64, tz_offset_minutes: i32) -> Self {
        Self {
            timestamp,
            tz_offset: tz_offset_minutes,
        }
    }

    /// Get the current time as a GitDate with local timezone.
    pub fn now() -> Self {
        let now = Local::now();
        let offset_secs = now.offset().local_minus_utc();
        let offset_minutes = offset_secs / 60;
        Self {
            timestamp: now.timestamp(),
            tz_offset: offset_minutes,
        }
    }

    /// Parse a date string in any git-recognized format.
    ///
    /// Supports:
    /// - Raw git format: "1234567890 +0000"
    /// - ISO 8601: "2025-01-15T12:00:00+00:00" or "2025-01-15 12:00:00 +0000"
    /// - RFC 2822: "Wed, 15 Jan 2025 12:00:00 +0000"
    /// - Unix timestamp: "@1234567890"
    /// - Short date: "2025-01-15"
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        if input.is_empty() {
            return Err(UtilError::DateParse("empty date string".into()));
        }

        // Try "@timestamp" format
        if let Some(ts_str) = input.strip_prefix('@') {
            return Self::parse_raw(ts_str);
        }

        // Try raw git format: "timestamp +/-offset"
        if let Ok(date) = Self::parse_raw(input) {
            return Ok(date);
        }

        // Try ISO 8601 strict: "2025-01-15T12:00:00+00:00"
        if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
            let offset_secs = dt.offset().local_minus_utc();
            return Ok(Self {
                timestamp: dt.timestamp(),
                tz_offset: offset_secs / 60,
            });
        }

        // Try RFC 2822: "Wed, 15 Jan 2025 12:00:00 +0000"
        if let Ok(dt) = DateTime::parse_from_rfc2822(input) {
            let offset_secs = dt.offset().local_minus_utc();
            return Ok(Self {
                timestamp: dt.timestamp(),
                tz_offset: offset_secs / 60,
            });
        }

        // Try ISO 8601 git-style: "2025-01-15 12:00:00 +0000"
        if let Ok(dt) = DateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S %z") {
            let offset_secs = dt.offset().local_minus_utc();
            return Ok(Self {
                timestamp: dt.timestamp(),
                tz_offset: offset_secs / 60,
            });
        }

        // Try short date: "2025-01-15"
        if let Ok(nd) = NaiveDateTime::parse_from_str(
            &format!("{} 00:00:00", input),
            "%Y-%m-%d %H:%M:%S",
        ) {
            // Use local timezone for bare dates
            let local = Local::now();
            let offset_secs = local.offset().local_minus_utc();
            let offset_minutes = offset_secs / 60;
            let offset = FixedOffset::east_opt(offset_secs).unwrap_or(FixedOffset::east_opt(0).unwrap());
            if let Some(dt) = offset.from_local_datetime(&nd).earliest() {
                return Ok(Self {
                    timestamp: dt.timestamp(),
                    tz_offset: offset_minutes,
                });
            }
        }

        Err(UtilError::DateParse(format!(
            "unable to parse date: '{}'",
            input
        )))
    }

    /// Parse raw git format: "timestamp +/-offset" or just "timestamp".
    pub fn parse_raw(input: &str) -> Result<Self> {
        let input = input.trim();

        let parts: Vec<&str> = input.splitn(2, ' ').collect();

        let timestamp: i64 = parts[0].parse().map_err(|_| {
            UtilError::DateParse(format!("invalid timestamp: '{}'", parts[0]))
        })?;

        let tz_offset = if parts.len() > 1 {
            let tz_str = parts[1].trim();
            let tz_int: i32 = tz_str
                .parse()
                .map_err(|_| UtilError::DateParse(format!("invalid timezone: '{}'", tz_str)))?;
            tz_offset_to_minutes(tz_int)
        } else {
            0
        };

        Ok(Self {
            timestamp,
            tz_offset,
        })
    }

    /// Parse "approxidate" format used by --since/--until.
    ///
    /// Supports relative dates like "2 weeks ago", "yesterday", "3 days ago".
    pub fn parse_approxidate(input: &str) -> Result<Self> {
        let input = input.trim().to_lowercase();

        // Try standard parse first
        if let Ok(date) = Self::parse(&input) {
            return Ok(date);
        }

        let now = Utc::now();

        // "now"
        if input == "now" {
            return Ok(Self::now());
        }

        // "yesterday"
        if input == "yesterday" {
            let ts = now.timestamp() - 86400;
            let local = Local::now();
            return Ok(Self {
                timestamp: ts,
                tz_offset: local.offset().local_minus_utc() / 60,
            });
        }

        // "N <unit> ago" patterns
        if let Some(rest) = input.strip_suffix(" ago") {
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() == 2 {
                if let Ok(count) = parts[0].parse::<i64>() {
                    let seconds = match parts[1].trim_end_matches('s') {
                        "second" => count,
                        "minute" => count * 60,
                        "hour" => count * 3600,
                        "day" => count * 86400,
                        "week" => count * 7 * 86400,
                        "month" => count * 30 * 86400,
                        "year" => count * 365 * 86400,
                        _ => {
                            return Err(UtilError::DateParse(format!(
                                "unknown time unit: '{}'",
                                parts[1]
                            )));
                        }
                    };
                    let ts = now.timestamp() - seconds;
                    let local = Local::now();
                    return Ok(Self {
                        timestamp: ts,
                        tz_offset: local.offset().local_minus_utc() / 60,
                    });
                }
            }
        }

        Err(UtilError::DateParse(format!(
            "unable to parse approxidate: '{}'",
            input
        )))
    }

    /// Format in the given style.
    pub fn format(&self, fmt: DateFormat) -> String {
        match fmt {
            DateFormat::Raw => {
                let tz = minutes_to_tz_offset(self.tz_offset);
                format!("{} {:+05}", self.timestamp, tz)
            }
            DateFormat::Unix => {
                format!("{}", self.timestamp)
            }
            DateFormat::Relative => self.format_relative(),
            DateFormat::Human => self.format_human(),
            _ => {
                let offset = FixedOffset::east_opt(self.tz_offset * 60)
                    .unwrap_or(FixedOffset::east_opt(0).unwrap());
                let dt = DateTime::from_timestamp(self.timestamp, 0)
                    .unwrap_or(DateTime::UNIX_EPOCH)
                    .with_timezone(&offset);

                match fmt {
                    DateFormat::Default => dt.format("%a %b %e %H:%M:%S %Y %z").to_string(),
                    DateFormat::Iso => dt.format("%Y-%m-%d %H:%M:%S %z").to_string(),
                    DateFormat::IsoStrict => dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
                    DateFormat::Rfc2822 => dt.format("%a, %d %b %Y %H:%M:%S %z").to_string(),
                    DateFormat::Short => dt.format("%Y-%m-%d").to_string(),
                    DateFormat::Local => {
                        let local_dt = DateTime::from_timestamp(self.timestamp, 0)
                            .unwrap_or(DateTime::UNIX_EPOCH)
                            .with_timezone(&Local);
                        local_dt.format("%a %b %e %H:%M:%S %Y").to_string()
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    /// Format as a relative time string like "2 hours ago".
    fn format_relative(&self) -> String {
        let now = Utc::now().timestamp();
        let diff = now - self.timestamp;

        if diff < 0 {
            return "in the future".to_string();
        }

        let diff = diff as u64;
        if diff < 2 {
            return "just now".to_string();
        }
        if diff < 60 {
            return format!("{} seconds ago", diff);
        }
        if diff < 120 {
            return "1 minute ago".to_string();
        }
        if diff < 3600 {
            return format!("{} minutes ago", diff / 60);
        }
        if diff < 7200 {
            return "1 hour ago".to_string();
        }
        if diff < 86400 {
            return format!("{} hours ago", diff / 3600);
        }
        if diff < 172800 {
            return "1 day ago".to_string();
        }
        if diff < 7 * 86400 {
            return format!("{} days ago", diff / 86400);
        }
        if diff < 14 * 86400 {
            return "1 week ago".to_string();
        }
        if diff < 30 * 86400 {
            return format!("{} weeks ago", diff / (7 * 86400));
        }
        if diff < 60 * 86400 {
            return "1 month ago".to_string();
        }
        if diff < 365 * 86400 {
            return format!("{} months ago", diff / (30 * 86400));
        }
        if diff < 2 * 365 * 86400 {
            return "1 year ago".to_string();
        }

        let years = diff / (365 * 86400);
        let months = (diff % (365 * 86400)) / (30 * 86400);
        if months > 0 {
            format!("{} years, {} months ago", years, months)
        } else {
            format!("{} years ago", years)
        }
    }

    /// Format as human-readable (relative for recent, absolute for old).
    fn format_human(&self) -> String {
        let now = Utc::now().timestamp();
        let diff = now - self.timestamp;

        // If within the last week, use relative
        if (0..7 * 86400).contains(&diff) {
            self.format_relative()
        } else {
            // Use short date for older dates
            self.format(DateFormat::Iso)
        }
    }

    /// Convert to a chrono DateTime with the stored timezone.
    pub fn to_datetime(&self) -> Option<DateTime<FixedOffset>> {
        let offset = FixedOffset::east_opt(self.tz_offset * 60)?;
        DateTime::from_timestamp(self.timestamp, 0).map(|dt| dt.with_timezone(&offset))
    }
}

/// Author/committer identity with timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: BString,
    pub email: BString,
    pub date: GitDate,
}

impl Signature {
    /// Parse from git format: `Name <email> timestamp tz`
    ///
    /// Example: "John Doe <john@example.com> 1234567890 +0000"
    pub fn parse(input: &BStr) -> Result<Self> {
        let input = input.as_bytes();

        // Find the last '>' to split off the date portion
        let gt_pos = input
            .iter()
            .rposition(|&b| b == b'>')
            .ok_or_else(|| UtilError::DateParse("missing '>' in signature".into()))?;

        // Find the '<' for the email
        let lt_pos = input[..gt_pos]
            .iter()
            .rposition(|&b| b == b'<')
            .ok_or_else(|| UtilError::DateParse("missing '<' in signature".into()))?;

        // Name is everything before '<', trimmed
        let name = &input[..lt_pos];
        let name = name.trim();

        // Email is between '<' and '>'
        let email = &input[lt_pos + 1..gt_pos];

        // Date is everything after '>'
        let date_str = &input[gt_pos + 1..];
        let date_str = date_str.trim();
        let date_str = std::str::from_utf8(date_str).map_err(|_| {
            UtilError::DateParse("non-UTF-8 date in signature".into())
        })?;

        let date = GitDate::parse_raw(date_str)?;

        Ok(Self {
            name: BString::from(name),
            email: BString::from(email),
            date,
        })
    }

    /// Format in git's canonical format: `Name <email> timestamp tz`
    pub fn to_bytes(&self) -> BString {
        let tz = minutes_to_tz_offset(self.date.tz_offset);
        let mut out = BString::new(Vec::new());
        out.push_str(&self.name);
        out.push_str(b" <");
        out.push_str(&self.email);
        out.push_str(b"> ");
        out.push_str(format!("{} {:+05}", self.date.timestamp, tz).as_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_raw() {
        let d = GitDate::parse_raw("1234567890 +0000").unwrap();
        assert_eq!(d.timestamp, 1234567890);
        assert_eq!(d.tz_offset, 0);
    }

    #[test]
    fn parse_raw_negative_tz() {
        let d = GitDate::parse_raw("1234567890 -0500").unwrap();
        assert_eq!(d.timestamp, 1234567890);
        assert_eq!(d.tz_offset, -300); // -5 hours = -300 minutes
    }

    #[test]
    fn parse_raw_positive_tz() {
        let d = GitDate::parse_raw("1234567890 +0530").unwrap();
        assert_eq!(d.timestamp, 1234567890);
        assert_eq!(d.tz_offset, 330); // 5.5 hours = 330 minutes
    }

    #[test]
    fn parse_at_timestamp() {
        let d = GitDate::parse("@1234567890").unwrap();
        assert_eq!(d.timestamp, 1234567890);
        assert_eq!(d.tz_offset, 0);
    }

    #[test]
    fn parse_iso8601() {
        let d = GitDate::parse("2025-01-15T12:00:00+00:00").unwrap();
        assert_eq!(d.timestamp, 1736942400);
        assert_eq!(d.tz_offset, 0);
    }

    #[test]
    fn parse_rfc2822() {
        let d = GitDate::parse("Wed, 15 Jan 2025 12:00:00 +0000").unwrap();
        assert_eq!(d.timestamp, 1736942400);
        assert_eq!(d.tz_offset, 0);
    }

    #[test]
    fn parse_git_iso() {
        let d = GitDate::parse("2025-01-15 12:00:00 +0000").unwrap();
        assert_eq!(d.timestamp, 1736942400);
        assert_eq!(d.tz_offset, 0);
    }

    #[test]
    fn format_raw() {
        let d = GitDate::new(1234567890, 0);
        assert_eq!(d.format(DateFormat::Raw), "1234567890 +0000");
    }

    #[test]
    fn format_raw_negative_tz() {
        let d = GitDate::new(1234567890, -300);
        assert_eq!(d.format(DateFormat::Raw), "1234567890 -0500");
    }

    #[test]
    fn format_unix() {
        let d = GitDate::new(1234567890, 0);
        assert_eq!(d.format(DateFormat::Unix), "1234567890");
    }

    #[test]
    fn format_short() {
        let d = GitDate::new(1736942400, 0);
        assert_eq!(d.format(DateFormat::Short), "2025-01-15");
    }

    #[test]
    fn format_iso() {
        let d = GitDate::new(1736942400, 0);
        assert_eq!(d.format(DateFormat::Iso), "2025-01-15 12:00:00 +0000");
    }

    #[test]
    fn format_iso_strict() {
        let d = GitDate::new(1736942400, 0);
        assert_eq!(d.format(DateFormat::IsoStrict), "2025-01-15T12:00:00+00:00");
    }

    #[test]
    fn format_rfc2822() {
        let d = GitDate::new(1736942400, 0);
        assert_eq!(
            d.format(DateFormat::Rfc2822),
            "Wed, 15 Jan 2025 12:00:00 +0000"
        );
    }

    #[test]
    fn approxidate_yesterday() {
        let d = GitDate::parse_approxidate("yesterday").unwrap();
        let now = Utc::now().timestamp();
        // Should be roughly 24 hours ago
        assert!((now - d.timestamp - 86400).unsigned_abs() < 5);
    }

    #[test]
    fn approxidate_n_days_ago() {
        let d = GitDate::parse_approxidate("3 days ago").unwrap();
        let now = Utc::now().timestamp();
        assert!((now - d.timestamp - 3 * 86400).unsigned_abs() < 5);
    }

    #[test]
    fn approxidate_n_weeks_ago() {
        let d = GitDate::parse_approxidate("2 weeks ago").unwrap();
        let now = Utc::now().timestamp();
        assert!((now - d.timestamp - 14 * 86400).unsigned_abs() < 5);
    }

    #[test]
    fn signature_parse() {
        let input = BStr::new(b"John Doe <john@example.com> 1234567890 +0000");
        let sig = Signature::parse(input).unwrap();
        assert_eq!(sig.name, BString::from("John Doe"));
        assert_eq!(sig.email, BString::from("john@example.com"));
        assert_eq!(sig.date.timestamp, 1234567890);
        assert_eq!(sig.date.tz_offset, 0);
    }

    #[test]
    fn signature_roundtrip() {
        let sig = Signature {
            name: BString::from("Jane Doe"),
            email: BString::from("jane@example.com"),
            date: GitDate::new(1234567890, -300),
        };
        let bytes = sig.to_bytes();
        assert_eq!(
            bytes,
            BString::from("Jane Doe <jane@example.com> 1234567890 -0500")
        );

        // Parse back
        let parsed = Signature::parse(bytes.as_ref()).unwrap();
        assert_eq!(parsed.name, sig.name);
        assert_eq!(parsed.email, sig.email);
        assert_eq!(parsed.date.timestamp, sig.date.timestamp);
        assert_eq!(parsed.date.tz_offset, sig.date.tz_offset);
    }

    #[test]
    fn tz_conversion_roundtrip() {
        // +0530 -> 330 minutes -> +0530
        assert_eq!(tz_offset_to_minutes(530), 330);
        assert_eq!(minutes_to_tz_offset(330), 530);

        // -0500 -> -300 minutes -> -0500
        assert_eq!(tz_offset_to_minutes(-500), -300);
        assert_eq!(minutes_to_tz_offset(-300), -500);

        // +0000 -> 0 minutes -> +0000
        assert_eq!(tz_offset_to_minutes(0), 0);
        assert_eq!(minutes_to_tz_offset(0), 0);
    }
}

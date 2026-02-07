//! Typed value conversion (bool, int, path, color).

use bstr::{BStr, ByteSlice};
use crate::error::ConfigError;

/// Parse a boolean config value.
///
/// Rules matching C git:
/// - None (key with no = sign) → true
/// - "" (empty string) → false
/// - "true", "yes", "on" (case-insensitive) → true
/// - "false", "no", "off" (case-insensitive) → false
/// - "1" → true
/// - "0" → false
pub fn parse_bool(value: Option<&BStr>) -> Result<bool, ConfigError> {
    match value {
        None => Ok(true), // key with no value
        Some(v) => {
            let s = v.to_str_lossy();
            let s = s.trim();
            if s.is_empty() {
                return Ok(false);
            }
            match s.to_ascii_lowercase().as_str() {
                "true" | "yes" | "on" => Ok(true),
                "false" | "no" | "off" => Ok(false),
                _ => {
                    // Try parsing as integer
                    if let Ok(n) = s.parse::<i64>() {
                        match n {
                            0 => Ok(false),
                            _ => Ok(true),
                        }
                    } else {
                        Err(ConfigError::InvalidBool(s.to_string()))
                    }
                }
            }
        }
    }
}

/// Parse an integer config value with optional k/m/g suffix.
///
/// Suffix multipliers (case-insensitive):
/// - k/K: ×1024
/// - m/M: ×1048576 (1024²)
/// - g/G: ×1073741824 (1024³)
pub fn parse_int(value: &BStr) -> Result<i64, ConfigError> {
    let s = value.to_str_lossy();
    let s = s.trim();
    if s.is_empty() {
        return Err(ConfigError::InvalidInt("empty value".into()));
    }

    let (num_str, multiplier): (&str, i64) = if s.len() > 1 {
        match s.as_bytes().last() {
            Some(b'k') | Some(b'K') => (&s[..s.len() - 1], 1024),
            Some(b'm') | Some(b'M') => (&s[..s.len() - 1], 1024 * 1024),
            Some(b'g') | Some(b'G') => (&s[..s.len() - 1], 1024 * 1024 * 1024),
            _ => (s, 1),
        }
    } else {
        (s, 1)
    };

    let base: i64 = num_str
        .parse()
        .map_err(|_| ConfigError::InvalidInt(s.to_string()))?;

    base.checked_mul(multiplier)
        .ok_or_else(|| ConfigError::InvalidInt(format!("overflow: {}", s)))
}

/// Parse a path config value, expanding `~/` to the home directory.
pub fn parse_path(value: &BStr) -> Result<std::path::PathBuf, ConfigError> {
    let s = value.to_str_lossy();
    let s = s.trim();

    if s.starts_with("~/") || s == "~" {
        if let Some(home) = home_dir() {
            if s == "~" {
                Ok(home)
            } else {
                Ok(home.join(&s[2..]))
            }
        } else {
            // Can't expand ~, return as-is
            Ok(std::path::PathBuf::from(s.to_string()))
        }
    } else {
        Ok(std::path::PathBuf::from(s.to_string()))
    }
}

/// Get the user's home directory.
fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// ANSI color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiColor {
    Normal,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    /// 256-color index.
    Ansi256(u8),
    /// 24-bit RGB color.
    Rgb(u8, u8, u8),
}

impl AnsiColor {
    /// Get the ANSI SGR parameter for foreground.
    pub fn fg_code(&self) -> String {
        match self {
            AnsiColor::Normal => String::new(),
            AnsiColor::Black => "30".into(),
            AnsiColor::Red => "31".into(),
            AnsiColor::Green => "32".into(),
            AnsiColor::Yellow => "33".into(),
            AnsiColor::Blue => "34".into(),
            AnsiColor::Magenta => "35".into(),
            AnsiColor::Cyan => "36".into(),
            AnsiColor::White => "37".into(),
            AnsiColor::BrightBlack => "90".into(),
            AnsiColor::BrightRed => "91".into(),
            AnsiColor::BrightGreen => "92".into(),
            AnsiColor::BrightYellow => "93".into(),
            AnsiColor::BrightBlue => "94".into(),
            AnsiColor::BrightMagenta => "95".into(),
            AnsiColor::BrightCyan => "96".into(),
            AnsiColor::BrightWhite => "97".into(),
            AnsiColor::Ansi256(n) => format!("38;5;{}", n),
            AnsiColor::Rgb(r, g, b) => format!("38;2;{};{};{}", r, g, b),
        }
    }

    /// Get the ANSI SGR parameter for background.
    pub fn bg_code(&self) -> String {
        match self {
            AnsiColor::Normal => String::new(),
            AnsiColor::Black => "40".into(),
            AnsiColor::Red => "41".into(),
            AnsiColor::Green => "42".into(),
            AnsiColor::Yellow => "43".into(),
            AnsiColor::Blue => "44".into(),
            AnsiColor::Magenta => "45".into(),
            AnsiColor::Cyan => "46".into(),
            AnsiColor::White => "47".into(),
            AnsiColor::BrightBlack => "100".into(),
            AnsiColor::BrightRed => "101".into(),
            AnsiColor::BrightGreen => "102".into(),
            AnsiColor::BrightYellow => "103".into(),
            AnsiColor::BrightBlue => "104".into(),
            AnsiColor::BrightMagenta => "105".into(),
            AnsiColor::BrightCyan => "106".into(),
            AnsiColor::BrightWhite => "107".into(),
            AnsiColor::Ansi256(n) => format!("48;5;{}", n),
            AnsiColor::Rgb(r, g, b) => format!("48;2;{};{};{}", r, g, b),
        }
    }
}

/// Color specification from config.
#[derive(Debug, Clone, Default)]
pub struct ColorSpec {
    pub foreground: Option<AnsiColor>,
    pub background: Option<AnsiColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strike: bool,
}

impl ColorSpec {
    /// Produce the ANSI escape sequence for this color spec.
    pub fn to_ansi(&self) -> String {
        let mut codes = Vec::new();

        if let Some(ref fg) = self.foreground {
            let code = fg.fg_code();
            if !code.is_empty() {
                codes.push(code);
            }
        }
        if let Some(ref bg) = self.background {
            let code = bg.bg_code();
            if !code.is_empty() {
                codes.push(code);
            }
        }
        if self.bold {
            codes.push("1".into());
        }
        if self.dim {
            codes.push("2".into());
        }
        if self.italic {
            codes.push("3".into());
        }
        if self.underline {
            codes.push("4".into());
        }
        if self.reverse {
            codes.push("7".into());
        }
        if self.strike {
            codes.push("9".into());
        }

        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}

/// Parse a color specification string (e.g., "red bold", "#ff0000 dim").
pub fn parse_color(value: &BStr) -> Result<ColorSpec, ConfigError> {
    let s = value.to_str_lossy();
    let s = s.trim();

    if s.is_empty() || s.eq_ignore_ascii_case("normal") || s.eq_ignore_ascii_case("reset") {
        return Ok(ColorSpec::default());
    }

    let mut spec = ColorSpec::default();
    let mut fg_set = false;

    for word in s.split_whitespace() {
        let lower = word.to_ascii_lowercase();
        match lower.as_str() {
            "bold" => spec.bold = true,
            "dim" => spec.dim = true,
            "italic" => spec.italic = true,
            "ul" | "underline" => spec.underline = true,
            "reverse" => spec.reverse = true,
            "strike" => spec.strike = true,
            "nobold" => spec.bold = false,
            "nodim" => spec.dim = false,
            "noitalic" => spec.italic = false,
            "noul" | "nounderline" => spec.underline = false,
            "noreverse" => spec.reverse = false,
            "nostrike" => spec.strike = false,
            "normal" | "reset" => {}
            _ => {
                // Try to parse as a color
                if let Some(color) = parse_color_name(&lower) {
                    if !fg_set {
                        spec.foreground = Some(color);
                        fg_set = true;
                    } else {
                        spec.background = Some(color);
                    }
                } else if word.starts_with('#') && word.len() == 7 {
                    // #rrggbb
                    let r = u8::from_str_radix(&word[1..3], 16)
                        .map_err(|_| ConfigError::InvalidColor(s.to_string()))?;
                    let g = u8::from_str_radix(&word[3..5], 16)
                        .map_err(|_| ConfigError::InvalidColor(s.to_string()))?;
                    let b = u8::from_str_radix(&word[5..7], 16)
                        .map_err(|_| ConfigError::InvalidColor(s.to_string()))?;
                    let color = AnsiColor::Rgb(r, g, b);
                    if !fg_set {
                        spec.foreground = Some(color);
                        fg_set = true;
                    } else {
                        spec.background = Some(color);
                    }
                } else if let Ok(n) = word.parse::<u8>() {
                    // 0-255 ANSI color
                    let color = AnsiColor::Ansi256(n);
                    if !fg_set {
                        spec.foreground = Some(color);
                        fg_set = true;
                    } else {
                        spec.background = Some(color);
                    }
                } else {
                    return Err(ConfigError::InvalidColor(format!(
                        "unknown color attribute: {}",
                        word
                    )));
                }
            }
        }
    }

    Ok(spec)
}

/// Parse a named color.
fn parse_color_name(name: &str) -> Option<AnsiColor> {
    match name {
        "normal" => Some(AnsiColor::Normal),
        "black" => Some(AnsiColor::Black),
        "red" => Some(AnsiColor::Red),
        "green" => Some(AnsiColor::Green),
        "yellow" => Some(AnsiColor::Yellow),
        "blue" => Some(AnsiColor::Blue),
        "magenta" => Some(AnsiColor::Magenta),
        "cyan" => Some(AnsiColor::Cyan),
        "white" => Some(AnsiColor::White),
        "brightblack" | "bright black" => Some(AnsiColor::BrightBlack),
        "brightred" | "bright red" => Some(AnsiColor::BrightRed),
        "brightgreen" | "bright green" => Some(AnsiColor::BrightGreen),
        "brightyellow" | "bright yellow" => Some(AnsiColor::BrightYellow),
        "brightblue" | "bright blue" => Some(AnsiColor::BrightBlue),
        "brightmagenta" | "bright magenta" => Some(AnsiColor::BrightMagenta),
        "brightcyan" | "bright cyan" => Some(AnsiColor::BrightCyan),
        "brightwhite" | "bright white" => Some(AnsiColor::BrightWhite),
        _ => None,
    }
}

/// push.default behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PushDefault {
    /// Refuse to push without explicit refspec.
    Nothing,
    /// Push current branch to same-named branch on remote.
    Current,
    /// Push current branch to its upstream tracking branch.
    Upstream,
    /// Push current branch to upstream, but only if names match (default).
    #[default]
    Simple,
    /// Push all branches with matching names on remote.
    Matching,
}

impl PushDefault {
    pub fn from_config(value: &str) -> Result<Self, ConfigError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nothing" => Ok(PushDefault::Nothing),
            "current" => Ok(PushDefault::Current),
            "upstream" | "tracking" => Ok(PushDefault::Upstream),
            "simple" => Ok(PushDefault::Simple),
            "matching" => Ok(PushDefault::Matching),
            other => Err(ConfigError::InvalidKey(format!(
                "invalid push.default value: {}",
                other
            ))),
        }
    }
}


/// Parsed push-related configuration.
#[derive(Debug, Clone)]
pub struct PushConfig {
    pub default: PushDefault,
    pub follow_tags: bool,
    pub auto_setup_remote: bool,
}

impl Default for PushConfig {
    fn default() -> Self {
        PushConfig {
            default: PushDefault::Simple,
            follow_tags: false,
            auto_setup_remote: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_bool tests ---

    #[test]
    fn bool_none_is_true() {
        assert_eq!(parse_bool(None).unwrap(), true);
    }

    #[test]
    fn bool_empty_is_false() {
        assert_eq!(parse_bool(Some(BStr::new(""))).unwrap(), false);
    }

    #[test]
    fn bool_true_variants() {
        for v in &["true", "yes", "on", "True", "YES", "On", "1"] {
            assert_eq!(parse_bool(Some(BStr::new(v))).unwrap(), true, "failed for {}", v);
        }
    }

    #[test]
    fn bool_false_variants() {
        for v in &["false", "no", "off", "False", "NO", "Off", "0"] {
            assert_eq!(parse_bool(Some(BStr::new(v))).unwrap(), false, "failed for {}", v);
        }
    }

    #[test]
    fn bool_invalid() {
        assert!(parse_bool(Some(BStr::new("maybe"))).is_err());
    }

    // --- parse_int tests ---

    #[test]
    fn int_plain() {
        assert_eq!(parse_int(BStr::new("42")).unwrap(), 42);
    }

    #[test]
    fn int_negative() {
        assert_eq!(parse_int(BStr::new("-5")).unwrap(), -5);
    }

    #[test]
    fn int_k_suffix() {
        assert_eq!(parse_int(BStr::new("10k")).unwrap(), 10240);
        assert_eq!(parse_int(BStr::new("10K")).unwrap(), 10240);
    }

    #[test]
    fn int_m_suffix() {
        assert_eq!(parse_int(BStr::new("10m")).unwrap(), 10485760);
        assert_eq!(parse_int(BStr::new("10M")).unwrap(), 10485760);
    }

    #[test]
    fn int_g_suffix() {
        assert_eq!(parse_int(BStr::new("1g")).unwrap(), 1073741824);
        assert_eq!(parse_int(BStr::new("1G")).unwrap(), 1073741824);
    }

    #[test]
    fn int_empty_fails() {
        assert!(parse_int(BStr::new("")).is_err());
    }

    #[test]
    fn int_invalid_fails() {
        assert!(parse_int(BStr::new("abc")).is_err());
    }

    // --- parse_path tests ---

    #[test]
    fn path_tilde_expansion() {
        let result = parse_path(BStr::new("~/foo/bar")).unwrap();
        if let Some(home) = home_dir() {
            assert_eq!(result, home.join("foo/bar"));
        }
    }

    #[test]
    fn path_absolute() {
        let result = parse_path(BStr::new("/absolute/path")).unwrap();
        assert_eq!(result, std::path::PathBuf::from("/absolute/path"));
    }

    #[test]
    fn path_relative() {
        let result = parse_path(BStr::new("relative/path")).unwrap();
        assert_eq!(result, std::path::PathBuf::from("relative/path"));
    }

    // --- parse_color tests ---

    #[test]
    fn color_simple() {
        let spec = parse_color(BStr::new("red")).unwrap();
        assert_eq!(spec.foreground, Some(AnsiColor::Red));
        assert_eq!(spec.background, None);
    }

    #[test]
    fn color_with_attribute() {
        let spec = parse_color(BStr::new("red bold")).unwrap();
        assert_eq!(spec.foreground, Some(AnsiColor::Red));
        assert!(spec.bold);
    }

    #[test]
    fn color_fg_and_bg() {
        let spec = parse_color(BStr::new("red blue")).unwrap();
        assert_eq!(spec.foreground, Some(AnsiColor::Red));
        assert_eq!(spec.background, Some(AnsiColor::Blue));
    }

    #[test]
    fn color_hex() {
        let spec = parse_color(BStr::new("#ff0000")).unwrap();
        assert_eq!(spec.foreground, Some(AnsiColor::Rgb(255, 0, 0)));
    }

    #[test]
    fn color_256() {
        let spec = parse_color(BStr::new("196")).unwrap();
        assert_eq!(spec.foreground, Some(AnsiColor::Ansi256(196)));
    }

    #[test]
    fn color_empty_is_default() {
        let spec = parse_color(BStr::new("")).unwrap();
        assert_eq!(spec.foreground, None);
    }

    #[test]
    fn color_ansi_output() {
        let spec = parse_color(BStr::new("red bold")).unwrap();
        let ansi = spec.to_ansi();
        assert!(ansi.contains("31"));
        assert!(ansi.contains("1"));
    }

    // --- PushDefault tests ---

    #[test]
    fn push_default_simple() {
        assert_eq!(PushDefault::from_config("simple").unwrap(), PushDefault::Simple);
    }

    #[test]
    fn push_default_tracking_alias() {
        assert_eq!(PushDefault::from_config("tracking").unwrap(), PushDefault::Upstream);
    }

    #[test]
    fn push_default_case_insensitive() {
        assert_eq!(PushDefault::from_config("CURRENT").unwrap(), PushDefault::Current);
    }

    #[test]
    fn push_default_invalid() {
        assert!(PushDefault::from_config("invalid").is_err());
    }
}

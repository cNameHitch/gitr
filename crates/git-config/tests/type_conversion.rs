//! Type conversion tests — verify typed value interpretation matches C git.

use bstr::BStr;
use git_config::{parse_bool, parse_int, parse_path, parse_color, PushDefault};
use git_config::types::AnsiColor;

// --- Boolean conversion ---

#[test]
fn bool_no_value_is_true() {
    assert_eq!(parse_bool(None).unwrap(), true);
}

#[test]
fn bool_empty_is_false() {
    assert_eq!(parse_bool(Some(BStr::new(""))).unwrap(), false);
}

#[test]
fn bool_true_variants() {
    for s in &["true", "True", "TRUE", "yes", "Yes", "YES", "on", "On", "ON", "1"] {
        assert!(
            parse_bool(Some(BStr::new(s))).unwrap(),
            "expected true for {:?}",
            s
        );
    }
}

#[test]
fn bool_false_variants() {
    for s in &["false", "False", "FALSE", "no", "No", "NO", "off", "Off", "OFF", "0"] {
        assert!(
            !parse_bool(Some(BStr::new(s))).unwrap(),
            "expected false for {:?}",
            s
        );
    }
}

#[test]
fn bool_invalid_value() {
    assert!(parse_bool(Some(BStr::new("maybe"))).is_err());
    assert!(parse_bool(Some(BStr::new("2"))).is_ok()); // non-zero integers are truthy
}

// --- Integer conversion ---

#[test]
fn int_plain_values() {
    assert_eq!(parse_int(BStr::new("0")).unwrap(), 0);
    assert_eq!(parse_int(BStr::new("1")).unwrap(), 1);
    assert_eq!(parse_int(BStr::new("42")).unwrap(), 42);
    assert_eq!(parse_int(BStr::new("-1")).unwrap(), -1);
    assert_eq!(parse_int(BStr::new("1000000")).unwrap(), 1000000);
}

#[test]
fn int_k_suffix() {
    assert_eq!(parse_int(BStr::new("1k")).unwrap(), 1024);
    assert_eq!(parse_int(BStr::new("10k")).unwrap(), 10240);
    assert_eq!(parse_int(BStr::new("1K")).unwrap(), 1024);
    assert_eq!(parse_int(BStr::new("512K")).unwrap(), 524288);
}

#[test]
fn int_m_suffix() {
    assert_eq!(parse_int(BStr::new("1m")).unwrap(), 1048576);
    assert_eq!(parse_int(BStr::new("10m")).unwrap(), 10485760);
    assert_eq!(parse_int(BStr::new("1M")).unwrap(), 1048576);
}

#[test]
fn int_g_suffix() {
    assert_eq!(parse_int(BStr::new("1g")).unwrap(), 1073741824);
    assert_eq!(parse_int(BStr::new("1G")).unwrap(), 1073741824);
    assert_eq!(parse_int(BStr::new("2g")).unwrap(), 2147483648);
}

#[test]
fn int_with_whitespace() {
    assert_eq!(parse_int(BStr::new("  42  ")).unwrap(), 42);
    assert_eq!(parse_int(BStr::new(" 10k ")).unwrap(), 10240);
}

#[test]
fn int_invalid() {
    assert!(parse_int(BStr::new("")).is_err());
    assert!(parse_int(BStr::new("abc")).is_err());
    assert!(parse_int(BStr::new("12x")).is_err());
}

// --- Path conversion ---

#[test]
fn path_tilde_expansion() {
    let home = std::env::var("HOME").unwrap();
    let result = parse_path(BStr::new("~/foo/bar")).unwrap();
    assert_eq!(
        result,
        std::path::PathBuf::from(format!("{}/foo/bar", home))
    );
}

#[test]
fn path_tilde_only() {
    let home = std::env::var("HOME").unwrap();
    let result = parse_path(BStr::new("~")).unwrap();
    assert_eq!(result, std::path::PathBuf::from(home));
}

#[test]
fn path_absolute_unchanged() {
    let result = parse_path(BStr::new("/absolute/path")).unwrap();
    assert_eq!(result, std::path::PathBuf::from("/absolute/path"));
}

#[test]
fn path_relative_unchanged() {
    let result = parse_path(BStr::new("relative/path")).unwrap();
    assert_eq!(result, std::path::PathBuf::from("relative/path"));
}

// --- Color conversion ---

#[test]
fn color_empty_is_default() {
    let spec = parse_color(BStr::new("")).unwrap();
    assert_eq!(spec.foreground, None);
    assert_eq!(spec.background, None);
    assert!(!spec.bold);
}

#[test]
fn color_named_foreground() {
    let spec = parse_color(BStr::new("red")).unwrap();
    assert_eq!(spec.foreground, Some(AnsiColor::Red));
}

#[test]
fn color_fg_and_bg() {
    let spec = parse_color(BStr::new("red blue")).unwrap();
    assert_eq!(spec.foreground, Some(AnsiColor::Red));
    assert_eq!(spec.background, Some(AnsiColor::Blue));
}

#[test]
fn color_with_attributes() {
    let spec = parse_color(BStr::new("red bold")).unwrap();
    assert_eq!(spec.foreground, Some(AnsiColor::Red));
    assert!(spec.bold);
}

#[test]
fn color_multiple_attributes() {
    let spec = parse_color(BStr::new("green bold ul italic")).unwrap();
    assert_eq!(spec.foreground, Some(AnsiColor::Green));
    assert!(spec.bold);
    assert!(spec.underline);
    assert!(spec.italic);
}

#[test]
fn color_hex_rgb() {
    let spec = parse_color(BStr::new("#ff0000")).unwrap();
    assert_eq!(spec.foreground, Some(AnsiColor::Rgb(255, 0, 0)));
}

#[test]
fn color_256_palette() {
    let spec = parse_color(BStr::new("196")).unwrap();
    assert_eq!(spec.foreground, Some(AnsiColor::Ansi256(196)));
}

#[test]
fn color_ansi_output() {
    let spec = parse_color(BStr::new("red bold")).unwrap();
    let ansi = spec.to_ansi();
    assert!(ansi.starts_with("\x1b["));
    assert!(ansi.contains("31")); // red
    assert!(ansi.contains("1")); // bold
}

#[test]
fn color_no_attributes() {
    let spec = parse_color(BStr::new("bold ul")).unwrap();
    assert_eq!(spec.foreground, None);
    assert!(spec.bold);
    assert!(spec.underline);
}

// --- PushDefault ---

#[test]
fn push_default_all_values() {
    assert_eq!(PushDefault::from_config("nothing").unwrap(), PushDefault::Nothing);
    assert_eq!(PushDefault::from_config("current").unwrap(), PushDefault::Current);
    assert_eq!(PushDefault::from_config("upstream").unwrap(), PushDefault::Upstream);
    assert_eq!(PushDefault::from_config("tracking").unwrap(), PushDefault::Upstream);
    assert_eq!(PushDefault::from_config("simple").unwrap(), PushDefault::Simple);
    assert_eq!(PushDefault::from_config("matching").unwrap(), PushDefault::Matching);
}

#[test]
fn push_default_case_insensitive() {
    assert_eq!(PushDefault::from_config("SIMPLE").unwrap(), PushDefault::Simple);
    assert_eq!(PushDefault::from_config("Current").unwrap(), PushDefault::Current);
}

#[test]
fn push_default_invalid() {
    assert!(PushDefault::from_config("invalid").is_err());
}

#[test]
fn push_default_default_is_simple() {
    assert_eq!(PushDefault::default(), PushDefault::Simple);
}

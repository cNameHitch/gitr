use std::collections::HashMap;
use std::io::IsTerminal;

/// Color configuration mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Auto-detect based on terminal and NO_COLOR env var.
    Auto,
    /// Always emit ANSI color codes.
    Always,
    /// Never emit ANSI color codes.
    Never,
}

/// Standard git colors matching C git's `color.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Normal,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Bold,
    Dim,
    Italic,
    Underline,
    Reset,
}

impl Color {
    /// Get the ANSI escape sequence for this color.
    pub fn ansi_code(self) -> &'static str {
        match self {
            Color::Normal => "",
            Color::Red => "\x1b[31m",
            Color::Green => "\x1b[32m",
            Color::Yellow => "\x1b[33m",
            Color::Blue => "\x1b[34m",
            Color::Magenta => "\x1b[35m",
            Color::Cyan => "\x1b[36m",
            Color::White => "\x1b[37m",
            Color::BrightRed => "\x1b[91m",
            Color::BrightGreen => "\x1b[92m",
            Color::BrightYellow => "\x1b[93m",
            Color::BrightBlue => "\x1b[94m",
            Color::BrightMagenta => "\x1b[95m",
            Color::BrightCyan => "\x1b[96m",
            Color::BrightWhite => "\x1b[97m",
            Color::Bold => "\x1b[1m",
            Color::Dim => "\x1b[2m",
            Color::Italic => "\x1b[3m",
            Color::Underline => "\x1b[4m",
            Color::Reset => "\x1b[0m",
        }
    }
}

/// Check if color should be used for the given mode and stream.
///
/// Respects:
/// - The `NO_COLOR` environment variable (<https://no-color.org/>)
/// - The `GIT_NO_COLOR` environment variable
/// - Whether the stream is a terminal (for Auto mode)
pub fn use_color(mode: ColorMode, is_terminal: bool) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            if std::env::var_os("NO_COLOR").is_some() {
                return false;
            }
            if std::env::var_os("GIT_NO_COLOR").is_some() {
                return false;
            }
            is_terminal
        }
    }
}

/// Check if stdout should use color.
pub fn use_color_stdout(mode: ColorMode) -> bool {
    use_color(mode, std::io::stdout().is_terminal())
}

/// Check if stderr should use color.
pub fn use_color_stderr(mode: ColorMode) -> bool {
    use_color(mode, std::io::stderr().is_terminal())
}

/// Format text with ANSI color if enabled, matching C git's color output.
pub fn colorize(text: &str, color: Color, enabled: bool) -> String {
    if !enabled || color == Color::Normal {
        return text.to_string();
    }
    format!("{}{}{}", color.ansi_code(), text, Color::Reset.ansi_code())
}

/// All git color slots with their default ANSI escape code mappings.
///
/// Each slot represents a semantic coloring point in git output (e.g., diff headers,
/// status lines, branch names). The default ANSI codes match C git's built-in defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSlot {
    // Diff colors
    DiffOldNormal,
    DiffNewNormal,
    DiffOldMoved,
    DiffNewMoved,
    DiffContext,
    DiffMetaInfo,
    DiffFragInfo,
    DiffFuncInfo,
    DiffOldPlain,
    DiffNewPlain,
    DiffWhitespace,
    // Status colors
    StatusHeader,
    StatusAdded,
    StatusChanged,
    StatusUntracked,
    StatusBranch,
    StatusNoBranch,
    // Branch colors
    BranchCurrent,
    BranchLocal,
    BranchRemote,
    BranchUpstream,
    BranchPlain,
    // Log/show decoration colors
    DecorateHead,
    DecorateBranch,
    DecorateRemote,
    DecorateTag,
    DecorateStash,
    DecorateGrafted,
    // Grep colors
    GrepFilename,
    GrepLineNumber,
    GrepSeparator,
    GrepMatch,
    // General
    Reset,
}

impl ColorSlot {
    /// Return the default ANSI escape code for this slot, matching C git's defaults.
    pub fn default_ansi(&self) -> &'static str {
        match self {
            // Diff colors
            ColorSlot::DiffOldNormal => "\x1b[31m",
            ColorSlot::DiffNewNormal => "\x1b[32m",
            ColorSlot::DiffOldMoved => "\x1b[1;35m",
            ColorSlot::DiffNewMoved => "\x1b[1;36m",
            ColorSlot::DiffContext => "",
            ColorSlot::DiffMetaInfo => "\x1b[1m",
            ColorSlot::DiffFragInfo => "\x1b[36m",
            ColorSlot::DiffFuncInfo => "",
            ColorSlot::DiffOldPlain => "\x1b[31m",
            ColorSlot::DiffNewPlain => "\x1b[32m",
            ColorSlot::DiffWhitespace => "\x1b[41m",
            // Status colors
            ColorSlot::StatusHeader => "",
            ColorSlot::StatusAdded => "\x1b[32m",
            ColorSlot::StatusChanged => "\x1b[31m",
            ColorSlot::StatusUntracked => "\x1b[31m",
            ColorSlot::StatusBranch => "",
            ColorSlot::StatusNoBranch => "\x1b[31m",
            // Branch colors
            ColorSlot::BranchCurrent => "\x1b[32m",
            ColorSlot::BranchLocal => "",
            ColorSlot::BranchRemote => "\x1b[31m",
            ColorSlot::BranchUpstream => "\x1b[34m",
            ColorSlot::BranchPlain => "",
            // Log/show decoration colors
            ColorSlot::DecorateHead => "\x1b[1;36m",
            ColorSlot::DecorateBranch => "\x1b[1;32m",
            ColorSlot::DecorateRemote => "\x1b[1;31m",
            ColorSlot::DecorateTag => "\x1b[1;33m",
            ColorSlot::DecorateStash => "\x1b[1;35m",
            ColorSlot::DecorateGrafted => "\x1b[1;34m",
            // Grep colors
            ColorSlot::GrepFilename => "\x1b[35m",
            ColorSlot::GrepLineNumber => "\x1b[32m",
            ColorSlot::GrepSeparator => "\x1b[36m",
            ColorSlot::GrepMatch => "\x1b[1;31m",
            // General
            ColorSlot::Reset => "\x1b[m",
        }
    }
}

/// Aggregated color configuration read from git config.
///
/// Holds the global `color.ui` mode, per-command overrides (`color.<cmd>`),
/// and per-slot custom ANSI codes (`color.<cmd>.<slot>`).
pub struct ColorConfig {
    /// The global `color.ui` setting (default: Auto).
    pub ui: ColorMode,
    /// Per-command color mode overrides (e.g., `color.diff = always`).
    pub commands: HashMap<String, ColorMode>,
    /// Per-slot custom ANSI escape codes.
    pub slots: HashMap<ColorSlot, String>,
}

impl ColorConfig {
    /// Create a new `ColorConfig` with default values.
    pub fn new() -> Self {
        Self {
            ui: ColorMode::Auto,
            commands: HashMap::new(),
            slots: HashMap::new(),
        }
    }

    /// Build a `ColorConfig` by reading values through a config lookup function.
    ///
    /// The `get_string` closure should look up a git config key (e.g., `"color.ui"`)
    /// and return `Ok(Some(value))` if set, `Ok(None)` if missing, or `Err` on failure.
    /// This is designed to work with `git_config::ConfigSet::get_string`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config_set = git_config::ConfigSet::load(Some(git_dir))?;
    /// let cc = ColorConfig::from_config(|key| config_set.get_string(key).ok().flatten());
    /// ```
    pub fn from_config<F>(get_string: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut cc = Self::new();

        // Read color.ui
        if let Some(val) = get_string("color.ui") {
            cc.ui = parse_color_mode(&val);
        }

        // Read per-command overrides
        for cmd in &[
            "diff", "status", "branch", "log", "grep", "show", "blame", "shortlog",
        ] {
            let key = format!("color.{}", cmd);
            if let Some(val) = get_string(&key) {
                cc.commands.insert(cmd.to_string(), parse_color_mode(&val));
            }
        }

        cc
    }

    /// Determine the effective color mode for a given command.
    ///
    /// Priority order: CLI flag > per-command config > `color.ui` > default Auto.
    pub fn effective_mode(&self, command: &str, cli_flag: Option<ColorMode>) -> ColorMode {
        if let Some(mode) = cli_flag {
            return mode;
        }
        if let Some(&mode) = self.commands.get(command) {
            return mode;
        }
        self.ui
    }

    /// Get the ANSI escape code for a color slot.
    ///
    /// Returns the custom override if one has been configured, otherwise
    /// falls back to the slot's built-in default.
    pub fn get_color(&self, slot: ColorSlot) -> &str {
        if let Some(custom) = self.slots.get(&slot) {
            return custom;
        }
        slot.default_ansi()
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a git color mode string into a `ColorMode`.
///
/// Recognized values (case-insensitive):
/// - `"always"`, `"true"`, `"yes"` -> `ColorMode::Always`
/// - `"never"`, `"false"`, `"no"` -> `ColorMode::Never`
/// - anything else -> `ColorMode::Auto`
pub fn parse_color_mode(s: &str) -> ColorMode {
    match s.to_lowercase().as_str() {
        "always" | "true" | "yes" => ColorMode::Always,
        "never" | "false" | "no" => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

/// Parse a git color value string into an ANSI escape sequence.
///
/// Supports the same color names and attributes as C git:
/// - Named colors: `normal`, `black`, `red`, `green`, `yellow`, `blue`,
///   `magenta`, `cyan`, `white`
/// - Attributes: `bold`, `dim`, `ul` (underline), `blink`, `reverse`, `strike`
/// - 24-bit color: `#RRGGBB`
/// - Combinations: `"bold red"`, `"ul green"`, etc.
///
/// Returns an empty string for `"normal"` or an empty input.
pub fn parse_color_value(s: &str) -> String {
    let mut codes: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match word {
            "normal" => {}
            "black" => codes.push("30".to_string()),
            "red" => codes.push("31".to_string()),
            "green" => codes.push("32".to_string()),
            "yellow" => codes.push("33".to_string()),
            "blue" => codes.push("34".to_string()),
            "magenta" => codes.push("35".to_string()),
            "cyan" => codes.push("36".to_string()),
            "white" => codes.push("37".to_string()),
            "bold" => codes.push("1".to_string()),
            "dim" => codes.push("2".to_string()),
            "ul" => codes.push("4".to_string()),
            "blink" => codes.push("5".to_string()),
            "reverse" => codes.push("7".to_string()),
            "strike" => codes.push("9".to_string()),
            _ if word.starts_with('#') && word.len() == 7 => {
                // #RRGGBB -> 38;2;R;G;B
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&word[1..3], 16),
                    u8::from_str_radix(&word[3..5], 16),
                    u8::from_str_radix(&word[5..7], 16),
                ) {
                    let rgb = format!("38;2;{};{};{}", r, g, b);
                    return format!("\x1b[{}m", rgb);
                }
            }
            _ => {}
        }
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorize_enabled() {
        let result = colorize("hello", Color::Red, true);
        assert_eq!(result, "\x1b[31mhello\x1b[0m");
    }

    #[test]
    fn colorize_disabled() {
        let result = colorize("hello", Color::Red, false);
        assert_eq!(result, "hello");
    }

    #[test]
    fn colorize_normal() {
        let result = colorize("hello", Color::Normal, true);
        assert_eq!(result, "hello");
    }

    #[test]
    fn use_color_always() {
        assert!(use_color(ColorMode::Always, false));
        assert!(use_color(ColorMode::Always, true));
    }

    #[test]
    fn use_color_never() {
        assert!(!use_color(ColorMode::Never, false));
        assert!(!use_color(ColorMode::Never, true));
    }

    #[test]
    fn use_color_auto_terminal() {
        // When it's a terminal and no NO_COLOR is set
        assert!(use_color(ColorMode::Auto, true));
    }

    #[test]
    fn use_color_auto_not_terminal() {
        assert!(!use_color(ColorMode::Auto, false));
    }

    #[test]
    fn bold_text() {
        let result = colorize("important", Color::Bold, true);
        assert_eq!(result, "\x1b[1mimportant\x1b[0m");
    }

    // --- ColorSlot tests ---

    #[test]
    fn color_slot_diff_defaults() {
        assert_eq!(ColorSlot::DiffOldNormal.default_ansi(), "\x1b[31m");
        assert_eq!(ColorSlot::DiffNewNormal.default_ansi(), "\x1b[32m");
        assert_eq!(ColorSlot::DiffOldMoved.default_ansi(), "\x1b[1;35m");
        assert_eq!(ColorSlot::DiffNewMoved.default_ansi(), "\x1b[1;36m");
        assert_eq!(ColorSlot::DiffContext.default_ansi(), "");
        assert_eq!(ColorSlot::DiffMetaInfo.default_ansi(), "\x1b[1m");
        assert_eq!(ColorSlot::DiffFragInfo.default_ansi(), "\x1b[36m");
        assert_eq!(ColorSlot::DiffFuncInfo.default_ansi(), "");
        assert_eq!(ColorSlot::DiffOldPlain.default_ansi(), "\x1b[31m");
        assert_eq!(ColorSlot::DiffNewPlain.default_ansi(), "\x1b[32m");
        assert_eq!(ColorSlot::DiffWhitespace.default_ansi(), "\x1b[41m");
    }

    #[test]
    fn color_slot_status_defaults() {
        assert_eq!(ColorSlot::StatusHeader.default_ansi(), "");
        assert_eq!(ColorSlot::StatusAdded.default_ansi(), "\x1b[32m");
        assert_eq!(ColorSlot::StatusChanged.default_ansi(), "\x1b[31m");
        assert_eq!(ColorSlot::StatusUntracked.default_ansi(), "\x1b[31m");
        assert_eq!(ColorSlot::StatusBranch.default_ansi(), "");
        assert_eq!(ColorSlot::StatusNoBranch.default_ansi(), "\x1b[31m");
    }

    #[test]
    fn color_slot_branch_defaults() {
        assert_eq!(ColorSlot::BranchCurrent.default_ansi(), "\x1b[32m");
        assert_eq!(ColorSlot::BranchLocal.default_ansi(), "");
        assert_eq!(ColorSlot::BranchRemote.default_ansi(), "\x1b[31m");
        assert_eq!(ColorSlot::BranchUpstream.default_ansi(), "\x1b[34m");
        assert_eq!(ColorSlot::BranchPlain.default_ansi(), "");
    }

    #[test]
    fn color_slot_decorate_defaults() {
        assert_eq!(ColorSlot::DecorateHead.default_ansi(), "\x1b[1;36m");
        assert_eq!(ColorSlot::DecorateBranch.default_ansi(), "\x1b[1;32m");
        assert_eq!(ColorSlot::DecorateRemote.default_ansi(), "\x1b[1;31m");
        assert_eq!(ColorSlot::DecorateTag.default_ansi(), "\x1b[1;33m");
        assert_eq!(ColorSlot::DecorateStash.default_ansi(), "\x1b[1;35m");
        assert_eq!(ColorSlot::DecorateGrafted.default_ansi(), "\x1b[1;34m");
    }

    #[test]
    fn color_slot_grep_defaults() {
        assert_eq!(ColorSlot::GrepFilename.default_ansi(), "\x1b[35m");
        assert_eq!(ColorSlot::GrepLineNumber.default_ansi(), "\x1b[32m");
        assert_eq!(ColorSlot::GrepSeparator.default_ansi(), "\x1b[36m");
        assert_eq!(ColorSlot::GrepMatch.default_ansi(), "\x1b[1;31m");
    }

    #[test]
    fn color_slot_reset() {
        assert_eq!(ColorSlot::Reset.default_ansi(), "\x1b[m");
    }

    #[test]
    fn color_slot_equality() {
        assert_eq!(ColorSlot::DiffOldNormal, ColorSlot::DiffOldNormal);
        assert_ne!(ColorSlot::DiffOldNormal, ColorSlot::DiffNewNormal);
    }

    #[test]
    fn color_slot_hash_works() {
        let mut map = HashMap::new();
        map.insert(ColorSlot::DiffOldNormal, "test");
        assert_eq!(map.get(&ColorSlot::DiffOldNormal), Some(&"test"));
        assert_eq!(map.get(&ColorSlot::DiffNewNormal), None);
    }

    // --- ColorConfig tests ---

    #[test]
    fn color_config_default() {
        let cc = ColorConfig::new();
        assert_eq!(cc.ui, ColorMode::Auto);
        assert!(cc.commands.is_empty());
        assert!(cc.slots.is_empty());
    }

    #[test]
    fn color_config_default_trait() {
        let cc = ColorConfig::default();
        assert_eq!(cc.ui, ColorMode::Auto);
    }

    #[test]
    fn color_config_from_config_reads_ui() {
        let cc = ColorConfig::from_config(|key| {
            if key == "color.ui" {
                Some("always".to_string())
            } else {
                None
            }
        });
        assert_eq!(cc.ui, ColorMode::Always);
    }

    #[test]
    fn color_config_from_config_reads_commands() {
        let cc = ColorConfig::from_config(|key| match key {
            "color.ui" => Some("auto".to_string()),
            "color.diff" => Some("always".to_string()),
            "color.status" => Some("never".to_string()),
            _ => None,
        });
        assert_eq!(cc.ui, ColorMode::Auto);
        assert_eq!(cc.commands.get("diff"), Some(&ColorMode::Always));
        assert_eq!(cc.commands.get("status"), Some(&ColorMode::Never));
        assert_eq!(cc.commands.get("branch"), None);
    }

    #[test]
    fn color_config_from_config_no_values() {
        let cc = ColorConfig::from_config(|_| None);
        assert_eq!(cc.ui, ColorMode::Auto);
        assert!(cc.commands.is_empty());
    }

    #[test]
    fn color_config_effective_mode_cli_flag_wins() {
        let mut cc = ColorConfig::new();
        cc.ui = ColorMode::Never;
        cc.commands
            .insert("diff".to_string(), ColorMode::Never);
        assert_eq!(
            cc.effective_mode("diff", Some(ColorMode::Always)),
            ColorMode::Always
        );
    }

    #[test]
    fn color_config_effective_mode_command_over_ui() {
        let mut cc = ColorConfig::new();
        cc.ui = ColorMode::Never;
        cc.commands
            .insert("diff".to_string(), ColorMode::Always);
        assert_eq!(cc.effective_mode("diff", None), ColorMode::Always);
    }

    #[test]
    fn color_config_effective_mode_falls_back_to_ui() {
        let mut cc = ColorConfig::new();
        cc.ui = ColorMode::Never;
        assert_eq!(cc.effective_mode("diff", None), ColorMode::Never);
    }

    #[test]
    fn color_config_effective_mode_defaults_to_auto() {
        let cc = ColorConfig::new();
        assert_eq!(cc.effective_mode("diff", None), ColorMode::Auto);
    }

    #[test]
    fn color_config_get_color_default() {
        let cc = ColorConfig::new();
        assert_eq!(cc.get_color(ColorSlot::DiffOldNormal), "\x1b[31m");
        assert_eq!(cc.get_color(ColorSlot::DiffNewNormal), "\x1b[32m");
        assert_eq!(cc.get_color(ColorSlot::Reset), "\x1b[m");
    }

    #[test]
    fn color_config_get_color_custom_override() {
        let mut cc = ColorConfig::new();
        cc.slots
            .insert(ColorSlot::DiffOldNormal, "\x1b[33m".to_string());
        assert_eq!(cc.get_color(ColorSlot::DiffOldNormal), "\x1b[33m");
        // Other slots still return defaults
        assert_eq!(cc.get_color(ColorSlot::DiffNewNormal), "\x1b[32m");
    }

    // --- parse_color_mode tests ---

    #[test]
    fn parse_color_mode_always() {
        assert_eq!(parse_color_mode("always"), ColorMode::Always);
        assert_eq!(parse_color_mode("Always"), ColorMode::Always);
        assert_eq!(parse_color_mode("ALWAYS"), ColorMode::Always);
        assert_eq!(parse_color_mode("true"), ColorMode::Always);
        assert_eq!(parse_color_mode("yes"), ColorMode::Always);
    }

    #[test]
    fn parse_color_mode_never() {
        assert_eq!(parse_color_mode("never"), ColorMode::Never);
        assert_eq!(parse_color_mode("Never"), ColorMode::Never);
        assert_eq!(parse_color_mode("NEVER"), ColorMode::Never);
        assert_eq!(parse_color_mode("false"), ColorMode::Never);
        assert_eq!(parse_color_mode("no"), ColorMode::Never);
    }

    #[test]
    fn parse_color_mode_auto() {
        assert_eq!(parse_color_mode("auto"), ColorMode::Auto);
        assert_eq!(parse_color_mode("Auto"), ColorMode::Auto);
        assert_eq!(parse_color_mode("anything-else"), ColorMode::Auto);
        assert_eq!(parse_color_mode(""), ColorMode::Auto);
    }

    // --- parse_color_value tests ---

    #[test]
    fn parse_color_value_named_colors() {
        assert_eq!(parse_color_value("red"), "\x1b[31m");
        assert_eq!(parse_color_value("green"), "\x1b[32m");
        assert_eq!(parse_color_value("yellow"), "\x1b[33m");
        assert_eq!(parse_color_value("blue"), "\x1b[34m");
        assert_eq!(parse_color_value("magenta"), "\x1b[35m");
        assert_eq!(parse_color_value("cyan"), "\x1b[36m");
        assert_eq!(parse_color_value("white"), "\x1b[37m");
        assert_eq!(parse_color_value("black"), "\x1b[30m");
    }

    #[test]
    fn parse_color_value_attributes() {
        assert_eq!(parse_color_value("bold"), "\x1b[1m");
        assert_eq!(parse_color_value("dim"), "\x1b[2m");
        assert_eq!(parse_color_value("ul"), "\x1b[4m");
        assert_eq!(parse_color_value("blink"), "\x1b[5m");
        assert_eq!(parse_color_value("reverse"), "\x1b[7m");
        assert_eq!(parse_color_value("strike"), "\x1b[9m");
    }

    #[test]
    fn parse_color_value_combinations() {
        assert_eq!(parse_color_value("bold red"), "\x1b[1;31m");
        assert_eq!(parse_color_value("ul green"), "\x1b[4;32m");
        assert_eq!(parse_color_value("bold dim cyan"), "\x1b[1;2;36m");
    }

    #[test]
    fn parse_color_value_normal() {
        assert_eq!(parse_color_value("normal"), "");
    }

    #[test]
    fn parse_color_value_empty() {
        assert_eq!(parse_color_value(""), "");
    }

    #[test]
    fn parse_color_value_rgb() {
        assert_eq!(parse_color_value("#ff0000"), "\x1b[38;2;255;0;0m");
        assert_eq!(parse_color_value("#00ff00"), "\x1b[38;2;0;255;0m");
        assert_eq!(parse_color_value("#0000ff"), "\x1b[38;2;0;0;255m");
        assert_eq!(parse_color_value("#1a2b3c"), "\x1b[38;2;26;43;60m");
    }

    #[test]
    fn parse_color_value_unknown_ignored() {
        // Unknown words are silently ignored
        assert_eq!(parse_color_value("notacolor"), "");
        assert_eq!(parse_color_value("bold notacolor red"), "\x1b[1;31m");
    }
}

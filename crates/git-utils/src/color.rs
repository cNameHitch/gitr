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
}

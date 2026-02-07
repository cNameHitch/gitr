//! CLI framework utilities for gitr, built on clap.
//!
//! This module provides the foundational CLI types and conventions that match
//! C git's argument handling:
//!
//! - `--` separates options from pathspecs
//! - `--no-X` negates boolean options
//! - Combined short flags like `-am` are split into `-a -m`
//! - Color mode is accepted as `--color=<when>` (auto/always/never)
//!
//! Individual commands are defined in later specs (015-018).
//! This module provides shared types and conventions.

use crate::color::ColorMode;

/// Global options shared by all gitr commands.
///
/// These match C git's global options that appear before the subcommand.
#[derive(Debug, Clone, clap::Parser)]
pub struct GlobalOptions {
    /// Use the given path as the working directory.
    #[arg(short = 'C', global = true)]
    pub directory: Option<String>,

    /// Set a configuration variable: name=value.
    #[arg(short = 'c', global = true)]
    pub config: Vec<String>,

    /// Colorize output.
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorWhen,

    /// Suppress all output.
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Be more verbose.
    #[arg(long, short, global = true)]
    pub verbose: bool,
}

/// Color mode argument matching C git's `--color=<when>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorWhen {
    /// Auto-detect based on terminal.
    Auto,
    /// Always use color.
    Always,
    /// Never use color.
    Never,
}

impl From<ColorWhen> for ColorMode {
    fn from(when: ColorWhen) -> Self {
        match when {
            ColorWhen::Auto => ColorMode::Auto,
            ColorWhen::Always => ColorMode::Always,
            ColorWhen::Never => ColorMode::Never,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, clap::Parser)]
    #[command(name = "gitr")]
    struct TestCli {
        #[command(flatten)]
        global: GlobalOptions,

        /// Remaining arguments (pathspecs after --)
        #[arg(last = true)]
        pathspecs: Vec<String>,
    }

    #[test]
    fn parse_color_always() {
        let cli = TestCli::parse_from(["gitr", "--color", "always"]);
        assert_eq!(cli.global.color, ColorWhen::Always);
    }

    #[test]
    fn parse_color_never() {
        let cli = TestCli::parse_from(["gitr", "--color", "never"]);
        assert_eq!(cli.global.color, ColorWhen::Never);
    }

    #[test]
    fn parse_color_default() {
        let cli = TestCli::parse_from(["gitr"]);
        assert_eq!(cli.global.color, ColorWhen::Auto);
    }

    #[test]
    fn parse_directory() {
        let cli = TestCli::parse_from(["gitr", "-C", "/tmp"]);
        assert_eq!(cli.global.directory, Some("/tmp".to_string()));
    }

    #[test]
    fn parse_config() {
        let cli = TestCli::parse_from(["gitr", "-c", "user.name=Test"]);
        assert_eq!(cli.global.config, vec!["user.name=Test"]);
    }

    #[test]
    fn parse_quiet_verbose() {
        let cli = TestCli::parse_from(["gitr", "--quiet"]);
        assert!(cli.global.quiet);

        let cli = TestCli::parse_from(["gitr", "--verbose"]);
        assert!(cli.global.verbose);
    }

    #[test]
    fn parse_double_dash_pathspecs() {
        let cli = TestCli::parse_from(["gitr", "--", "file1.txt", "file2.txt"]);
        assert_eq!(cli.pathspecs, vec!["file1.txt", "file2.txt"]);
    }
}

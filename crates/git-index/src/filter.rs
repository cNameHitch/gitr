//! Clean/smudge filter execution.
//!
//! Implements `filter.<name>.clean` and `filter.<name>.smudge` from git config.
//! Filters are external programs that transform file content during add (clean)
//! and checkout (smudge) operations.
//!
//! # Usage
//!
//! Given a `.gitattributes` entry like `*.c filter=indent`, and config:
//! ```text
//! [filter "indent"]
//!     clean = indent
//!     smudge = cat
//! ```
//!
//! - On `git add`, the file content is piped through `indent` (clean filter)
//! - On `git checkout`, the stored content is piped through `cat` (smudge filter)

use std::io::Write;
use std::process::{Command, Stdio};

use bstr::BStr;

use crate::attributes::AttributeStack;
use crate::IndexError;

/// Direction of filter application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDirection {
    /// Clean filter: working tree -> object database (used by `git add`).
    Clean,
    /// Smudge filter: object database -> working tree (used by `git checkout`).
    Smudge,
}

/// Resolve the filter command for a given path and direction from config.
///
/// Returns `None` if no filter is configured for this path, or if the
/// relevant direction (clean/smudge) is not set.
pub fn resolve_filter_command(
    attrs: &AttributeStack,
    path: &BStr,
    direction: FilterDirection,
    config: &git_config::ConfigSet,
) -> Option<String> {
    let filter_name = attrs.filter_for(path)?;

    let config_key = match direction {
        FilterDirection::Clean => format!("filter.{}.clean", filter_name),
        FilterDirection::Smudge => format!("filter.{}.smudge", filter_name),
    };

    config.get_string(&config_key).ok().flatten()
}

/// Execute a filter command, piping `input` through it and returning the output.
///
/// The filter command is executed via the system shell (`sh -c` on Unix),
/// with the file content fed to stdin and the filtered content read from stdout.
///
/// # Arguments
///
/// * `command` - The filter command string (e.g., `"indent"`, `"gzip -d"`)
/// * `input` - The file content to filter
///
/// # Errors
///
/// Returns an error if the command cannot be spawned, fails to write stdin,
/// or exits with a non-zero status.
pub fn run_filter(command: &str, input: &[u8]) -> Result<Vec<u8>, IndexError> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| IndexError::Io(std::io::Error::other(
            format!("failed to spawn filter '{}': {}", command, e),
        )))?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).map_err(|e| IndexError::Io(std::io::Error::other(
            format!("failed to write to filter '{}' stdin: {}", command, e),
        )))?;
    }

    let output = child.wait_with_output().map_err(|e| IndexError::Io(std::io::Error::other(
        format!("filter '{}' failed: {}", command, e),
    )))?;

    if !output.status.success() {
        return Err(IndexError::Io(std::io::Error::other(
            format!(
                "filter '{}' exited with status {}",
                command,
                output.status.code().unwrap_or(-1)
            ),
        )));
    }

    Ok(output.stdout)
}

/// Apply the appropriate filter (clean or smudge) to file content.
///
/// Looks up the `filter` attribute for the path, resolves the command from
/// config, and if both are present, runs the filter. Returns the original
/// content if no filter is configured.
///
/// # Arguments
///
/// * `attrs` - The attribute stack for attribute lookup
/// * `path` - The file path (for attribute matching)
/// * `content` - The file content to filter
/// * `direction` - Whether to apply clean or smudge filter
/// * `config` - The git config set for resolving filter commands
pub fn apply_filter(
    attrs: &AttributeStack,
    path: &BStr,
    content: &[u8],
    direction: FilterDirection,
    config: &git_config::ConfigSet,
) -> Result<Vec<u8>, IndexError> {
    match resolve_filter_command(attrs, path, direction, config) {
        Some(command) => run_filter(&command, content),
        None => Ok(content.to_vec()),
    }
}

/// Substitute `%f` in a filter command with the file path.
///
/// Some filter commands use `%f` as a placeholder for the filename.
/// This function performs the substitution.
pub fn substitute_path_in_command(command: &str, path: &str) -> String {
    command.replace("%f", path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use crate::attributes::AttributeStack;

    #[test]
    fn resolve_no_filter() {
        let attrs = AttributeStack::new();
        let config = git_config::ConfigSet::new();
        let result = resolve_filter_command(
            &attrs,
            BStr::new(b"file.txt"),
            FilterDirection::Clean,
            &config,
        );
        assert!(result.is_none());
    }

    #[test]
    fn resolve_filter_with_attrs_but_no_config() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.c filter=indent\n", Path::new(".gitattributes"));
        let config = git_config::ConfigSet::new();

        let result = resolve_filter_command(
            &attrs,
            BStr::new(b"main.c"),
            FilterDirection::Clean,
            &config,
        );
        // filter attribute is set, but no config for filter.indent.clean
        assert!(result.is_none());
    }

    #[test]
    fn substitute_path() {
        assert_eq!(
            substitute_path_in_command("myfilter --file=%f", "src/main.c"),
            "myfilter --file=src/main.c"
        );
        assert_eq!(
            substitute_path_in_command("cat", "src/main.c"),
            "cat"
        );
    }

    #[test]
    fn run_cat_filter() {
        // "cat" should pass content through unchanged
        let input = b"hello world\n";
        let output = run_filter("cat", input).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn run_filter_transforms() {
        // Use tr to convert lowercase to uppercase
        let input = b"hello";
        let output = run_filter("tr a-z A-Z", input).unwrap();
        assert_eq!(output, b"HELLO");
    }

    #[test]
    fn run_filter_nonexistent_command() {
        let result = run_filter("nonexistent_command_xyz_12345", b"test");
        // Should fail because the command doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn apply_filter_no_filter_returns_content() {
        let attrs = AttributeStack::new();
        let config = git_config::ConfigSet::new();
        let content = b"hello world";

        let result = apply_filter(
            &attrs,
            BStr::new(b"file.txt"),
            content,
            FilterDirection::Clean,
            &config,
        )
        .unwrap();
        assert_eq!(result, content);
    }
}

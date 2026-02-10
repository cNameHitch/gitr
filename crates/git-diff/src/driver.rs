//! Diff driver selection from gitattributes.
//!
//! When `diff=<driver>` is set in `.gitattributes`, the diff engine can use
//! `diff.<driver>.command` from config to delegate to an external tool, or
//! apply driver-specific behavior (e.g., `diff=rust` for function context).
//!
//! # Built-in drivers
//!
//! Git has several built-in diff drivers that provide language-aware
//! function-name patterns for hunk headers:
//! - `ada`, `bibtex`, `cpp`, `csharp`, `css`, `dts`, `elixir`, `fortran`,
//!   `fountain`, `golang`, `html`, `java`, `kotlin`, `markdown`, `matlab`,
//!   `objc`, `pascal`, `perl`, `php`, `python`, `ruby`, `rust`, `scheme`,
//!   `tex`
//!
//! # External drivers
//!
//! When `diff.<driver>.command` is set in config, the external command is
//! invoked with 7 arguments:
//! `<command> <old-file> <old-hex> <old-mode> <new-file> <new-hex> <new-mode>`

use std::io::Write;
use std::process::{Command, Stdio};

use bstr::BStr;
use git_index::attributes::AttributeStack;

/// Information about a diff driver resolved from gitattributes + config.
#[derive(Debug, Clone)]
pub struct DiffDriver {
    /// The driver name (e.g., "rust", "python", "custom").
    pub name: String,
    /// External command, if configured via `diff.<name>.command`.
    pub command: Option<String>,
    /// Function name regex pattern for hunk headers, from `diff.<name>.xfuncname`
    /// or a built-in default.
    pub xfuncname: Option<String>,
    /// Word regex pattern for word-diff, from `diff.<name>.wordRegex`.
    pub word_regex: Option<String>,
    /// Whether this driver marks the file as binary (`diff.<name>.binary`).
    pub binary: bool,
}

impl DiffDriver {
    /// Create a driver with just a name and no configuration.
    pub fn named(name: &str) -> Self {
        let mut driver = Self {
            name: name.to_string(),
            command: None,
            xfuncname: None,
            word_regex: None,
            binary: false,
        };
        // Apply built-in defaults
        driver.apply_builtin_defaults();
        driver
    }

    /// Resolve a diff driver for a path from gitattributes and config.
    ///
    /// Returns `None` if no diff driver is set, or if `diff` is unset
    /// (which means "use the default internal diff").
    pub fn resolve(
        attrs: &AttributeStack,
        path: &BStr,
        config: &git_config::ConfigSet,
    ) -> Option<Self> {
        let driver_name = attrs.diff_driver(path)?;
        let name = String::from_utf8_lossy(&driver_name).to_string();

        let mut driver = Self::named(&name);

        // Read config overrides
        let cmd_key = format!("diff.{}.command", name);
        if let Ok(Some(cmd)) = config.get_string(&cmd_key) {
            driver.command = Some(cmd);
        }

        let xfunc_key = format!("diff.{}.xfuncname", name);
        if let Ok(Some(pattern)) = config.get_string(&xfunc_key) {
            driver.xfuncname = Some(pattern);
        }

        let word_key = format!("diff.{}.wordRegex", name);
        if let Ok(Some(pattern)) = config.get_string(&word_key) {
            driver.word_regex = Some(pattern);
        }

        let binary_key = format!("diff.{}.binary", name);
        if let Ok(Some(val)) = config.get_string(&binary_key) {
            driver.binary = val == "true" || val == "yes" || val == "1";
        }

        Some(driver)
    }

    /// Check if this driver has an external command.
    pub fn has_external_command(&self) -> bool {
        self.command.is_some()
    }

    /// Run the external diff command.
    ///
    /// The command is invoked with the standard 7 arguments:
    /// `<command> <old-file> <old-hex> <old-mode> <new-file> <new-hex> <new-mode>`
    ///
    /// Returns the command's stdout output.
    #[allow(clippy::too_many_arguments)]
    pub fn run_external(
        &self,
        old_path: &str,
        old_hex: &str,
        old_mode: &str,
        new_path: &str,
        new_hex: &str,
        new_mode: &str,
        old_content: &[u8],
        new_content: &[u8],
    ) -> Result<Vec<u8>, std::io::Error> {
        let command = self.command.as_ref().ok_or_else(|| {
            std::io::Error::other("no external command configured")
        })?;

        // Write old and new content to temporary files
        let old_tmp = write_temp_file(old_content, old_path)?;
        let new_tmp = write_temp_file(new_content, new_path)?;

        let old_tmp_path = old_tmp.path().to_string_lossy().to_string();
        let new_tmp_path = new_tmp.path().to_string_lossy().to_string();

        // Build the command with arguments
        let full_command = format!(
            "{} {} {} {} {} {} {}",
            command, old_tmp_path, old_hex, old_mode, new_tmp_path, new_hex, new_mode
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(&full_command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        // External diff tools may return non-zero for "differences found"
        // (exit code 1), which is not an error. Only truly fatal errors
        // (e.g., signal) should be propagated.
        Ok(output.stdout)
    }

    /// Apply built-in function name patterns for known languages.
    fn apply_builtin_defaults(&mut self) {
        if self.xfuncname.is_some() {
            return;
        }

        self.xfuncname = match self.name.as_str() {
            "rust" => Some(
                r"^[ \t]*(pub(\([^)]*\))?\s+)?(unsafe\s+)?(async\s+)?(fn|struct|enum|impl|trait|mod|type|const|static)\s+\w+"
                    .to_string(),
            ),
            "python" => {
                Some(r"^[ \t]*((class|def)\s+\w+)".to_string())
            }
            "cpp" | "csharp" | "objc" => Some(
                r"^((\w[\w ]*::)?[[:alpha:]_]\w*\s*\([^;]*)$"
                    .to_string(),
            ),
            "java" | "kotlin" => Some(
                r"^[ \t]*(((public|protected|private|static|abstract|final|synchronized|native)\s+)*[\w<>\[\]]+\s+\w+\s*\()"
                    .to_string(),
            ),
            "ruby" => Some(
                r"^[ \t]*((class|module|def)\s+\w+)".to_string(),
            ),
            "golang" => Some(
                r"^func\s+(\([^)]+\)\s+)?\w+".to_string(),
            ),
            "html" => Some(
                r"^[ \t]*(<[hH][1-6](\s|>)|<(head|body|div|section|article|nav|aside|header|footer|main|form|table|ul|ol|dl|fieldset|details|script|style)\b)"
                    .to_string(),
            ),
            "css" => Some(
                r"^[ \t]*([\w.#@][^{]*)\{".to_string(),
            ),
            "php" => Some(
                r"^[ \t]*(((public|protected|private|static|abstract|final)\s+)*(function)\s+\w+)"
                    .to_string(),
            ),
            "perl" => Some(
                r"^[ \t]*((sub|package)\s+\w+)".to_string(),
            ),
            "tex" => Some(
                r"^[ \t]*(\\(sub)*section|\\chapter|\\(begin|end)\{)".to_string(),
            ),
            "markdown" => Some(
                r"^#{1,6}\s+".to_string(),
            ),
            _ => None,
        };
    }
}

/// Write content to a temporary file for external diff tools.
fn write_temp_file(
    content: &[u8],
    _name_hint: &str,
) -> Result<tempfile::NamedTempFile, std::io::Error> {
    let mut tmp = tempfile::NamedTempFile::new()?;
    tmp.write_all(content)?;
    tmp.flush()?;
    Ok(tmp)
}

/// Check if a path should use an external diff driver.
///
/// Returns the resolved driver if one is configured, or `None` for
/// the default internal diff.
pub fn resolve_diff_driver(
    attrs: &AttributeStack,
    path: &BStr,
    config: &git_config::ConfigSet,
) -> Option<DiffDriver> {
    DiffDriver::resolve(attrs, path, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn builtin_rust_driver() {
        let driver = DiffDriver::named("rust");
        assert_eq!(driver.name, "rust");
        assert!(driver.xfuncname.is_some());
        assert!(driver.xfuncname.as_ref().unwrap().contains("fn"));
        assert!(!driver.has_external_command());
    }

    #[test]
    fn builtin_python_driver() {
        let driver = DiffDriver::named("python");
        assert!(driver.xfuncname.is_some());
        assert!(driver.xfuncname.as_ref().unwrap().contains("def"));
    }

    #[test]
    fn builtin_unknown_driver() {
        let driver = DiffDriver::named("unknown_driver_xyz");
        assert!(driver.xfuncname.is_none());
        assert!(!driver.binary);
    }

    #[test]
    fn resolve_no_attrs() {
        let attrs = AttributeStack::new();
        let config = git_config::ConfigSet::new();
        let result = DiffDriver::resolve(&attrs, BStr::new(b"file.txt"), &config);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_with_attrs() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.rs diff=rust\n", Path::new(".gitattributes"));
        let config = git_config::ConfigSet::new();
        let result = DiffDriver::resolve(&attrs, BStr::new(b"main.rs"), &config);
        assert!(result.is_some());
        let driver = result.unwrap();
        assert_eq!(driver.name, "rust");
        assert!(driver.xfuncname.is_some());
    }

    #[test]
    fn resolve_with_external_command() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.custom diff=mydriver\n", Path::new(".gitattributes"));

        let mut config = git_config::ConfigSet::new();
        config.add_file(git_config::ConfigFile::parse(
            b"[diff \"mydriver\"]\n\tcommand = my-diff-tool\n",
            None,
            git_config::ConfigScope::Local,
        ).unwrap());

        let result = DiffDriver::resolve(&attrs, BStr::new(b"file.custom"), &config);
        assert!(result.is_some());
        let driver = result.unwrap();
        assert_eq!(driver.name, "mydriver");
        assert!(driver.has_external_command());
        assert_eq!(driver.command.as_deref(), Some("my-diff-tool"));
    }

    #[test]
    fn resolve_driver_helper() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.md diff=markdown\n", Path::new(".gitattributes"));
        let config = git_config::ConfigSet::new();

        let result = resolve_diff_driver(&attrs, BStr::new(b"README.md"), &config);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "markdown");
    }

    #[test]
    fn resolve_driver_helper_no_match() {
        let attrs = AttributeStack::new();
        let config = git_config::ConfigSet::new();
        let result = resolve_diff_driver(&attrs, BStr::new(b"file.txt"), &config);
        assert!(result.is_none());
    }
}

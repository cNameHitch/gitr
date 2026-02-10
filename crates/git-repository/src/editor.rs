//! Editor resolution and invocation.

use std::path::Path;
use std::process::Command;

pub struct EditorConfig {
    pub command: String,
}

impl EditorConfig {
    /// Resolve editor from config following git's cascade:
    /// $GIT_EDITOR > core.editor > $VISUAL > $EDITOR > vi
    pub fn from_config(config: &git_config::ConfigSet) -> Self {
        let command = if let Ok(val) = std::env::var("GIT_EDITOR") {
            val
        } else if let Ok(Some(val)) = config.get_string("core.editor") {
            val
        } else if let Ok(val) = std::env::var("VISUAL") {
            val
        } else if let Ok(val) = std::env::var("EDITOR") {
            val
        } else {
            "vi".to_string()
        };

        Self { command }
    }

    /// Open the editor on a file and wait for it to exit.
    pub fn edit_file(&self, path: &Path) -> Result<(), std::io::Error> {
        let parts: Vec<&str> = self.command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(std::io::Error::other(
                "no editor configured",
            ));
        }

        let status = Command::new(parts[0])
            .args(&parts[1..])
            .arg(path)
            .status()?;

        if !status.success() {
            return Err(std::io::Error::other(
                format!("editor '{}' exited with error", self.command),
            ));
        }

        Ok(())
    }
}

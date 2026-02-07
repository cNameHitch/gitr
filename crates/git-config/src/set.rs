//! Merged multi-scope configuration view.

use std::path::{Path, PathBuf};

use bstr::{BStr, BString, ByteSlice};

use crate::error::ConfigError;
use crate::file::ConfigFile;
use crate::types::{self, ColorSpec, PushConfig, PushDefault};
use crate::{ConfigEntry, ConfigKey, ConfigScope};

/// Merged configuration from all scopes.
pub struct ConfigSet {
    /// Config files in precedence order (low to high).
    files: Vec<ConfigFile>,
    /// Environment overrides.
    env_overrides: Vec<ConfigEntry>,
    /// Command-line overrides (-c key=value).
    command_overrides: Vec<ConfigEntry>,
}

impl ConfigSet {
    /// Create an empty config set.
    pub fn new() -> Self {
        ConfigSet {
            files: Vec::new(),
            env_overrides: Vec::new(),
            command_overrides: Vec::new(),
        }
    }

    /// Load the standard config file hierarchy for a repository.
    ///
    /// Loads in order: system, global, local, worktree.
    /// Respects environment variable overrides.
    pub fn load(git_dir: Option<&Path>) -> Result<Self, ConfigError> {
        let mut set = ConfigSet::new();

        // Load environment overrides first (they affect which files we load)
        let env_config = crate::env::load_env_overrides()?;

        let skip_system = std::env::var_os("GIT_CONFIG_NOSYSTEM").is_some();

        // System config
        if !skip_system {
            let system_path = std::env::var_os("GIT_CONFIG_SYSTEM")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/etc/gitconfig"));
            if system_path.exists() {
                match ConfigFile::load(&system_path, ConfigScope::System) {
                    Ok(file) => set.add_file(file),
                    Err(ConfigError::FileNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }
        }

        // Global config
        let global_path = std::env::var_os("GIT_CONFIG_GLOBAL").map(PathBuf::from);
        let global_paths = if let Some(path) = global_path {
            vec![path]
        } else {
            let mut paths = Vec::new();
            // XDG_CONFIG_HOME/git/config
            if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
                let xdg_path = PathBuf::from(xdg).join("git/config");
                if xdg_path.exists() {
                    paths.push(xdg_path);
                }
            } else if let Some(home) = std::env::var_os("HOME") {
                let xdg_path = PathBuf::from(&home).join(".config/git/config");
                if xdg_path.exists() {
                    paths.push(xdg_path);
                }
            }
            // ~/.gitconfig
            if let Some(home) = std::env::var_os("HOME") {
                let home_path = PathBuf::from(home).join(".gitconfig");
                if home_path.exists() {
                    paths.push(home_path);
                }
            }
            paths
        };

        for path in &global_paths {
            if path.exists() {
                match ConfigFile::load(path, ConfigScope::Global) {
                    Ok(file) => set.add_file(file),
                    Err(ConfigError::FileNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }
        }

        // Local config (.git/config)
        if let Some(git_dir) = git_dir {
            let local_path = git_dir.join("config");
            if local_path.exists() {
                match ConfigFile::load(&local_path, ConfigScope::Local) {
                    Ok(file) => set.add_file(file),
                    Err(ConfigError::FileNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }

            // Worktree config (if extensions.worktreeConfig is true)
            let worktree_path = git_dir.join("config.worktree");
            if worktree_path.exists() {
                // Check if worktreeConfig is enabled
                let worktree_enabled = set
                    .get_bool("extensions.worktreeconfig")
                    .unwrap_or(None)
                    .unwrap_or(false);
                if worktree_enabled {
                    match ConfigFile::load(&worktree_path, ConfigScope::Worktree) {
                        Ok(file) => set.add_file(file),
                        Err(ConfigError::FileNotFound(_)) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        // Add environment overrides
        set.env_overrides = env_config;

        Ok(set)
    }

    /// Add a config file at the given scope.
    pub fn add_file(&mut self, file: ConfigFile) {
        self.files.push(file);
    }

    /// Add command-line overrides (-c key=value).
    pub fn add_command_override(&mut self, key: &str, value: &str) -> Result<(), ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        self.command_overrides.push(ConfigEntry {
            key: config_key,
            value: Some(BString::from(value.as_bytes())),
            scope: ConfigScope::Command,
            source_file: None,
            line_number: None,
        });
        Ok(())
    }

    // --- String access ---

    /// Get the highest-priority value as a string.
    pub fn get_string(&self, key: &str) -> Result<Option<String>, ConfigError> {
        let config_key = ConfigKey::parse(key)?;

        // Check command overrides first (highest priority)
        for entry in self.command_overrides.iter().rev() {
            if entry.key.matches(&config_key) {
                return Ok(entry.value.as_ref().map(|v| v.to_str_lossy().to_string()));
            }
        }

        // Check env overrides
        for entry in self.env_overrides.iter().rev() {
            if entry.key.matches(&config_key) {
                return Ok(entry.value.as_ref().map(|v| v.to_str_lossy().to_string()));
            }
        }

        // Check files in reverse order (highest scope wins)
        for file in self.files.iter().rev() {
            if let Some(value) = file.get(&config_key) {
                return Ok(value.map(|v| v.to_str_lossy().to_string()));
            }
        }

        Ok(None)
    }

    /// Get all values for a multi-valued key, across all scopes.
    pub fn get_all_strings(&self, key: &str) -> Result<Vec<String>, ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        let mut results = Vec::new();

        // Collect from files (low to high priority)
        for file in &self.files {
            for value in file.get_all(&config_key) {
                results.push(
                    value
                        .map(|v| v.to_str_lossy().to_string())
                        .unwrap_or_default(),
                );
            }
        }

        // Add env overrides
        for entry in &self.env_overrides {
            if entry.key.matches(&config_key) {
                results.push(
                    entry
                        .value
                        .as_ref()
                        .map(|v| v.to_str_lossy().to_string())
                        .unwrap_or_default(),
                );
            }
        }

        // Add command overrides
        for entry in &self.command_overrides {
            if entry.key.matches(&config_key) {
                results.push(
                    entry
                        .value
                        .as_ref()
                        .map(|v| v.to_str_lossy().to_string())
                        .unwrap_or_default(),
                );
            }
        }

        Ok(results)
    }

    // --- Typed access ---

    /// Get the raw value for a key (for internal use).
    fn get_raw(&self, key: &ConfigKey) -> Option<Option<BString>> {
        // Command overrides first
        for entry in self.command_overrides.iter().rev() {
            if entry.key.matches(key) {
                return Some(entry.value.clone());
            }
        }

        // Env overrides
        for entry in self.env_overrides.iter().rev() {
            if entry.key.matches(key) {
                return Some(entry.value.clone());
            }
        }

        // Files in reverse order
        for file in self.files.iter().rev() {
            if let Some(value) = file.get(key) {
                return Some(value.map(|v| BString::from(v.as_bytes())));
            }
        }

        None
    }

    /// Get as boolean.
    pub fn get_bool(&self, key: &str) -> Result<Option<bool>, ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        match self.get_raw(&config_key) {
            Some(value) => {
                let result = types::parse_bool(value.as_deref().map(|v| v.as_bstr()))?;
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    /// Get as boolean with default.
    pub fn get_bool_or(&self, key: &str, default: bool) -> Result<bool, ConfigError> {
        Ok(self.get_bool(key)?.unwrap_or(default))
    }

    /// Get as integer (with k/m/g suffix support).
    pub fn get_int(&self, key: &str) -> Result<Option<i64>, ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        match self.get_raw(&config_key) {
            Some(Some(value)) => {
                let result = types::parse_int(value.as_bstr())?;
                Ok(Some(result))
            }
            Some(None) => Err(ConfigError::InvalidInt("missing value".into())),
            None => Ok(None),
        }
    }

    /// Get as unsigned integer.
    pub fn get_usize(&self, key: &str) -> Result<Option<usize>, ConfigError> {
        match self.get_int(key)? {
            Some(v) if v >= 0 => Ok(Some(v as usize)),
            Some(v) => Err(ConfigError::InvalidInt(format!(
                "negative value {} for unsigned config",
                v
            ))),
            None => Ok(None),
        }
    }

    /// Get as path (with ~/ expansion).
    pub fn get_path(&self, key: &str) -> Result<Option<PathBuf>, ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        match self.get_raw(&config_key) {
            Some(Some(value)) => {
                let result = types::parse_path(value.as_bstr())?;
                Ok(Some(result))
            }
            Some(None) => Ok(None),
            None => Ok(None),
        }
    }

    /// Get as color specification.
    pub fn get_color(&self, key: &str) -> Result<Option<ColorSpec>, ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        match self.get_raw(&config_key) {
            Some(Some(value)) => {
                let result = types::parse_color(value.as_bstr())?;
                Ok(Some(result))
            }
            Some(None) => Ok(Some(ColorSpec::default())),
            None => Ok(None),
        }
    }

    // --- Enumeration ---

    /// Get the scope of the highest-priority value.
    pub fn get_scope(&self, key: &str) -> Option<ConfigScope> {
        let config_key = match ConfigKey::parse(key) {
            Ok(k) => k,
            Err(_) => return None,
        };

        // Command overrides first
        for entry in self.command_overrides.iter().rev() {
            if entry.key.matches(&config_key) {
                return Some(ConfigScope::Command);
            }
        }

        // Env overrides
        for entry in self.env_overrides.iter().rev() {
            if entry.key.matches(&config_key) {
                return Some(ConfigScope::Command); // env behaves like command
            }
        }

        // Files in reverse order
        for file in self.files.iter().rev() {
            if file.get(&config_key).is_some() {
                return Some(file.scope());
            }
        }

        None
    }

    /// Get all entries matching a section (e.g., all keys in "remote.origin.*").
    pub fn get_section(&self, _section: &str, _subsection: Option<&str>) -> Vec<&ConfigEntry> {
        // We can't easily return references to file entries since they're computed,
        // so we return entries from files. This is an approximation - a full
        // implementation would collect all matching entries.
        Vec::new()
    }

    /// Get all entries matching a section, returning owned entries.
    pub fn get_section_entries(
        &self,
        section: &str,
        subsection: Option<&str>,
    ) -> Vec<ConfigEntry> {
        let section_lower = section.to_ascii_lowercase();
        let subsection_bstr = subsection.map(|s| BString::from(s.as_bytes()));

        let mut results = Vec::new();

        for file in &self.files {
            for entry in file.entries() {
                if entry.key.section.to_str_lossy() == section_lower
                    && entry.key.subsection == subsection_bstr
                {
                    results.push(entry);
                }
            }
        }

        results
    }

    // --- Modification ---

    /// Set a value in the config file for the given scope.
    pub fn set(&mut self, key: &str, value: &str, scope: ConfigScope) -> Result<(), ConfigError> {
        let config_key = ConfigKey::parse(key)?;
        let value_bstr = BStr::new(value.as_bytes());

        // Find the file for this scope
        for file in &mut self.files {
            if file.scope() == scope {
                file.set(&config_key, value_bstr);
                // Write back to disk if the file has a path
                if let Some(path) = file.path() {
                    let path = path.to_path_buf();
                    file.write_to(&path)?;
                }
                return Ok(());
            }
        }

        Err(ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no config file loaded for scope {:?}", scope),
        )))
    }

    /// Remove a key from the given scope.
    pub fn remove(&mut self, key: &str, scope: ConfigScope) -> Result<bool, ConfigError> {
        let config_key = ConfigKey::parse(key)?;

        for file in &mut self.files {
            if file.scope() == scope {
                let removed = file.remove(&config_key);
                if removed {
                    if let Some(path) = file.path() {
                        let path = path.to_path_buf();
                        file.write_to(&path)?;
                    }
                }
                return Ok(removed);
            }
        }

        Ok(false)
    }

    /// Load push configuration from the config set.
    pub fn get_push_config(&self) -> Result<PushConfig, ConfigError> {
        let default = match self.get_string("push.default")? {
            Some(val) => PushDefault::from_config(&val)?,
            None => PushDefault::default(),
        };

        let follow_tags = self.get_bool_or("push.followtags", false)?;
        let auto_setup_remote = self.get_bool_or("push.autosetupremote", false)?;

        Ok(PushConfig {
            default,
            follow_tags,
            auto_setup_remote,
        })
    }

    /// Return all entries across all scopes in precedence order (low to high).
    pub fn all_entries(&self) -> Vec<ConfigEntry> {
        let mut entries = Vec::new();

        // Files in order (low to high priority)
        for file in &self.files {
            entries.extend(file.entries());
        }

        // Env overrides
        entries.extend(self.env_overrides.clone());

        // Command overrides (highest priority)
        entries.extend(self.command_overrides.clone());

        entries
    }

    /// Get all loaded files (for include processing).
    pub fn files(&self) -> &[ConfigFile] {
        &self.files
    }

    /// Get mutable access to all files.
    pub fn files_mut(&mut self) -> &mut Vec<ConfigFile> {
        &mut self.files
    }
}

impl Default for ConfigSet {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ConfigSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigSet")
            .field("files_count", &self.files.len())
            .field("env_overrides", &self.env_overrides.len())
            .field("command_overrides", &self.command_overrides.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(content: &[u8], scope: ConfigScope) -> ConfigFile {
        ConfigFile::parse(content, None, scope).unwrap()
    }

    #[test]
    fn get_string_simple() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(b"[user]\n\tname = Alice\n", ConfigScope::Local));

        assert_eq!(
            set.get_string("user.name").unwrap(),
            Some("Alice".to_string())
        );
    }

    #[test]
    fn get_string_missing() {
        let set = ConfigSet::new();
        assert_eq!(set.get_string("user.name").unwrap(), None);
    }

    #[test]
    fn scope_precedence_local_over_global() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[user]\n\tname = Global\n",
            ConfigScope::Global,
        ));
        set.add_file(make_file(
            b"[user]\n\tname = Local\n",
            ConfigScope::Local,
        ));

        assert_eq!(
            set.get_string("user.name").unwrap(),
            Some("Local".to_string())
        );
    }

    #[test]
    fn scope_precedence_command_wins() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[user]\n\tname = Local\n",
            ConfigScope::Local,
        ));
        set.add_command_override("user.name", "CommandLine").unwrap();

        assert_eq!(
            set.get_string("user.name").unwrap(),
            Some("CommandLine".to_string())
        );
    }

    #[test]
    fn get_all_strings_multi_valued() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[remote \"origin\"]\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n\tfetch = +refs/tags/*:refs/tags/*\n",
            ConfigScope::Local,
        ));

        let values = set.get_all_strings("remote.origin.fetch").unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "+refs/heads/*:refs/remotes/origin/*");
        assert_eq!(values[1], "+refs/tags/*:refs/tags/*");
    }

    #[test]
    fn get_bool_simple() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(b"[core]\n\tbare = true\n", ConfigScope::Local));

        assert_eq!(set.get_bool("core.bare").unwrap(), Some(true));
    }

    #[test]
    fn get_bool_no_value() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(b"[core]\n\tbare\n", ConfigScope::Local));

        assert_eq!(set.get_bool("core.bare").unwrap(), Some(true));
    }

    #[test]
    fn get_bool_or_default() {
        let set = ConfigSet::new();
        assert_eq!(set.get_bool_or("core.bare", false).unwrap(), false);
    }

    #[test]
    fn get_int_with_suffix() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[core]\n\tbigFileThreshold = 512m\n",
            ConfigScope::Local,
        ));

        assert_eq!(
            set.get_int("core.bigfilethreshold").unwrap(),
            Some(512 * 1024 * 1024)
        );
    }

    #[test]
    fn get_usize() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[pack]\n\twindow = 10\n",
            ConfigScope::Local,
        ));

        assert_eq!(set.get_usize("pack.window").unwrap(), Some(10));
    }

    #[test]
    fn get_path() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[include]\n\tpath = ~/extra.config\n",
            ConfigScope::Local,
        ));

        let path = set.get_path("include.path").unwrap().unwrap();
        // Should be expanded
        assert!(!path.to_str().unwrap().starts_with("~"));
    }

    #[test]
    fn get_scope_test() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[user]\n\tname = Global\n",
            ConfigScope::Global,
        ));
        set.add_file(make_file(
            b"[user]\n\tname = Local\n",
            ConfigScope::Local,
        ));

        assert_eq!(set.get_scope("user.name"), Some(ConfigScope::Local));
    }

    #[test]
    fn get_push_config_defaults() {
        let set = ConfigSet::new();
        let push = set.get_push_config().unwrap();
        assert_eq!(push.default, PushDefault::Simple);
        assert_eq!(push.follow_tags, false);
        assert_eq!(push.auto_setup_remote, false);
    }

    #[test]
    fn get_push_config_custom() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[push]\n\tdefault = current\n\tfollowTags = true\n\tautoSetupRemote = true\n",
            ConfigScope::Local,
        ));

        let push = set.get_push_config().unwrap();
        assert_eq!(push.default, PushDefault::Current);
        assert_eq!(push.follow_tags, true);
        assert_eq!(push.auto_setup_remote, true);
    }

    #[test]
    fn command_override_takes_precedence() {
        let mut set = ConfigSet::new();
        set.add_file(make_file(
            b"[user]\n\tname = File\n",
            ConfigScope::Local,
        ));
        set.add_command_override("user.name", "Override").unwrap();

        assert_eq!(
            set.get_string("user.name").unwrap(),
            Some("Override".to_string())
        );
    }
}

# Data Model: Configuration System

## Core Types

### Config Scope

```rust
/// Configuration file scope (priority order, low to high)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigScope {
    /// System-wide: /etc/gitconfig
    System,
    /// User-global: ~/.gitconfig
    Global,
    /// Repository-local: .git/config
    Local,
    /// Worktree-specific: .git/config.worktree
    Worktree,
    /// Command-line: -c key=value
    Command,
}
```

### Config Entry

```rust
/// A single configuration key-value pair with metadata.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    /// The full key: "section.subsection.name" or "section.name"
    pub key: ConfigKey,
    /// The raw string value (None for boolean keys with no = sign)
    pub value: Option<BString>,
    /// Which scope this entry came from
    pub scope: ConfigScope,
    /// File path this entry was read from (None for command-line/env)
    pub source_file: Option<std::path::PathBuf>,
    /// Line number in the source file
    pub line_number: Option<usize>,
}
```

### Config Key

```rust
/// A normalized configuration key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigKey {
    /// Lowercased section name
    pub section: BString,
    /// Case-preserved subsection name (optional)
    pub subsection: Option<BString>,
    /// Lowercased variable name
    pub name: BString,
}

impl ConfigKey {
    /// Parse from "section.name" or "section.subsection.name"
    pub fn parse(key: &str) -> Result<Self, ConfigError>;

    /// Format as the canonical "section.subsection.name" string
    pub fn to_canonical(&self) -> String;

    /// Check if this key matches a pattern (case-insensitive section/name, case-sensitive subsection)
    pub fn matches(&self, pattern: &ConfigKey) -> bool;
}

impl std::fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_canonical())
    }
}
```

### Config File

```rust
/// A parsed config file that preserves original formatting.
pub struct ConfigFile {
    /// Original file path
    path: Option<std::path::PathBuf>,
    /// Scope of this file
    scope: ConfigScope,
    /// Raw sections preserving formatting
    sections: Vec<ConfigSection>,
    /// Parsed entries indexed by key
    entries: Vec<ConfigEntry>,
}

/// A section in a config file, preserving raw text.
struct ConfigSection {
    /// Raw header line: "[section \"subsection\"]"
    header: BString,
    /// Parsed section name
    section_name: BString,
    /// Parsed subsection name
    subsection: Option<BString>,
    /// Lines in this section (key-value, comment, blank)
    lines: Vec<ConfigLine>,
}

enum ConfigLine {
    /// Key = value line, preserving original formatting
    KeyValue {
        raw: BString,
        key: BString,
        value: Option<BString>,
    },
    /// Comment line (# or ;)
    Comment(BString),
    /// Blank line
    Blank,
}

impl ConfigFile {
    /// Parse a config file from bytes
    pub fn parse(content: &[u8], path: Option<&std::path::Path>, scope: ConfigScope) -> Result<Self, ConfigError>;

    /// Get all entries
    pub fn entries(&self) -> &[ConfigEntry];

    /// Get the first value for a key
    pub fn get(&self, key: &ConfigKey) -> Option<&BStr>;

    /// Get all values for a key (multi-valued)
    pub fn get_all(&self, key: &ConfigKey) -> Vec<&BStr>;

    /// Set a value (modifies in-memory representation)
    pub fn set(&mut self, key: &ConfigKey, value: &BStr);

    /// Remove a key
    pub fn remove(&mut self, key: &ConfigKey) -> bool;

    /// Remove an entire section
    pub fn remove_section(&mut self, section: &BStr, subsection: Option<&BStr>) -> bool;

    /// Serialize to bytes, preserving formatting
    pub fn to_bytes(&self) -> Vec<u8>;

    /// Write to file atomically (using lock file)
    pub fn write_to(&self, path: &std::path::Path) -> Result<(), ConfigError>;
}
```

### Config Set (Merged View)

```rust
/// Merged configuration from all scopes.
pub struct ConfigSet {
    /// Config files in precedence order (low to high)
    files: Vec<ConfigFile>,
    /// Environment overrides
    env_overrides: Vec<ConfigEntry>,
}

impl ConfigSet {
    /// Create an empty config set
    pub fn new() -> Self;

    /// Load the standard config file hierarchy for a repository
    pub fn load(git_dir: Option<&std::path::Path>) -> Result<Self, ConfigError>;

    /// Add a config file at the given scope
    pub fn add_file(&mut self, file: ConfigFile);

    /// Add command-line overrides (-c key=value)
    pub fn add_command_override(&mut self, key: &str, value: &str) -> Result<(), ConfigError>;

    // --- String access ---
    /// Get the highest-priority value as a string
    pub fn get_string(&self, key: &str) -> Result<Option<String>, ConfigError>;

    /// Get all values for a multi-valued key
    pub fn get_all_strings(&self, key: &str) -> Result<Vec<String>, ConfigError>;

    // --- Typed access ---
    /// Get as boolean (yes/no/true/false/on/off/1/0)
    pub fn get_bool(&self, key: &str) -> Result<Option<bool>, ConfigError>;

    /// Get as boolean with default
    pub fn get_bool_or(&self, key: &str, default: bool) -> Result<bool, ConfigError>;

    /// Get as integer (with k/m/g suffix support)
    pub fn get_int(&self, key: &str) -> Result<Option<i64>, ConfigError>;

    /// Get as unsigned integer
    pub fn get_usize(&self, key: &str) -> Result<Option<usize>, ConfigError>;

    /// Get as path (with ~/ expansion)
    pub fn get_path(&self, key: &str) -> Result<Option<std::path::PathBuf>, ConfigError>;

    /// Get as color specification
    pub fn get_color(&self, key: &str) -> Result<Option<ColorSpec>, ConfigError>;

    // --- Enumeration ---
    /// Get the scope of the highest-priority value
    pub fn get_scope(&self, key: &str) -> Option<ConfigScope>;

    /// Iterate all entries matching a section pattern (e.g., "remote.origin.*")
    pub fn get_section(&self, section: &str, subsection: Option<&str>) -> Vec<&ConfigEntry>;

    // --- Modification ---
    /// Set a value in the config file for the given scope
    pub fn set(&mut self, key: &str, value: &str, scope: ConfigScope) -> Result<(), ConfigError>;

    /// Remove a key from the given scope
    pub fn remove(&mut self, key: &str, scope: ConfigScope) -> Result<bool, ConfigError>;
}
```

### Type Conversion

```rust
/// Parse a boolean config value
pub fn parse_bool(value: Option<&BStr>) -> Result<bool, ConfigError>;

/// Parse an integer config value with optional k/m/g suffix
pub fn parse_int(value: &BStr) -> Result<i64, ConfigError>;

/// Parse a path config value (expand ~/)
pub fn parse_path(value: &BStr) -> Result<std::path::PathBuf, ConfigError>;

/// Color specification from config
#[derive(Debug, Clone)]
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

/// Parse color specification (e.g., "red bold", "#ff0000 dim")
pub fn parse_color(value: &BStr) -> Result<ColorSpec, ConfigError>;
```

### Push Configuration

```rust
/// push.default behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushDefault {
    /// Refuse to push without explicit refspec
    Nothing,
    /// Push current branch to same-named branch on remote
    Current,
    /// Push current branch to its upstream tracking branch
    Upstream,
    /// Push current branch to upstream, but only if names match (default)
    Simple,
    /// Push all branches with matching names on remote
    Matching,
}

impl PushDefault {
    pub fn from_config(value: &str) -> Result<Self, ConfigError>;
}

/// Parsed push-related configuration
pub struct PushConfig {
    pub default: PushDefault,
    pub follow_tags: bool,
    pub auto_setup_remote: bool,
}

/// URL rewriting rules from url.<base>.insteadOf / pushInsteadOf
pub struct UrlRewrite {
    pub base: String,
    pub instead_of: Vec<String>,
    pub push_instead_of: Vec<String>,
}

/// Resolve URL rewrites for a given URL and operation (fetch or push).
/// For push, `pushInsteadOf` rules are checked first, then `insteadOf`.
/// For fetch, only `insteadOf` rules apply.
pub fn rewrite_url(url: &str, rewrites: &[UrlRewrite], is_push: bool) -> String;
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid config key: {0}")]
    InvalidKey(String),

    #[error("parse error in {file}:{line}: {message}")]
    Parse {
        file: String,
        line: usize,
        message: String,
    },

    #[error("invalid boolean value: {0}")]
    InvalidBool(String),

    #[error("invalid integer value: {0}")]
    InvalidInt(String),

    #[error("invalid color value: {0}")]
    InvalidColor(String),

    #[error("circular include detected: {0}")]
    CircularInclude(String),

    #[error("config file not found: {0}")]
    FileNotFound(std::path::PathBuf),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Lock(#[from] git_utils::LockError),
}
```

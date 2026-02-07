# Data Model: Foundation Utilities

## Core Types

### Byte String Extensions

```rust
// Re-export bstr types as the canonical byte string types
pub use bstr::{BStr, BString, ByteSlice, ByteVec};

/// Extension trait for git-specific byte string operations
pub trait GitBStringExt {
    /// Shell-quote a byte string for safe display
    fn shell_quote(&self) -> BString;
    /// C-style quote (backslash escaping)
    fn c_quote(&self) -> BString;
    /// Check if the string needs quoting for git output
    fn needs_quoting(&self) -> bool;
    /// Trim trailing newlines (like strbuf_rtrim)
    fn rtrim_newlines(&self) -> &BStr;
}
```

### Collections

```rust
/// Sorted list of strings with optional associated data.
/// Equivalent to C git's `string_list` with `SORTED` flag.
pub struct StringList<T = ()> {
    items: Vec<StringListItem<T>>,
    sorted: bool,
    case_insensitive: bool,
}

pub struct StringListItem<T = ()> {
    pub string: BString,
    pub util: T,
}

impl<T> StringList<T> {
    pub fn new_sorted() -> Self;
    pub fn new_unsorted() -> Self;
    pub fn insert(&mut self, string: BString, util: T) -> &mut StringListItem<T>;
    pub fn lookup(&self, string: &BStr) -> Option<&StringListItem<T>>;
    pub fn has_string(&self, string: &BStr) -> bool;
    pub fn for_each<F: FnMut(&StringListItem<T>) -> Result<()>>(&self, f: F) -> Result<()>;
}

/// Priority queue with configurable comparison.
pub struct PriorityQueue<T> {
    heap: std::collections::BinaryHeap<T>,
}
```

### Path Utilities

```rust
/// A git-normalized path (always forward slashes, no trailing slash).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitPath(BString);

impl GitPath {
    /// Create from a byte slice, normalizing separators
    pub fn new(path: &BStr) -> Self;
    /// Join two paths with '/'
    pub fn join(&self, other: &BStr) -> GitPath;
    /// Get the directory portion (like dirname)
    pub fn dirname(&self) -> &BStr;
    /// Get the filename portion (like basename)
    pub fn basename(&self) -> &BStr;
    /// Normalize (remove . and .. components)
    pub fn normalize(&self) -> Result<GitPath>;
    /// Convert to OS path for file system operations
    pub fn to_os_path(&self) -> std::path::PathBuf;
    /// Check if path is absolute
    pub fn is_absolute(&self) -> bool;
    /// Make path relative to a base
    pub fn relative_to(&self, base: &GitPath) -> Result<GitPath>;
}
```

### Wildmatch

```rust
/// Compiled wildmatch pattern for efficient repeated matching.
#[derive(Debug, Clone)]
pub struct WildmatchPattern {
    pattern: BString,
    flags: WildmatchFlags,
}

bitflags::bitflags! {
    pub struct WildmatchFlags: u32 {
        const CASEFOLD = 0x01;
        const PATHNAME = 0x02;  // Don't match '/' with wildcards
    }
}

impl WildmatchPattern {
    pub fn new(pattern: &BStr, flags: WildmatchFlags) -> Self;
    /// Match against a path. Returns true if the pattern matches.
    pub fn matches(&self, text: &BStr) -> bool;
}

/// Standalone match function (like C git's wildmatch())
pub fn wildmatch(pattern: &BStr, text: &BStr, flags: WildmatchFlags) -> bool;
```

### Date Handling

```rust
/// A parsed git date with timezone information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitDate {
    /// Seconds since Unix epoch
    pub timestamp: i64,
    /// Timezone offset in minutes from UTC (e.g., -300 for EST)
    pub tz_offset: i32,
}

/// Supported date output formats
#[derive(Debug, Clone, Copy)]
pub enum DateFormat {
    /// "2 hours ago"
    Relative,
    /// Locale-dependent local time
    Local,
    /// ISO 8601 strict: "2025-01-15T12:00:00+00:00"
    Iso,
    /// ISO 8601 like: "2025-01-15 12:00:00 +0000"
    IsoStrict,
    /// RFC 2822: "Wed, 15 Jan 2025 12:00:00 +0000"
    Rfc2822,
    /// Short: "2025-01-15"
    Short,
    /// Raw: "1736942400 +0000"
    Raw,
    /// Human-readable (relative for recent, absolute for old)
    Human,
    /// Unix timestamp only
    Unix,
}

impl GitDate {
    /// Parse a date string in any git-recognized format
    pub fn parse(input: &str) -> Result<Self>;
    /// Parse the "approxidate" format used by --since/--until
    pub fn parse_approxidate(input: &str) -> Result<Self>;
    /// Parse raw git format: "<timestamp> <tz>"
    pub fn parse_raw(input: &str) -> Result<Self>;
    /// Format in the given style
    pub fn format(&self, fmt: DateFormat) -> String;
    /// Get the current time as a GitDate
    pub fn now() -> Self;
}

/// Author/committer identity with timestamp
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: BString,
    pub email: BString,
    pub date: GitDate,
}

impl Signature {
    /// Parse from git format: "Name <email> timestamp tz"
    pub fn parse(input: &BStr) -> Result<Self>;
    /// Format in git's canonical format
    pub fn to_bytes(&self) -> BString;
}
```

### Lock File

```rust
/// RAII lock file guard. Creates a `.lock` file on construction,
/// atomically renames on commit, removes on drop if not committed.
pub struct LockFile {
    path: std::path::PathBuf,
    lock_path: std::path::PathBuf,
    file: Option<std::fs::File>,
    committed: bool,
}

impl LockFile {
    /// Acquire a lock on the given path. Creates `path.lock`.
    pub fn acquire(path: impl AsRef<std::path::Path>) -> Result<Self>;
    /// Try to acquire without blocking. Returns None if already locked.
    pub fn try_acquire(path: impl AsRef<std::path::Path>) -> Result<Option<Self>>;
    /// Get a mutable reference to the underlying file for writing
    pub fn file_mut(&mut self) -> &mut std::fs::File;
    /// Commit: atomically rename .lock to target
    pub fn commit(self) -> Result<()>;
    /// Rollback: remove .lock file (also happens on Drop)
    pub fn rollback(self) -> Result<()>;
}

impl Drop for LockFile {
    fn drop(&mut self) {
        // Remove .lock file if not committed
    }
}

impl std::io::Write for LockFile {
    // Delegate to internal file
}
```

### Progress Display

```rust
/// Progress display on stderr.
pub struct Progress {
    title: String,
    total: Option<u64>,
    current: u64,
    start_time: std::time::Instant,
    last_update: std::time::Instant,
    delay_ms: u64,
}

impl Progress {
    /// Create a new progress display with a title
    pub fn new(title: &str, total: Option<u64>) -> Self;
    /// Update the progress count
    pub fn update(&mut self, count: u64);
    /// Increment by one
    pub fn tick(&mut self);
    /// Finish and clear the progress line
    pub fn finish(self);
}
```

### Error Types

```rust
/// Base error type for git-utils operations
#[derive(Debug, thiserror::Error)]
pub enum UtilError {
    #[error("lock file error: {0}")]
    Lock(#[from] LockError),

    #[error("date parse error: {0}")]
    DateParse(String),

    #[error("path error: {0}")]
    Path(String),

    #[error("subprocess failed: {command}: {source}")]
    Subprocess {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("unable to create lock file '{path}': already locked")]
    AlreadyLocked { path: std::path::PathBuf },

    #[error("unable to create lock file '{path}': {source}")]
    Create {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unable to commit lock file '{path}': {source}")]
    Commit {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}
```

### Subprocess

```rust
/// Builder for git subprocess execution.
pub struct GitCommand {
    program: OsString,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    stdin_mode: StdioMode,
    stdout_mode: StdioMode,
    stderr_mode: StdioMode,
    working_dir: Option<PathBuf>,
    timeout: Option<Duration>,
}

pub enum StdioMode {
    Inherit,
    Pipe,
    Null,
}

pub struct GitCommandResult {
    pub status: std::process::ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl GitCommand {
    pub fn new(program: impl AsRef<OsStr>) -> Self;
    pub fn arg(self, arg: impl AsRef<OsStr>) -> Self;
    pub fn env(self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> Self;
    pub fn stdin(self, mode: StdioMode) -> Self;
    pub fn stdout(self, mode: StdioMode) -> Self;
    pub fn stderr(self, mode: StdioMode) -> Self;
    pub fn working_dir(self, dir: impl AsRef<Path>) -> Self;
    pub fn timeout(self, duration: Duration) -> Self;
    pub fn run(&self) -> Result<GitCommandResult>;
    pub fn spawn(&self) -> Result<std::process::Child>;
}
```

### Color and Pager

```rust
/// Color configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

/// Standard git colors
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Normal,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Bold,
    Dim,
    Reset,
}

/// Check if the given stream should use color
pub fn use_color(mode: ColorMode, stream: atty::Stream) -> bool;

/// Format text with ANSI color if enabled
pub fn colorize(text: &str, color: Color, enabled: bool) -> String;

/// Setup pager for stdout (respects GIT_PAGER, core.pager, PAGER)
pub fn setup_pager(config_pager: Option<&str>) -> Result<()>;
```

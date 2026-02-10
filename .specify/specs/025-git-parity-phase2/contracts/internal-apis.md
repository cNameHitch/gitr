# Internal API Contracts: Git Behavioral Parity — Phase 2

**Feature**: 025-git-parity-phase2 | **Date**: 2026-02-09

## Overview

This feature has no external APIs (REST, GraphQL, etc.). All contracts are internal Rust API changes across crate boundaries.

## Contract 1: git-config — Config Unset

**Crate**: `git-config`
**Method**: `GitConfig::unset(key: &str, scope: ConfigScope) -> Result<()>`

```rust
/// Remove a configuration key from the specified scope.
/// Returns Ok(()) if the key was removed or didn't exist.
/// Returns Err if the config file cannot be written.
pub fn unset(&mut self, key: &str, scope: ConfigScope) -> Result<()>;
```

**Behavior**:
- Parses the key into section.subsection.name
- Removes the entry from the in-memory config
- Writes the updated config file for the specified scope
- If the key doesn't exist, returns Ok(()) with exit code 5 (matching git)

## Contract 2: git-config — Global Config Access

**Crate**: `git-config`
**Method**: `GitConfig::get_string_from_scope(key: &str, scope: ConfigScope) -> Result<Option<String>>`

```rust
/// Get a config value from a specific scope only.
pub fn get_string_from_scope(&self, key: &str, scope: ConfigScope) -> Result<Option<String>>;

/// Set a config value in a specific scope.
pub fn set(&mut self, key: &str, value: &str, scope: ConfigScope) -> Result<()>;
```

**Behavior**:
- `get_string_from_scope` filters entries by the specified scope
- For `ConfigScope::Global`, reads from `~/.gitconfig`
- `set` with `ConfigScope::Global` creates `~/.gitconfig` if it doesn't exist

## Contract 3: git-utils — Custom Date Format

**Crate**: `git-utils`
**Type**: `DateFormat::Custom(String)`

```rust
pub enum DateFormat {
    // ... existing variants ...
    /// Custom strftime format string (e.g., "%Y-%m-%d %H:%M:%S")
    Custom(String),
}

impl GitDate {
    pub fn format(&self, fmt: DateFormat) -> String;
    // For Custom variant, uses chrono's strftime
}
```

**Behavior**:
- `Custom(fmt)` applies the strftime format string using the commit's timezone offset
- Invalid format strings produce a best-effort output (chrono handles gracefully)

## Contract 4: git-diff — Word Diff Format

**Crate**: `git-diff`
**Type**: `DiffOutputFormat::WordDiff`

```rust
pub enum DiffOutputFormat {
    // ... existing variants ...
    /// Word-level diff using [-removed-]{+added+} markers
    WordDiff,
}
```

**Function**: `git_diff::format::word_diff::format_word_diff(result: &DiffResult) -> String`

**Behavior**:
- Splits each changed line into word tokens (split on whitespace and punctuation boundaries)
- Applies the diff algorithm at the word level
- Formats with `[-removed-]{+added+}` markers (plain mode)
- Unchanged words are emitted as-is
- Context lines are emitted unchanged

## Contract 5: git-ref — Reflog Append

**Crate**: `git-ref`
**Function**: `append_reflog_entry(git_dir: &Path, ref_name: &RefName, entry: &ReflogEntry) -> Result<()>`

```rust
/// Append a reflog entry for the given ref.
/// Creates the reflog file if it doesn't exist.
/// The entry is appended as a new line in the reflog format.
pub fn append_reflog_entry(
    git_dir: &Path,
    ref_name: &RefName,
    entry: &ReflogEntry,
) -> Result<(), RefError>;
```

**Behavior**:
- Creates `logs/` directory structure if needed
- Appends entry in format: `<old-hex> <new-hex> <name> <<email>> <timestamp> <tz>\t<message>\n`
- File is opened in append mode (no locking for now — matches simple git behavior)

## Contract 6: git-revwalk — Fuller Format with Merge Line

**Crate**: `git-revwalk`
**Function**: `format_builtin()` for `BuiltinFormat::Fuller`

**Behavior change**:
- When formatting a merge commit (`parents.len() > 1`) with `Fuller` format
- Insert `Merge: <parent1-abbrev> <parent2-abbrev>` line after the `commit <oid>` line
- Abbreviated parent hashes use 7 characters

## Contract 7: git-revwalk — Graph Drawer Fix

**Crate**: `git-revwalk`
**Method**: `GraphDrawer::draw_commit()`

**Behavior change**:
- On linear history (single active branch), do not emit extra `|` continuation lines between commits
- Only emit `|` lines when there are multiple active columns in the graph

# Research: Configuration System

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| config.c | ~3000 | `parse.rs`, `file.rs`, `set.rs`, `write.rs`, `types.rs` | Monolithic in C; we split into focused modules |
| repo-settings.c | ~500 | `set.rs` | Cached settings derived from config |
| environment.c | ~400 | `env.rs` | GIT_* environment overrides |

## Config File Format Details

### Structure
```ini
# Comment
; Also a comment

[section]
    key = value
    bool-key              # Equivalent to bool-key = true
    multiline = first \
        second line

[section "subsection"]
    key = value

[include]
    path = /path/to/other.config

[includeIf "gitdir:~/work/"]
    path = ~/work/.gitconfig
```

### Key Normalization
- Section names: case-insensitive, lowercased
- Variable names: case-insensitive, lowercased
- Subsection names: case-sensitive, preserved
- Full key format: `section.subsection.variable` or `section.variable`

### Type Rules
- **Boolean**: true/yes/on/1 → true; false/no/off/0/"" → false; key-with-no-value → true
- **Integer**: decimal, with optional k/K (×1024), m/M (×1048576), g/G (×1073741824) suffix
- **Path**: `~/` expanded to $HOME; `~user/` expanded to user's home
- **Color**: space-separated attributes: `red`, `bold`, `ul`, `reverse`, `dim`, `italic`, `strike`, `#rrggbb`, `0-255`

### Scope Order (low to high priority)
1. System: `$(prefix)/etc/gitconfig` or `$GIT_CONFIG_SYSTEM`
2. Global: `~/.gitconfig` or `$XDG_CONFIG_HOME/git/config` or `$GIT_CONFIG_GLOBAL`
3. Local: `.git/config`
4. Worktree: `.git/config.worktree` (if extensions.worktreeConfig=true)
5. Command: `-c key=value` or `GIT_CONFIG_COUNT`/`KEY`/`VALUE`
6. Blob: `--blob=<ref>:path` (rarely used)

### includeIf Conditions
- `gitdir:path` — matches if .git dir is under path (glob patterns supported)
- `gitdir/i:path` — case-insensitive version
- `onbranch:pattern` — matches current branch name
- `hasconfig:remote.*.url:pattern` — matches if any remote URL matches

## gitoxide Reference

`gix-config` provides a comprehensive config implementation:
- Preserves original file content with edit operations
- Supports all include types
- Event-based parser (sections, keys, comments as events)
- Good reference for the preserving-formatting requirement

## Key Porting Challenges

1. **Formatting preservation**: When writing config, existing comments, whitespace, and ordering must be preserved. Only the modified key should change. This requires keeping the raw text alongside parsed values.

2. **Include cycles**: Must track included files to detect circular includes.

3. **Platform differences**: Config file paths differ on Windows, macOS, Linux. System config location varies by installation.

4. **Concurrent access**: Config writes must use lock files to prevent corruption from concurrent git processes.

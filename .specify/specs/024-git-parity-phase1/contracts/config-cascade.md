# Contract: Config Cascade

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## Overview

Defines the configuration file loading order and platform-specific config file locations to match C git's behavior.

## Config Loading Order (lowest to highest priority)

```
1. System config (platform-specific)
2. Global config (~/.gitconfig or $XDG_CONFIG_HOME/git/config)
3. Local config (.git/config)
4. Worktree config (.git/config.worktree) — out of scope
5. Command-line overrides (-c key=value) — existing
```

Higher-priority values override lower-priority values for the same key.

## System Config Paths

### macOS
1. `/Library/Developer/CommandLineTools/usr/share/git-core/gitconfig` (Xcode CLT)
2. `/Applications/Xcode.app/Contents/Developer/usr/share/git-core/gitconfig` (Xcode.app)
3. `/usr/local/etc/gitconfig` (Homebrew prefix)
4. `/opt/homebrew/etc/gitconfig` (Homebrew ARM prefix)

**Discovery order**: Check each path in order, load the first one that exists.

### Linux
1. `/etc/gitconfig`

### Environment Overrides
- `GIT_CONFIG_SYSTEM`: If set, use this path instead of platform defaults
- `GIT_CONFIG_NOSYSTEM`: If set to any value, skip system config entirely

## --local Flag Behavior (FR-020)

When `config --local` is specified:
- Read ONLY from `.git/config`
- Write ONLY to `.git/config`
- Do NOT include system or global config values
- Error if not inside a git repository

## --show-origin Behavior (FR-040)

When `config --show-origin <key>` is specified:
- Output format: `file:<path>\t<key>=<value>`
- The path is relative to the working directory when possible
- Examples:
  ```
  file:.git/config	user.name=Jane Doe
  file:/Users/jane/.gitconfig	core.editor=vim
  file:/Library/Developer/CommandLineTools/usr/share/git-core/gitconfig	credential.helper=osxkeychain
  ```

## --list with --show-origin

When `config --list --show-origin` is specified:
- Each entry shows its origin file
- Entries appear in cascade order (system first, local last)
- Format: `file:<path>\t<key>=<value>` per line

## Init Platform Config (FR-041)

During `git init` on macOS:
```ini
[core]
	repositoryformatversion = 0
	filemode = true
	bare = false
	logallrefupdates = true
	ignorecase = true
	precomposeunicode = true
```

The `ignorecase` and `precomposeunicode` fields are macOS-specific additions.

## Implementation Notes

- Use `#[cfg(target_os = "macos")]` for platform-conditional code
- System config loading must be skippable (for tests: set `GIT_CONFIG_NOSYSTEM=1`)
- The existing test harness already sets `GIT_CONFIG_NOSYSTEM=1` in `pin_env()`, so system config loading won't affect existing tests

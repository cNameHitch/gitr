# Data Model: Git Behavioral Parity — Phase 2

**Feature**: 025-git-parity-phase2 | **Date**: 2026-02-09

## Overview

This feature primarily modifies existing CLI command arguments and output formatting. No new on-disk formats are introduced. Changes span the CLI layer (`git-cli`), date handling (`git-utils`), revision formatting (`git-revwalk`), diff output (`git-diff`), config operations (`git-config`), and reflog recording (`git-ref`).

## Modified Entities

### 1. Cli (git-cli main.rs)

**Location**: `crates/git-cli/src/main.rs`

**Change**: Add `#[command(version)]` to enable `--version` flag.

| Field | Type | Change |
|-------|------|--------|
| `Cli` struct | `#[command]` | Add `version` attribute |

### 2. LogArgs (git-cli)

**Location**: `crates/git-cli/src/commands/log.rs`

**Change**: Add `--date`, `--merges`, `--no-merges` flags. Wire `-- <path>` filtering.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `date` | `Option<String>` | `None` | NEW: Date format (iso, relative, short, default, format:...) |
| `merges` | `bool` | `false` | NEW: Show only merge commits |
| `no_merges` | `bool` | `false` | NEW: Exclude merge commits |

**Path filtering**: The existing `_pathspecs` variable (line 136) must be renamed and used to filter commits by checking if each commit touches files matching the pathspec.

### 3. MergeArgs (git-cli)

**Location**: `crates/git-cli/src/commands/merge.rs`

**Change**: Add `--no-edit` flag.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `no_edit` | `bool` | `false` | NEW: Use auto-generated message without editor |

### 4. RevertArgs (git-cli)

**Location**: `crates/git-cli/src/commands/revert.rs`

**Change**: Add `--no-edit` flag.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `no_edit` | `bool` | `false` | NEW: Use auto-generated message without editor |

### 5. SwitchArgs (git-cli)

**Location**: `crates/git-cli/src/commands/switch.rs`

**Change**: Add `-c` short form to existing `--create` flag.

| Field | Current | Change |
|-------|---------|--------|
| `create` | `#[arg(long)]` | `#[arg(short = 'c', long)]` |

### 6. ShowArgs (git-cli)

**Location**: `crates/git-cli/src/commands/show.rs`

**Change**: Add `-s` short form for `--no-patch`.

| Field | Current | Change |
|-------|---------|--------|
| `no_patch` | `#[arg(long)]` | `#[arg(short = 's', long)]` |

### 7. ConfigArgs (git-cli)

**Location**: `crates/git-cli/src/commands/config.rs`

**Change**: Add `--unset` and `--global` flags.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `unset` | `bool` | `false` | NEW: Remove a config key |
| `global` | `bool` | `false` | NEW: Use global (~/.gitconfig) scope |

### 8. DiffArgs (git-cli)

**Location**: `crates/git-cli/src/commands/diff.rs`

**Change**: Add `--word-diff` flag.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `word_diff` | `Option<Option<String>>` | `None` | NEW: Word-level diff mode (plain/color/porcelain) |

### 9. BranchArgs (git-cli)

**Location**: `crates/git-cli/src/commands/branch.rs`

**Change**: Add `--contains` flag.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `contains` | `Option<String>` | `None` | NEW: Filter branches containing specified commit |

### 10. DateFormat (git-utils)

**Location**: `crates/git-utils/src/date.rs`

**Change**: Add `Custom(String)` variant for `--date=format:%Y-%m-%d`.

| Variant | Description |
|---------|-------------|
| `Custom(String)` | NEW: Custom strftime format string |

### 11. DiffOutputFormat (git-diff)

**Location**: `crates/git-diff/src/lib.rs`

**Change**: Add `WordDiff` variant.

| Variant | Description |
|---------|-------------|
| `WordDiff` | NEW: Word-level diff using `[-removed-]{+added+}` markers |

### 12. GraphDrawer (git-revwalk)

**Location**: `crates/git-revwalk/src/graph.rs`

**Change**: Fix `draw_commit()` to not emit extra `|` lines between commits on linear history.

### 13. GitConfig (git-config)

**Location**: `crates/git-config/src/lib.rs`

**Change**: Add `remove()` / `unset()` method if not present; ensure global config path resolution works for `--global` reads.

| Method | Description |
|--------|-------------|
| `unset(key, scope)` | NEW (if not exists): Remove a key from the specified scope |

## New Behavioral Requirements (No New Types)

### Reflog Recording

Each HEAD-modifying operation must call `append_reflog_entry()` from `git-ref/src/reflog.rs`:

| Operation | Reflog Message Format |
|-----------|-----------------------|
| `commit` | `commit: <subject>` or `commit (initial): <subject>` |
| `checkout` / `switch` | `checkout: moving from <old> to <new>` |
| `reset` | `reset: moving to <ref>` |
| `merge` | `merge <branch>: Fast-forward` or `merge <branch>: Merge made by the 'ort' strategy.` |
| `rebase` | `rebase (start): checkout <upstream>` / `rebase: <subject>` / `rebase (finish): returning to <branch>` |
| `cherry-pick` | `cherry-pick: <subject>` |
| `stash pop` | `checkout: moving from <branch> to <branch>` (restoring stash) |

### Output Formatting Changes

| Command | Current Behavior | Required Behavior |
|---------|-----------------|-------------------|
| `commit` output | `[branch hash] subject` | Add diffstat summary line |
| `commit --amend` output | Same as commit | Add Date line + diffstat |
| `merge` output | `Merge made by the 'ort' strategy.` | Add diffstat after strategy line |
| `cherry-pick` output | `[hash] subject` | `[branch hash] subject` (include branch) |
| `stash pop` output | Minimal | Full working tree status + 40-char hash in Dropped message |
| `reset` (mixed) output | Silent | Show "Unstaged changes after reset:" + file list |
| `reset --hard` output | Silent | Show `HEAD is now at <short-hash> <subject>` |
| `rebase` output | Basic | `Rebasing (N/M)` progress + success message |
| `gc` output | Progress messages | Silent by default |
| `describe` errors | May have doubled `fatal:` | Match git's exact error wording |
| `tag -n` | Missing commit subject for lightweight tags | Show commit subject |
| `show` for annotated tags | 4-space indent on message | No indent |
| `log --pretty=fuller` for merges | Missing `Merge:` line | Add `Merge:` line with abbreviated parents |
| `init` output path | May not resolve symlinks | Resolve symlinks, omit `.git` suffix |
| `status` unstage hint | `git restore --staged` | `git rm --cached` for initial commits |

## State Transitions

No new state machines. The reflog recording is append-only — each operation appends an entry to the appropriate reflog file.

## Validation Rules

- `--date` format must be one of: `iso`, `relative`, `short`, `default`, `raw`, `rfc2822`, `unix`, `human`, or `format:<strftime>`. Invalid formats produce a descriptive error.
- `--unset` requires exactly one key argument and no value argument.
- `--global` without a key is valid (equivalent to `config --list --global`).
- `switch -c` with an existing branch name produces `fatal: a branch named '<name>' already exists`.
- `branch --contains` with an invalid commit ref produces an error.

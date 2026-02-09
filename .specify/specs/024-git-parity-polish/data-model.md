# Data Model: Git Behavioral Parity Polish

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## Overview

This feature modifies existing data structures and argument parsing in the CLI layer to close 42 behavioral gaps with C git. No new on-disk formats are introduced. All changes are to in-memory argument handling, output formatting, and command behavior. No new crates are needed.

## Modified Entities

### 1. DiffArgs (git-cli)

**Location**: `crates/git-cli/src/commands/diff.rs`

**Current state**: Arguments parsed via `parse_diff_args()` which splits on `--` separator only. Arguments before `--` are all treated as commit refs.

**Change**: Add pathspec disambiguation logic.

| Method | Current | Change |
|--------|---------|--------|
| `parse_diff_args(args)` | Split on `--` only | For each arg before `--`: try `resolve_revision()` → if fails and path exists → treat as pathspec. If neither → error with git-matching message. |

**Disambiguation algorithm**:
```
for each arg before '--':
  1. Try resolve_revision(arg, repo)
  2. If Ok → it's a revision
  3. If Err → check Path::exists(arg)
  4. If exists → it's a pathspec
  5. If neither → error "ambiguous argument"
```

### 2. ResetArgs (git-cli)

**Location**: `crates/git-cli/src/commands/reset.rs`

**Current state**: Uses clap's `last = true` attribute on `paths` field. Requires `--` before pathspecs, or `HEAD file.txt` syntax may not work correctly.

**Change**: Apply same disambiguation as diff — bare `reset file.txt` must work.

| Field | Current | Change |
|-------|---------|--------|
| `paths` | `#[arg(last = true)]` | Remove `last = true`, add custom parsing that tries revision first, falls back to pathspec |
| argument parsing | Positional only | First arg: try as revision. Remaining args: pathspecs. If first arg fails as revision and exists as path → all args are pathspecs with implicit HEAD. |

### 3. LogArgs (git-cli)

**Location**: `crates/git-cli/src/commands/log.rs`

**Current state**: Has `--since`/`--until` fields but doesn't filter. No `--decorate` flag. `-N` shorthand not supported.

**Change**: Add filtering and new flags.

| Field | Type | Current | Change |
|-------|------|---------|--------|
| `since` | `Option<String>` | Parsed but unused | Filter commits by `author.date >= parse_date(since)` |
| `until` | `Option<String>` | Parsed but unused | Filter commits by `author.date <= parse_date(until)` |
| `decorate` | `bool` | Not present | NEW: `#[arg(long)]` — enable ref decoration |
| `-N` shorthand | — | Not supported | Handled in `preprocess_args()` in main.rs |

### 4. ShowArgs (git-cli)

**Location**: `crates/git-cli/src/commands/show.rs`

**Current state**: `--stat` shows stat but also shows full diff. No `Merge:` header for merge commits. `--format=oneline` uses abbreviated hash.

**Change**: Fix stat-only mode, add merge header, fix format modes.

| Behavior | Current | Change |
|----------|---------|--------|
| `--stat` | Shows stat AND diff | Show stat only (skip diff when `--stat` is set) |
| Merge header | Not emitted | Emit `Merge: <short1> <short2>` for multi-parent commits |
| `--format=oneline` hash | Abbreviated | Full 40-char hash |
| `--format=raw` message | No indentation | 4-space indent per line |

### 5. FormatOptions / format_commit (git-revwalk)

**Location**: `crates/git-revwalk/src/pretty.rs`

**Current state**: Supports `%H`, `%h`, `%T`, `%t`, `%P`, `%p`, `%an`, `%ae`, `%ad`, `%cn`, `%ce`, `%cd`, `%s`, `%b`, `%B`, `%n`, `%%`, plus date variant specifiers.

**Change**: Add missing decoration placeholders and ref-name data.

| Field/Method | Current | Change |
|-------------|---------|--------|
| `format_commit()` signature | `(commit, oid, format, options)` | Add optional `decorations: Option<&HashMap<ObjectId, Vec<String>>>` parameter |
| `%d` placeholder | Not supported | Output ` (HEAD -> main, tag: v1.0)` with leading space, or empty if no decorations |
| `%D` placeholder | Not supported | Output `HEAD -> main, tag: v1.0` without parens, or empty |

### 6. DateFormat (git-utils)

**Location**: `crates/git-utils/src/date.rs`

**Current state**: `Default` format uses `%e` (space-padded day). Email format uses `%d` (zero-padded day).

**Change**: Fix padding specifiers.

| Format | Current strftime | Correct strftime | Example |
|--------|-----------------|------------------|---------|
| Default | `%a %b %e %H:%M:%S %Y %z` | `%a %b %-e %H:%M:%S %Y %z` | `Mon Feb 9` not `Mon Feb  9` |
| Rfc2822 (email) | `%a, %d %b %Y %H:%M:%S %z` | `%a, %-d %b %Y %H:%M:%S %z` | `9 Feb` not `09 Feb` |

**Note**: chrono's `%-e` and `%-d` produce unpadded output. Alternatively, format the day manually and concatenate.

### 7. ConfigStore (git-config)

**Location**: `crates/git-config/src/lib.rs`

**Current state**: Loads local config (`.git/config`) and global config (`~/.gitconfig`). No system config.

**Change**: Add system config to the cascade.

| Level | Current | Change |
|-------|---------|--------|
| System | Not loaded | Load from platform-specific paths (macOS: `/Library/Developer/CommandLineTools/usr/share/git-core/gitconfig`; Linux: `/etc/gitconfig`). Respect `GIT_CONFIG_NOSYSTEM` env var. |
| Global | Loaded | Unchanged |
| Local | Loaded | Unchanged |

**Load order** (lowest to highest priority): system → global → local

### 8. InitOptions (git-repository)

**Location**: `crates/git-repository/src/lib.rs`

**Current state**: Creates standard directory structure. No platform-specific config. No sample hooks.

**Change**: Add macOS config defaults and sample hook file creation.

| Behavior | Current | Change |
|----------|---------|--------|
| `core.ignorecase` | Not set | Set to `true` on macOS |
| `core.precomposeunicode` | Not set | Set to `true` on macOS |
| Sample hooks | Not created | Create `pre-commit.sample`, `commit-msg.sample`, `pre-push.sample`, etc. in `.git/hooks/` |
| Success message path | Raw path | Resolve symlinks via `std::fs::canonicalize()`, ensure trailing `/` |

## Unchanged Entities

### Commit (git-object)
No changes. Commit structure already supports multiple parents for merge commits.

### RevWalk (git-revwalk)
No structural changes. Date filtering is applied as a post-walk filter in the log command, not in the walker itself.

### IgnorePattern (git-utils/wildmatch)
Minor behavioral fix to directory pattern matching, but no structural change to the `WildmatchPattern` type.

### PackFile / LooseObjectStore
No changes. Object storage layer is unaffected by output formatting fixes.

## State Transitions

None — this feature has no new state machines. All changes are to stateless output formatting and argument parsing.

## Relationships

```
CLI Commands
├── diff.rs ──── uses ──→ resolve_revision() for disambiguation
├── reset.rs ─── uses ──→ resolve_revision() for disambiguation
├── log.rs ───── uses ──→ format_commit() with decorations
├── show.rs ──── uses ──→ format_commit() with decorations
└── config.rs ── uses ──→ ConfigStore (now with system config)

git-revwalk/pretty.rs
└── format_commit() ── receives ──→ Optional decoration map

git-utils/date.rs
└── DateFormat::format() ── uses ──→ chrono strftime with corrected specifiers

git-config/lib.rs
└── ConfigStore::new() ── loads ──→ system + global + local configs

git-repository/lib.rs
└── Repository::init_opts() ── creates ──→ platform config + sample hooks
```

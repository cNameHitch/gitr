# Data Model: Git Command Parity

**Feature**: 020-git-parity | **Date**: 2026-02-07

## Overview

This feature modifies existing data structures and introduces minimal new types to achieve byte-identical output parity with C git. No new on-disk formats are introduced — all changes are to in-memory representations and serialization behavior.

## Modified Entities

### 1. DateFormat (git-utils)

**Location**: `crates/git-utils/src/date.rs`

**Current state**: Enum with variants `Relative`, `Local`, `Iso`, `IsoStrict`, `Rfc2822`, `Short`, `Raw`, `Human`, `Unix`.

**Change**: Add `Default` variant.

| Field | Type | Description |
|-------|------|-------------|
| `Default` | variant | C git's default format: `"%a %b %e %H:%M:%S %Y %z"` using commit's stored timezone offset |

**Format string**: `"%a %b %e %H:%M:%S %Y %z"` applied with `FixedOffset` from the commit's `tz_offset` field (NOT local system time).

**Example output**: `Thu Feb 13 23:31:30 2009 +0000`

**Validation**: Must use the commit's timezone, not the system's local timezone.

### 2. FormatOptions (git-revwalk)

**Location**: `crates/git-revwalk/src/pretty.rs`

**Current state**: Defaults `date_format` to `DateFormat::Iso`.

**Change**: Default `date_format` to `DateFormat::Default`.

| Field | Type | Default | Change |
|-------|------|---------|--------|
| `date_format` | `DateFormat` | `Iso` | → `Default` |
| `abbrev_len` | `usize` | `7` | unchanged |

### 3. PackFile (git-pack)

**Location**: `crates/git-pack/src/pack.rs`

**Current state**: REF_DELTA resolution only searches within the same packfile.

**Change**: Accept an optional external object resolver callback for cross-pack REF_DELTA resolution.

| Method | Current | Change |
|--------|---------|--------|
| `read_object(&self, oid)` | Searches within this pack only | Add `read_object_with_resolver(&self, oid, resolver)` where resolver is `Fn(&ObjectId) -> Option<(ObjectType, Vec<u8>)>` |

### 4. StashEntry (conceptual — no dedicated struct)

**Location**: `crates/git-cli/src/commands/stash.rs`

**Current state**: Overwrites `refs/stash` on each push. Pop reads only `refs/stash`.

**Change**: Use reflog entries on `refs/stash` for stash stack.

| Operation | Current Behavior | New Behavior |
|-----------|-----------------|--------------|
| `push` | Overwrites `refs/stash` | Appends reflog entry to `refs/stash` |
| `pop` | Reads `refs/stash` | Reads `refs/stash` reflog entry N, drops entry |
| `list` | Shows single entry | Reads all reflog entries for `refs/stash` |
| `push --include-untracked` | Not implemented | Creates 3-parent stash commit (HEAD, index, untracked) |

**Stash commit structure** (matching C git):
```
stash commit
├── parent 1: HEAD commit
├── parent 2: index state commit ("index on branch: oid summary")
├── parent 3: untracked files commit (only with --include-untracked)
└── tree: full working tree state
```

### 5. RevisionSuffix (git-revwalk)

**Location**: `crates/git-revwalk/src/range.rs`

**Current state**: Supports `~N` and `^N` suffixes only.

**Change**: Add `^{type}` peeling suffix.

| Suffix | Current | New |
|--------|---------|-----|
| `~N` | Supported | unchanged |
| `^N` | Supported | unchanged |
| `^{tree}` | Not supported | Peel to tree object |
| `^{commit}` | Not supported | Peel to commit (deref tags) |
| `^{blob}` | Not supported | Peel to blob |
| `^{tag}` | Not supported | Peel to tag |
| `^{}` | Not supported | Recursive peel until non-tag |

**Peeling algorithm**:
1. Resolve base revision to OID
2. Read object at OID
3. If object type matches target: return OID
4. If object is tag and target is not `tag`: read tag's target, goto 3
5. If object is commit and target is `tree`: return commit's tree OID
6. Otherwise: error "object cannot be peeled to type"

### 6. PathQuoting (new utility — git-utils)

**Location**: `crates/git-utils/src/path.rs` (new function)

**Purpose**: Implement C git's `core.quotePath` default behavior for path output.

| Function | Signature | Description |
|----------|-----------|-------------|
| `quote_path(path: &[u8]) -> String` | Input: raw byte path. Output: quoted string with octal escapes for non-ASCII | If any byte is non-printable or > 127, wraps in double quotes and escapes those bytes as `\NNN` (octal). Printable ASCII bytes are passed through. Backslash and double-quote are also escaped. |

**Examples**:
- `café.txt` → `"caf\303\251.txt"`
- `naïve.txt` → `"na\303\257ve.txt"`
- `hello.txt` → `hello.txt` (no quoting needed)

## Unchanged Entities

### MergeResult / ConflictEntry (git-merge)
No structural changes. The existing types correctly model merge outcomes. Fixes are behavioral (exit codes, conflict marker format), not data model changes.

### Commit (git-object)
No changes. Two-parent merge commits are already supported. Author/committer timestamps are correctly modeled.

### RemoteConfig (git-protocol)
No structural changes. The existing config model handles `remote.origin.url` and `remote.origin.fetch` correctly.

## State Transitions

### Stash Lifecycle
```
Working tree dirty
    → stash push → refs/stash reflog entry N created, working tree clean
    → stash push → refs/stash reflog entry N+1 created
    → stash list → show all reflog entries
    → stash pop → restore entry 0, drop from reflog
    → stash drop N → drop entry N from reflog
    → stash clear → delete all reflog entries
```

### Merge Lifecycle (existing — no changes)
```
Working tree clean
    → merge feature (FF) → ref advanced, tree checked out
    → merge feature (3-way clean) → merge commit created, tree checked out
    → merge feature (3-way conflict) → exit 1, conflict markers written, MERGE_HEAD set
        → [user resolves] → commit → merge commit created
        → merge --abort → restore ORIG_HEAD
```

## Relationships

```
ObjectDatabase
├── LooseObjectStore
├── PackFile[] ←── now with cross-pack REF_DELTA resolver
│   └── PackIndex (v2)
└── AlternatesStore

FormatOptions
└── DateFormat::Default ←── new default

StashCommand
└── refs/stash reflog[] ←── reflog-based stack

RevisionParser
└── PeelingSuffix ←── new ^{type} support

PathOutput
└── quote_path() ←── new utility for ls-files, status, etc.
```

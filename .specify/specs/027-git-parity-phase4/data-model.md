# Data Model: Git Parity Phase 4

**Feature**: 027-git-parity-phase4 | **Date**: 2026-02-09

## Entities

### InteractiveHunk

A contiguous block of changes within a file that can be individually staged, unstaged, or discarded.

| Field | Type | Description |
|-------|------|-------------|
| header | String | The `@@ -a,b +c,d @@` header line |
| old_start | u32 | Starting line in the original file |
| old_count | u32 | Number of lines from the original file |
| new_start | u32 | Starting line in the modified file |
| new_count | u32 | Number of lines in the modified file |
| lines | Vec\<DiffLine\> | The diff lines (context, add, remove) |
| file_path | BString | Path of the file this hunk belongs to |
| selected | Option\<bool\> | User's selection: true=accept, false=reject, None=undecided |

**Relationships**: Belongs to a file diff. Can be split into sub-hunks.

**Validation Rules**:
- `old_start` and `new_start` are 1-based line numbers
- `lines` must contain at least one added or removed line
- `can_split()`: returns true if there are 2+ change regions separated by 3+ context lines

**State Transitions**:
- Undecided → Accepted (y/a)
- Undecided → Rejected (n/d)
- Undecided → Split (s) → produces multiple sub-InteractiveHunks
- Undecided → Edited (e) → replaced with user-modified hunk

**Existing Location**: `crates/git-cli/src/interactive.rs` — the hunk data is currently inlined in the `select_hunks()` function. Formalize as a struct if not already.

---

### InteractiveMode

The context for an interactive patch operation, determining how hunks are computed and applied.

| Field | Type | Description |
|-------|------|-------------|
| mode | PatchMode | Which operation is being performed |
| source | TreeSource | Where to read the "before" state from |
| target | ApplyTarget | Where to write the selected changes |

**PatchMode enum**:
- `AddPatch` — diff: worktree vs index; apply: selected hunks to index (`add -p`)
- `ResetPatch` — diff: index vs HEAD; apply: reverse selected hunks from index (`reset -p`)
- `CheckoutPatch` — diff: worktree vs index (or HEAD); apply: reverse selected hunks to worktree (`checkout -p`)
- `RestorePatch` — diff: worktree vs source; apply: reverse selected hunks to worktree (`restore -p`)
- `StashPatch` — diff: worktree vs index; apply: selected hunks to stash, reverse from worktree (`stash -p`)

**TreeSource enum**:
- `Index` — the staging area
- `Head` — HEAD commit's tree
- `Commit(ObjectId)` — arbitrary commit tree (for `restore --source=`)

**ApplyTarget enum**:
- `Index` — write changes to the staging area
- `Worktree` — write changes to the working tree
- `Both` — write to both (stash -p: stash selected, reverse from worktree)

---

### RebaseTodoEntry

A single action line in an interactive rebase todo list.

| Field | Type | Description |
|-------|------|-------------|
| action | RebaseAction | The action to perform |
| commit_id | Option\<ObjectId\> | The commit hash (None for exec/break/label/reset) |
| short_hash | Option\<String\> | Abbreviated commit hash for display |
| subject | String | Commit subject line or exec command |
| original_line | String | The raw line from the todo file (for round-trip) |

**RebaseAction enum**:
- `Pick` — apply commit as-is
- `Reword` — apply commit, open editor for message
- `Edit` — apply commit, pause for user amendment
- `Squash` — combine with previous commit, edit combined message
- `Fixup` — combine with previous commit, keep previous message
- `Drop` — skip this commit
- `Exec(String)` — run shell command
- `Break` — pause rebase for user interaction
- `Label(String)` — label current HEAD for later reference
- `Reset(String)` — reset HEAD to labeled position
- `Merge` — create merge commit

**Validation Rules**:
- `Pick`, `Reword`, `Edit`, `Squash`, `Fixup`, `Drop` require a commit_id
- `Exec` requires a non-empty command string
- `Squash`/`Fixup` cannot be the first entry (nothing to combine with)
- Lines starting with `#` are comments (preserved but not executed)
- Empty todo list (all lines deleted/commented) aborts the rebase

**State Transitions (per entry)**:
- Pending → InProgress → Done
- Pending → Skipped (if user changes action to Drop)
- InProgress → Conflicted → Resolved → Done (if conflict occurs)

**Existing Location**: `crates/git-merge/src/sequencer.rs` — `SequencerAction` enum already has Pick, Revert, Edit, Squash, Fixup, Exec. Extend with Reword, Drop, Break, Label, Reset, Merge.

---

### RebaseState

Persistent state for an in-progress interactive rebase.

| Field | Type | Description |
|-------|------|-------------|
| todo_entries | Vec\<RebaseTodoEntry\> | Full todo list |
| done_count | usize | Number of entries already processed |
| original_head | ObjectId | HEAD before rebase started (for abort) |
| onto | ObjectId | The commit we're rebasing onto |
| head_name | Option\<String\> | Branch name being rebased (None if detached) |
| current_fixup_message | Option\<String\> | Accumulated message for squash/fixup chain |
| autosquash | bool | Whether autosquash reordering was applied |

**Storage**: `.git/rebase-merge/` directory with files:
- `interactive` — marker file indicating interactive mode
- `todo` — remaining entries to process
- `done` — completed entries
- `head-name` — branch name
- `onto` — target commit
- `orig-head` — original HEAD for abort
- `message` — accumulated squash/fixup message
- `author-script` — author info for current commit
- `amend` — marker if current commit should be amended

---

### ScheduledTask

Platform-specific background maintenance registration.

| Field | Type | Description |
|-------|------|-------------|
| repo_path | PathBuf | Absolute path to the repository |
| schedule | MaintenanceSchedule | How often to run |
| platform | Platform | Which scheduler to use |

**MaintenanceSchedule enum**:
- `Hourly` — run `gitr maintenance run --auto` every hour
- `Daily` — run daily
- `Weekly` — run weekly

**Platform enum**:
- `Launchd` — macOS: plist at `~/Library/LaunchAgents/org.git-scm.git.maintenance.plist`
- `Cron` — Linux: crontab entry
- `Systemd` — Linux: systemd user timer

**Validation Rules**:
- `repo_path` must be an absolute path to an existing git repository
- Only one scheduled task per repository (idempotent registration)

---

### SubtreeMapping

Path prefix mapping for subtree merge strategy.

| Field | Type | Description |
|-------|------|-------------|
| prefix | BString | Subdirectory path in the parent repo (e.g., `lib/`) |
| match_score | f64 | Heuristic score (0.0-1.0) indicating how well the subtree matches |

**Detection Algorithm**:
1. For each subdirectory in "ours" tree, compare its entries against the root of "theirs" tree
2. Score = (matching entries) / (total entries in theirs)
3. Select subdirectory with highest score above threshold (0.5)
4. If `--subtree=<prefix>` is specified, use that directly (skip detection)

**Validation Rules**:
- `prefix` must end with `/` (directory separator)
- `match_score` must be > 0.5 for automatic detection to succeed

---

### WhitespaceError

A whitespace violation detected by `diff --check`.

| Field | Type | Description |
|-------|------|-------------|
| file_path | BString | File containing the error |
| line_number | u32 | Line number in the new file |
| error_type | WhitespaceErrorType | Kind of whitespace error |
| line_content | BString | The offending line content |

**WhitespaceErrorType enum**:
- `TrailingWhitespace` — spaces/tabs at end of line
- `SpaceBeforeTab` — space followed by tab in indentation (configurable)
- `IndentWithNonTab` — indentation uses spaces instead of tabs (configurable via `core.whitespace`)
- `TabInIndent` — tab in indentation when `tabwidth` configured (configurable)
- `BlankAtEof` — blank lines at end of file

**Output Format**: `{file_path}:{line_number}: {error_description}.`

---

### SymmetricDiffCommit

A commit annotated with symmetric diff metadata for log output.

| Field | Type | Description |
|-------|------|-------------|
| commit_id | ObjectId | The commit hash |
| side | DiffSide | Which side of the symmetric diff |
| patch_id | Option\<ObjectId\> | Hash of the commit's diff (for cherry equivalence) |
| is_equivalent | bool | True if patch-id matches a commit on the other side |
| source_ref | Option\<String\> | Ref through which this commit was discovered (--source) |

**DiffSide enum**:
- `Left` — reachable from the left side only (displayed as `<`)
- `Right` — reachable from the right side only (displayed as `>`)

**Rendering with flags**:
- `--left-right`: prefix with `<` or `>`
- `--cherry-pick`: omit commits where `is_equivalent == true`
- `--cherry-mark`: prefix equivalent commits with `=`, non-equivalent with `+`
- `--source`: append source_ref after commit hash

## Relationships

```text
InteractiveMode 1──* InteractiveHunk        (mode determines how hunks are computed/applied)
RebaseState     1──* RebaseTodoEntry         (state tracks the todo list)
Sequencer       1──1 RebaseState             (sequencer manages rebase lifecycle)
MergeOptions    1──1 SubtreeMapping          (subtree strategy uses the mapping)
DiffResult      1──* WhitespaceError         (--check mode detects errors in diff)
RevWalk         1──* SymmetricDiffCommit     (symmetric diff annotates walk results)
```

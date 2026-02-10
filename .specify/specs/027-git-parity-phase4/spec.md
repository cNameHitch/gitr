# Feature Specification: Git Parity Phase 4 — Remaining Behavioral Gaps

**Feature Branch**: `027-git-parity-phase4`
**Created**: 2026-02-09
**Status**: Draft
**Input**: User description: "Complete all remaining git behavioral parity gaps: 9 stub subcommands, 9 no-op flags, 7 interactive modes, and 3 incomplete core engines."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - No-Op Flag Completion (Priority: P1)

A developer runs gitr commands with flags they routinely use in git — `gitr log --follow -- file.txt`, `gitr pull --rebase`, `gitr log --left-right main...feature`, `gitr diff --no-index a.txt b.txt`, or `gitr diff --check` — and expects them to work correctly. Currently these flags are accepted but silently do nothing or fall back to different behavior, producing incorrect or misleading output without any indication to the user.

**Why this priority**: Silent behavioral divergence is the most dangerous class of parity gap. Users believe they are getting correct results when they are not. A warning message provides some protection, but the underlying functionality must be implemented to achieve true parity.

**Independent Test**: Run `gitr log --follow -- renamed-file.txt` in a repo where the file was renamed and verify history continues across the rename; run `gitr pull --rebase origin main` and verify the local branch is rebased (not merged) onto the fetched commits.

**Acceptance Scenarios**:

1. **Given** a file `new.txt` that was renamed from `old.txt`, **When** `gitr log --follow -- new.txt` is run, **Then** commits affecting both `new.txt` and `old.txt` appear in the log (history crosses the rename boundary).
2. **Given** a symmetric diff range `main...feature`, **When** `gitr log --left-right main...feature` is run, **Then** each commit is prefixed with `<` (reachable from main only) or `>` (reachable from feature only).
3. **Given** the same symmetric diff range, **When** `gitr log --cherry-pick main...feature` is run, **Then** commits that produce identical diffs on both sides are omitted.
4. **Given** the same range, **When** `gitr log --cherry-mark main...feature` is run, **Then** equivalent commits are prefixed with `=` and non-equivalent with `+`.
5. **Given** a branching history, **When** `gitr log --ancestry-path A..B` is run, **Then** only commits on the direct ancestry path between A and B are shown (side branches excluded).
6. **Given** multiple refs, **When** `gitr log --source --all` is run, **Then** each commit line includes the ref name through which it was reached.
7. **Given** two files `a.txt` and `b.txt` outside any git repository, **When** `gitr diff --no-index a.txt b.txt` is run, **Then** a unified diff comparing the two files is produced (exit code 0 if identical, 1 if different).
8. **Given** a file with trailing whitespace added, **When** `gitr diff --check` is run, **Then** whitespace errors are reported with line numbers and the command exits with code 2.
9. **Given** a local branch behind its upstream, **When** `gitr pull --rebase` is run, **Then** local commits are rebased on top of the fetched upstream (not merged), matching `git pull --rebase` behavior.

---

### User Story 2 - Stub Subcommand Implementation (Priority: P1)

A developer or automation script invokes gitr subcommands like `gitr remote prune origin`, `gitr remote get-url origin`, `gitr reflog expire --all`, or `gitr maintenance start` and expects them to perform real work. Currently these commands accept arguments but print "not yet implemented" messages or do nothing, breaking workflows that depend on them.

**Why this priority**: Stub commands that print error messages are immediate workflow blockers. Unlike no-op flags (which silently degrade), these actively tell the user the feature is missing, making gitr unsuitable as a git replacement in any environment that uses these commands.

**Independent Test**: Run `gitr remote prune origin` after deleting a remote branch and verify stale tracking refs are removed; run `gitr remote get-url origin` and verify the URL is printed.

**Acceptance Scenarios**:

1. **Given** a remote `origin` with a default branch `main`, **When** `gitr remote set-head origin main` is run, **Then** `refs/remotes/origin/HEAD` is set as a symbolic ref pointing to `refs/remotes/origin/main`.
2. **Given** a remote `origin` with a deleted branch `old-feature`, **When** `gitr remote prune origin` is run, **Then** the stale tracking ref `refs/remotes/origin/old-feature` is removed.
3. **Given** multiple remotes configured, **When** `gitr remote update` is run, **Then** all remotes are fetched (equivalent to `gitr fetch --all`).
4. **Given** a remote `origin`, **When** `gitr remote set-branches origin main develop` is run, **Then** the fetch refspec for `origin` is updated to only track `main` and `develop`.
5. **Given** a remote `origin` with URL `https://github.com/user/repo.git`, **When** `gitr remote get-url origin` is run, **Then** the URL `https://github.com/user/repo.git` is printed to stdout.
6. **Given** a reflog with entries older than 90 days, **When** `gitr reflog expire --expire=90.days.ago --all` is run, **Then** entries older than 90 days are removed from all reflogs.
7. **Given** a reflog entry `HEAD@{3}`, **When** `gitr reflog delete HEAD@{3}` is run, **Then** that specific reflog entry is removed.
8. **Given** a repository, **When** `gitr maintenance start` is run, **Then** a background maintenance schedule is registered (cron job on Linux, launchd plist on macOS).
9. **Given** background maintenance is running, **When** `gitr maintenance stop` is run, **Then** the scheduled maintenance is unregistered and no further background tasks run.

---

### User Story 3 - Incomplete Core Engine Completion (Priority: P1)

A developer uses advanced merge workflows — octopus merges with 3+ branches, subtree merges to integrate a sub-project, or aborting a multi-commit cherry-pick/revert — and expects them to work correctly. Currently the subtree strategy silently delegates to ORT (ignoring subtree shift), octopus fails for 3+ heads, and sequencer abort does not restore the original HEAD.

**Why this priority**: These are correctness bugs in existing functionality. Users who invoke `--strategy=subtree` or abort a cherry-pick expect the documented behavior. Incorrect merge results or failure to restore state can cause data loss or silent corruption.

**Independent Test**: Run `gitr merge --strategy=subtree sub-project-branch` and verify the sub-project tree is shifted into the correct subdirectory; run a multi-commit `gitr cherry-pick A B C`, trigger a conflict, then `gitr cherry-pick --abort` and verify HEAD is restored to the pre-cherry-pick state.

**Acceptance Scenarios**:

1. **Given** a sub-project merged at `lib/` subdirectory, **When** `gitr merge -s subtree sub-project-branch` is run, **Then** the merge correctly shifts the sub-project tree into `lib/` (not merged at root).
2. **Given** branches A, B, and C diverging from a common ancestor, **When** `gitr merge A B C` (octopus merge) is run, **Then** all three branches are merged into a single merge commit with 4 parents (current + A + B + C), matching git's octopus behavior.
3. **Given** the octopus merge encounters a content conflict, **When** the merge is attempted, **Then** the merge is aborted with an error (octopus strategy does not handle conflicts, matching git).
4. **Given** a multi-commit cherry-pick `gitr cherry-pick A B C` that conflicts on commit B, **When** `gitr cherry-pick --abort` is run, **Then** HEAD is restored to the commit it pointed to before the cherry-pick began, the index is restored, and all sequencer state is cleaned up.
5. **Given** a multi-commit revert that conflicts, **When** `gitr revert --abort` is run, **Then** HEAD and the index are both restored to the pre-revert state.

---

### User Story 4 - Interactive Patch Modes (Priority: P2)

A developer uses `gitr add -p`, `gitr checkout -p`, `gitr reset -p`, `gitr restore -p`, or `gitr stash -p` to interactively select individual hunks for staging, unstaging, or discarding. They also use `gitr clean -i` to interactively select files to remove. The interactive prompts, hunk display, and key bindings match git's behavior.

**Why this priority**: Interactive patch modes are a power-user feature used less frequently than basic commands, but they represent a significant UX gap. Users who rely on `add -p` for clean commits cannot switch to gitr without this feature. The existing `interactive.rs` module provides a foundation but lacks manual hunk editing.

**Independent Test**: Modify a file with 3 separate hunks, run `gitr add -p`, stage only the first hunk (y/n/n), then run `gitr diff --cached` to verify only the first hunk is staged.

**Acceptance Scenarios**:

1. **Given** a file with 3 modified hunks, **When** `gitr add -p` is run, **Then** each hunk is presented individually with the prompt `Stage this hunk [y,n,q,a,d,s,e,?]?` matching git's interactive staging.
2. **Given** a hunk is presented during `add -p`, **When** the user presses `s`, **Then** the hunk is split into smaller sub-hunks (if splittable) and each is presented individually.
3. **Given** a hunk is presented during `add -p`, **When** the user presses `e`, **Then** an editor opens with the hunk in unified diff format for manual editing, and the edited hunk is applied on save.
4. **Given** staged and unstaged changes, **When** `gitr reset -p` is run, **Then** each staged hunk is presented and the user can choose to unstage individual hunks.
5. **Given** changes in the working tree, **When** `gitr checkout -p -- file.txt` is run, **Then** each hunk is presented and the user can choose to discard individual hunks.
6. **Given** changes in the working tree, **When** `gitr restore -p -- file.txt` is run, **Then** each hunk is presented and the user can choose to restore individual hunks from the source.
7. **Given** staged and unstaged changes, **When** `gitr stash -p` is run, **Then** each hunk is presented and only selected hunks are stashed.
8. **Given** untracked files, **When** `gitr clean -i` is run, **Then** files are listed with numbers and the user can select which to remove via interactive menu matching git's clean interactive mode.

---

### User Story 5 - Interactive Rebase (Priority: P2)

A developer runs `gitr rebase -i HEAD~5` to reorder, squash, edit, or drop recent commits using an editor-based workflow. The todo list format, available actions, and conflict resolution flow match git's interactive rebase exactly.

**Why this priority**: Interactive rebase is one of git's most powerful features for history cleanup. While less frequent than basic operations, it is a critical workflow for maintaining clean commit histories and is a hard requirement for many teams' contribution guidelines.

**Independent Test**: Run `gitr rebase -i HEAD~3`, change the second commit's action from `pick` to `squash` in the editor, save and close, then verify the resulting history has 2 commits with the squashed commit's changes combined into the first.

**Acceptance Scenarios**:

1. **Given** 5 recent commits, **When** `gitr rebase -i HEAD~5` is run, **Then** the configured editor opens with a todo list showing each commit with `pick` action, in oldest-first order.
2. **Given** an interactive rebase todo list, **When** the user changes `pick` to `reword` for a commit, **Then** during replay the editor opens for that commit to edit its message.
3. **Given** an interactive rebase todo list, **When** the user changes `pick` to `squash` for a commit, **Then** that commit's changes are combined with the preceding commit and the editor opens to combine messages.
4. **Given** an interactive rebase todo list, **When** the user changes `pick` to `fixup` for a commit, **Then** that commit's changes are combined with the preceding commit and the original message is kept.
5. **Given** an interactive rebase todo list, **When** the user changes `pick` to `edit` for a commit, **Then** the rebase pauses after applying that commit, allowing the user to amend it before running `gitr rebase --continue`.
6. **Given** an interactive rebase todo list, **When** the user changes `pick` to `drop` (or deletes the line), **Then** that commit is omitted from the rebased history.
7. **Given** an interactive rebase todo list, **When** the user adds `exec make test` after a line, **Then** the specified command is executed after that commit is applied.
8. **Given** a conflict during interactive rebase, **When** the user resolves and runs `gitr rebase --continue`, **Then** the rebase resumes from the next todo entry.
9. **Given** `rebase.autosquash=true` in config, **When** `gitr rebase -i` is run with commits whose messages start with `fixup!` or `squash!`, **Then** those commits are automatically reordered and marked with the appropriate action.

---

### Edge Cases

- What happens when `gitr log --follow` is used with multiple paths? (Error: `--follow` requires exactly one path, matching git)
- What happens when `gitr diff --no-index` is given a directory? (Recursively diff all files in both directories, matching git)
- What happens when `gitr remote prune origin` finds no stale refs? (Silent success, no output, exit 0)
- What happens when `gitr remote get-url --push origin` is run but no push URL is configured? (Falls back to fetch URL, matching git)
- What happens when `gitr reflog expire --expire=now --all` is run? (All reflog entries except the current tip are removed)
- What happens when `gitr maintenance start` is run but the system scheduler is not available? (Error message explaining the platform limitation)
- What happens when octopus merge encounters a tree conflict (directory/file)? (Merge is aborted — octopus does not handle conflicts)
- What happens when `gitr cherry-pick --abort` is run with no cherry-pick in progress? (Error: "no cherry-pick in progress")
- What happens when a hunk cannot be split during `add -p` (single-line change)? (The `s` option is not offered, matching git)
- What happens when the user provides an invalid edit during `add -p -e`? (Error message and re-prompt, matching git)
- What happens when `gitr rebase -i` is run on a branch with merge commits? (Merge commits are dropped by default unless `--rebase-merges` is specified)
- What happens when `gitr pull --rebase` encounters conflicts? (Rebase pauses, user resolves, then runs `gitr rebase --continue`)
- What happens when `gitr log --ancestry-path` is used without a range? (Error: `--ancestry-path` requires a commit range)

## Requirements *(mandatory)*

### Functional Requirements

**No-Op Flag Completion**

- **FR-001**: System MUST implement `log --follow` to track file history across renames by detecting rename operations and continuing traversal through the pre-rename path
- **FR-002**: System MUST implement `log --left-right` to prefix commits with `<` or `>` indicating which side of a symmetric difference they are reachable from
- **FR-003**: System MUST implement `log --cherry-pick` to omit commits from symmetric diff output that produce identical patches on both sides
- **FR-004**: System MUST implement `log --cherry-mark` to prefix equivalent commits with `=` and non-equivalent commits with `+` in symmetric diff output
- **FR-005**: System MUST implement `log --ancestry-path` to restrict output to commits that are on the direct ancestry chain between the two endpoints of a range
- **FR-006**: System MUST implement `log --source` to annotate each commit with the ref name through which it was discovered during traversal
- **FR-007**: System MUST implement `diff --no-index` to compare two arbitrary filesystem paths without requiring a git repository, producing unified diff output with exit code 1 for differences
- **FR-008**: System MUST implement `diff --check` to detect and report whitespace errors (trailing whitespace, mixed tabs/spaces, blank lines at EOF) with line numbers, exiting with code 2 when errors are found
- **FR-009**: System MUST implement `pull --rebase` to fetch and then rebase local commits onto the upstream branch instead of merging

**Stub Subcommand Implementation**

- **FR-010**: System MUST implement `remote set-head <remote> <branch>` to set `refs/remotes/<remote>/HEAD` as a symbolic ref, with `--auto` to query the remote for its default branch and `--delete` to remove the symbolic ref
- **FR-011**: System MUST implement `remote prune <remote>` to remove local tracking refs for branches that no longer exist on the remote, with `--dry-run` support
- **FR-012**: System MUST implement `remote update [group]` to fetch from all configured remotes (or a specified remote group), with `--prune` support
- **FR-013**: System MUST implement `remote set-branches <remote> <branch>...` to update the fetch refspec for the named remote, with `--add` to append rather than replace
- **FR-014**: System MUST implement `remote get-url <remote>` to print the configured URL, with `--push` to show push URL and `--all` to show all URLs
- **FR-015**: System MUST implement `reflog expire` to remove reflog entries older than a configurable threshold, with `--expire=<time>`, `--expire-unreachable=<time>`, and `--all` flags
- **FR-016**: System MUST implement `reflog delete` to remove specific reflog entries by qualified name (e.g., `HEAD@{2}`), rewriting subsequent entry indices
- **FR-017**: System MUST implement `maintenance start` to register background maintenance using the platform's scheduler (launchd on macOS, cron/systemd on Linux)
- **FR-018**: System MUST implement `maintenance stop` to unregister and disable background maintenance previously set up by `maintenance start`

**Interactive Patch Modes**

- **FR-020**: System MUST implement interactive hunk selection for `add -p` with prompts matching git's format: `Stage this hunk [y,n,q,a,d,s,e,?]?`
- **FR-021**: The `s` (split) action MUST split the current hunk into smaller sub-hunks at non-adjacent change boundaries and present each individually
- **FR-022**: The `e` (edit) action MUST open the configured editor with the hunk in unified diff format, validate the edited hunk, and apply the result
- **FR-023**: System MUST implement `reset -p` for interactive unstaging of individual hunks from the index
- **FR-024**: System MUST implement `checkout -p` for interactive discard of individual working tree hunks
- **FR-025**: System MUST implement `restore -p` for interactive restoration of individual hunks from a specified source
- **FR-026**: System MUST implement `stash -p` for interactive selection of hunks to stash
- **FR-027**: System MUST implement `clean -i` with git's interactive menu: numbered file list, select/deselect by number or pattern, confirm before deletion

**Interactive Rebase**

- **FR-030**: System MUST implement `rebase -i` opening the configured editor with a todo list in git's format: `<action> <short-hash> <subject>` per line, oldest-first
- **FR-031**: System MUST support all standard interactive rebase actions: `pick`, `reword`, `edit`, `squash`, `fixup`, `drop`, `exec`, `break`, `label`, `reset`, `merge`
- **FR-032**: System MUST support `--autosquash` to automatically reorder and mark `fixup!` and `squash!` commits
- **FR-033**: System MUST support `--exec <cmd>` to insert `exec` directives after each `pick` line in the todo list
- **FR-034**: When `edit` action pauses the rebase, system MUST set up state so that `gitr rebase --continue` resumes from the next entry after the user amends
- **FR-035**: When a conflict occurs during interactive rebase, system MUST pause, allow resolution, and resume via `--continue`, skip via `--skip`, or abort via `--abort`

**Core Engine Completion**

- **FR-040**: System MUST implement subtree merge strategy with proper subtree shift detection — identifying the subdirectory where the sub-project tree is mapped and adjusting paths during merge
- **FR-041**: System MUST implement octopus merge strategy for 3 or more heads by iteratively merging each head, aborting immediately if any step produces a conflict
- **FR-042**: System MUST implement sequencer abort (`cherry-pick --abort`, `revert --abort`) to restore HEAD to the commit recorded at the start of the operation, restore the index, and clean up all sequencer state files

### Key Entities

- **InteractiveHunk**: A contiguous block of changes within a file that can be individually staged, unstaged, or discarded. Supports split into sub-hunks and manual editing.
- **RebaseTodoEntry**: A single action line in an interactive rebase todo list, consisting of an action keyword, a commit reference, and optional parameters.
- **ScheduledTask**: A platform-specific background maintenance registration (launchd plist, cron entry, or systemd timer) that runs `gitr maintenance run --auto` on a schedule.
- **SubtreeMapping**: The path prefix mapping between a sub-project's root and its location in the parent repository tree, used during subtree merge shift detection.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 9 previously no-op flags produce correct output matching git's behavior when tested against the same repository state
- **SC-002**: All 9 previously stub subcommands perform real work and produce output/effects matching git's behavior
- **SC-003**: Interactive `add -p` supports all prompt actions (y/n/q/a/d/s/e/?) with behavior matching git, including split and manual edit
- **SC-004**: Interactive rebase (`rebase -i`) supports all standard actions (pick, reword, edit, squash, fixup, drop, exec) with behavior matching git
- **SC-005**: Subtree merge correctly shifts sub-project trees into the mapped subdirectory without user-visible differences from git
- **SC-006**: Octopus merge succeeds for 3+ non-conflicting branches, producing a merge commit with N+1 parents
- **SC-007**: Sequencer abort (`cherry-pick --abort`, `revert --abort`) restores HEAD and index to pre-operation state in all cases
- **SC-008**: `pull --rebase` correctly rebases local commits on upstream rather than merging, with conflict handling matching git
- **SC-009**: All interactive patch modes (`add -p`, `reset -p`, `checkout -p`, `restore -p`, `stash -p`, `clean -i`) handle their respective operations with hunk-level granularity
- **SC-010**: End-to-end tests validate output parity between gitr and git for each of the 28 items addressed in this phase

## Assumptions

- Git 2.39+ is the reference version for parity comparison
- Interactive modes use `/dev/tty` for terminal input, matching git's approach
- The `rebase -i` editor-based workflow uses the standard editor resolution cascade: `$GIT_SEQUENCE_EDITOR` > `sequence.editor` config > `$GIT_EDITOR` > `core.editor` > `$VISUAL` > `$EDITOR` > `vi`
- `maintenance start/stop` on macOS uses `launchctl` with a plist in `~/Library/LaunchAgents/`; on Linux uses `crontab` or `systemd --user` timers; other platforms emit a clear error
- The existing `interactive.rs` module provides the foundation for all `-p` flag implementations; hunk splitting and display logic are already partially in place
- Subtree shift detection uses the heuristic of finding the best-matching subdirectory by comparing tree contents, matching git's approach
- Octopus merge aborts on any conflict (it does not attempt conflict resolution), matching git's documented behavior
- `log --follow` tracks a single file at a time (not multiple paths), matching git's limitation
- `diff --no-index` works outside of git repositories and does not require a `.git` directory

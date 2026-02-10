# Research: Git Parity Phase 4

**Feature**: 027-git-parity-phase4 | **Date**: 2026-02-09

## R-001: Log --follow Rename Tracking

**Decision**: Implement rename detection during rev-walk by comparing parent-to-child tree diffs for the tracked path, using the existing diff engine's rename detection (M-score threshold).

**Rationale**: Git's `--follow` works by detecting renames during traversal. When a commit's diff shows a rename from `old.txt` to `new.txt` and we're tracking `new.txt`, we switch to tracking `old.txt` for all older commits. This is a single-file-at-a-time operation (git limitation). The existing `git-diff` crate already has rename detection with configurable similarity threshold — we reuse it in the walk loop.

**Alternatives considered**:
- Pre-compute all renames before walking: Rejected — too expensive for large histories; git does it lazily during walk.
- Use a separate rename index: Rejected — adds complexity with no benefit; git doesn't do this.

## R-002: Symmetric Diff Flags (left-right, cherry-pick, cherry-mark)

**Decision**: Implement symmetric difference as a first-class walk mode in git-revwalk. Left-right marks each commit based on which side it's reachable from. Cherry-pick/cherry-mark use patch-id comparison to detect equivalent commits.

**Rationale**: Git's symmetric diff (`A...B`) finds commits reachable from either A or B but not both. `--left-right` annotates each with `<` (left-only) or `>` (right-only). `--cherry-pick` filters out commits with matching patch-ids on both sides. `--cherry-mark` keeps all commits but prefixes equivalent ones with `=` and unique ones with `+`. The existing `cherry.rs` module in git-revwalk already has patch-id computation — extend it with the marking/filtering logic.

**Alternatives considered**:
- Implement at CLI level by post-filtering: Rejected — too slow for large repos; must be integrated into walk.
- Use commit message matching instead of patch-id: Rejected — git uses patch-id for correctness.

## R-003: Ancestry Path Filtering

**Decision**: Implement as a walk filter that keeps only commits on the direct ancestry chain between the two range endpoints.

**Rationale**: `--ancestry-path A..B` restricts output to commits that are both descendants of A and ancestors of B. This filters out side branches that merge into the path. Implementation: during walk, check if each commit is an ancestor of B and a descendant of A using the existing graph traversal infrastructure.

**Alternatives considered**:
- Pre-compute ancestry set: This is actually the approach — compute the set of commits on the ancestry path first, then filter during output.

## R-004: Log --source Ref Tracking

**Decision**: Track the originating ref for each commit during traversal by recording which ref tip initially queued each commit.

**Rationale**: `git log --source --all` shows which ref each commit was first discovered through. Implementation: when initializing the walk queue from ref tips, tag each starting commit with its ref name. As the walk proceeds, propagate the source ref to parent commits (first-wins — if a commit is reachable from multiple refs, the first one to reach it wins).

**Alternatives considered**:
- Record all source refs per commit: Rejected — git only shows one source ref per commit (first discovery).

## R-005: Diff --no-index (Non-Repository Diff)

**Decision**: Implement as a special mode in the diff command that bypasses repository lookup entirely and directly compares two filesystem paths.

**Rationale**: `git diff --no-index` works outside of any git repository. It reads two paths (files or directories), computes a unified diff, and exits with code 0 (identical), 1 (different), or 2 (error). For directories, it recursively diffs all files. The existing git-diff engine can compute diffs between arbitrary byte sequences — the CLI just needs to read files directly instead of going through the object database.

**Alternatives considered**:
- Shell out to system diff: Rejected — must use gitr's own diff engine for consistency.

## R-006: Diff --check (Whitespace Errors)

**Decision**: Implement as a post-processing mode on diff output that scans for whitespace errors and reports them with line numbers.

**Rationale**: `git diff --check` reports: trailing whitespace, mixed tabs/spaces (configurable via `core.whitespace`), and blank lines at EOF. It outputs `filename:line: trailing whitespace.` style messages and exits with code 2 if any errors found, 0 otherwise. Implementation reads the diff hunks and inspects each added line for violations.

**Alternatives considered**:
- Implement as a separate lint pass on files: Rejected — git specifically checks the diff (only reports errors in changed lines).

## R-007: Pull --rebase Integration

**Decision**: After fetch, invoke the rebase command logic instead of merge when `--rebase` is specified.

**Rationale**: `git pull --rebase` is equivalent to `git fetch` followed by `git rebase` onto the upstream branch. The existing pull.rs already does fetch + merge; for --rebase, replace the merge call with a rebase call. The rebase infrastructure exists (non-interactive rebase is implemented). Need to handle: determining the correct upstream ref, passing through autostash/strategy options.

**Alternatives considered**:
- Implement as a separate command: Rejected — it's a pull option, not a new command.

## R-008: Remote Subcommand Implementation

**Decision**: Implement all 5 stub remote subcommands using existing git-config and git-ref APIs.

**Rationale**:
- `set-head`: Create/delete symbolic ref at `refs/remotes/<remote>/HEAD` → uses git-ref's symbolic ref API.
- `prune`: Compare local tracking refs against remote refs (from last fetch), remove stale ones → uses git-ref listing + delete.
- `update`: Iterate configured remotes and call fetch for each → reuses fetch command logic.
- `set-branches`: Modify `remote.<name>.fetch` config entries → uses git-config API.
- `get-url`: Read `remote.<name>.url` (or `.pushurl`) from config → simple config lookup.

**Alternatives considered**:
- Shell out to git for remote operations: Rejected — defeats purpose of native implementation.

## R-009: Reflog Expire and Delete

**Decision**: Implement in git-ref crate by reading/rewriting reflog files with entry filtering.

**Rationale**: Reflog files are append-only logs at `.git/logs/refs/...`. `expire` removes entries older than a threshold by rewriting the file. `delete` removes a specific entry by index. Both operations require: reading all entries, filtering, writing back. The existing git-ref crate can read reflogs — add write-back capability.

**Alternatives considered**:
- In-place editing: Rejected — risky for data integrity; rewrite-and-rename is safer (matching git's approach).

## R-010: Maintenance Start/Stop

**Decision**: Platform-specific scheduler registration: launchd plist on macOS, crontab on Linux.

**Rationale**: `git maintenance start` registers a periodic task. On macOS: creates `~/Library/LaunchAgents/org.git-scm.git.maintenance.plist` and loads it via `launchctl`. On Linux: adds a crontab entry for `gitr maintenance run --auto`. `stop` reverses: unloads/removes the registration.

**Alternatives considered**:
- systemd timers on Linux: Could support both cron and systemd, but cron is more universally available. Start with cron, add systemd as enhancement.
- Cross-platform scheduler library: Rejected — unnecessary dependency for simple shell-out to launchctl/crontab.

## R-011: Subtree Merge Strategy

**Decision**: Implement shift detection by finding the best-matching subdirectory in the base tree, then adjusting all paths during the ORT merge.

**Rationale**: Git's subtree strategy works by: (1) finding which subdirectory in the "ours" tree best matches the root of "theirs" tree, (2) shifting all paths in "theirs" to be under that subdirectory, (3) running a normal three-way merge with the shifted paths. The heuristic compares tree entries to find the highest-overlap subdirectory. `--subtree=<prefix>` allows explicitly specifying the prefix.

**Alternatives considered**:
- Require explicit prefix always: Rejected — git auto-detects it; we should match behavior.

## R-012: Octopus Merge Strategy

**Decision**: Implement iterative merge: merge heads one at a time into the result, aborting on any conflict.

**Rationale**: Git's octopus strategy merges N heads by: (1) starting with the current HEAD as the base, (2) merging each additional head sequentially, (3) if any merge produces conflicts, abort the entire operation. The result is a single merge commit with N+1 parents. Uses the read-tree three-way merge for each step (we can use ORT).

**Alternatives considered**:
- Parallel merge of all heads simultaneously: Rejected — git does it sequentially; matches their behavior.

## R-013: Sequencer Abort State Restoration

**Decision**: Fix the abort() method to: (1) read saved original HEAD from `.git/sequencer/head`, (2) reset HEAD to that commit, (3) restore the index from that commit's tree, (4) clean up sequencer state.

**Rationale**: The current abort() only calls cleanup() (removes sequencer directory) but doesn't restore HEAD. This is a correctness bug. Git's cherry-pick/revert --abort restores HEAD to the value stored when the operation began. The original_head is already saved — just need to use it.

**Alternatives considered**:
- Use ORIG_HEAD instead of sequencer/head: Could use either, but sequencer/head is more reliable since it's specifically saved for abort.

## R-014: Interactive Patch Modes

**Decision**: Extend the existing `interactive.rs` module to: (1) complete the `e` (edit) action using the editor resolution cascade, (2) integrate with add/reset/checkout/restore/stash commands via the existing `select_hunks()` API.

**Rationale**: The interactive.rs module already handles: hunk display, y/n/q/a/d/s prompts, hunk splitting, and hunk application. Missing: manual edit (opens editor with hunk in unified diff format, validates edits, applies result). Each command integration follows the pattern: compute diff between appropriate trees, present hunks, apply selected hunks to the appropriate target (index for add, worktree for checkout, etc.).

**Alternatives considered**:
- Separate interactive module per command: Rejected — the existing shared module with mode parameter is cleaner.

## R-015: Interactive Rebase

**Decision**: Implement as a new module in git-cli using the existing sequencer infrastructure, with editor-based todo list editing.

**Rationale**: Interactive rebase: (1) generates a todo list of commits, (2) opens editor for user to modify actions, (3) replays commits according to the todo list. Actions: pick (apply as-is), reword (apply, edit message), edit (apply, pause), squash (combine with previous, edit combined message), fixup (combine with previous, keep previous message), drop (skip), exec (run command). The existing sequencer already supports todo lists and action types — extend it for the full rebase workflow. Editor resolution: `$GIT_SEQUENCE_EDITOR` > `sequence.editor` config > `$GIT_EDITOR` > `core.editor` > `$VISUAL` > `$EDITOR` > `vi`.

**Alternatives considered**:
- Implement without sequencer: Rejected — sequencer provides abort/continue/skip infrastructure that interactive rebase needs.

## R-016: Clean -i Interactive Mode

**Decision**: Implement as a separate interactive menu in the clean command, distinct from the hunk-based interactive.rs.

**Rationale**: `git clean -i` uses a numbered menu (not hunk selection). It lists untracked files with numbers, offers commands: clean (remove selected), filter by pattern, select by number ranges, quit. This is fundamentally different from the patch-mode interactive — it's a file selection interface, not a hunk selection interface. Implement directly in clean.rs.

**Alternatives considered**:
- Reuse interactive.rs: Rejected — different interaction model (file list vs hunk selection).

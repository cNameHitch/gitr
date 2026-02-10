# Tasks: Git Parity Phase 4 — Remaining Behavioral Gaps

**Input**: Design documents from `/specs/027-git-parity-phase4/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, quickstart.md

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Verify baseline and branch readiness

- [X] T001 Verify workspace builds cleanly (`cargo build --workspace && cargo test --workspace && cargo clippy --workspace`) and confirm branch `027-git-parity-phase4` is active

---

## Phase 2: US3 — Core Engine Completion (Priority: P1) 🎯 MVP

**Goal**: Fix 3 correctness bugs in existing merge/sequencer infrastructure — sequencer abort state restoration, octopus merge for 3+ heads, subtree merge shift detection. These are correctness bugs in existing functionality that can cause data loss or silent corruption.

**Independent Test**: Run `gitr cherry-pick A B C` where commit B conflicts, then `gitr cherry-pick --abort` and verify HEAD is restored; run `gitr merge A B C` (octopus) and verify 4-parent merge commit; run `gitr merge -s subtree sub-project-branch` and verify tree is shifted into subdirectory.

**Why first**: Sequencer abort fix is required by US5 (interactive rebase uses sequencer for abort/continue/skip). Octopus and subtree are independent but grouped here as core engine fixes.

### Implementation for US3

- [X] T002 [US3] Fix `abort()` method to restore HEAD to `self.original_head`, restore the index from that commit's tree, then call `cleanup()` — in `crates/git-merge/src/sequencer.rs`. Currently abort() only calls cleanup() without restoring HEAD. Must: (1) read original_head, (2) update HEAD ref to original_head, (3) read-tree original_head to restore index, (4) cleanup sequencer state. Also handle edge case: error "no cherry-pick/revert in progress" when no sequencer state exists. (FR-042)
- [X] T003 [P] [US3] Implement octopus merge strategy for 3+ heads in `crates/git-merge/src/strategy/octopus.rs`. Currently delegates to ORT for 2-head case and errors on 3+. Must: (1) start with current HEAD tree as base, (2) for each additional head, run ORT merge against accumulated result, (3) if any step produces a conflict, abort entire operation (octopus never resolves conflicts, matching git), (4) return merge result with N+1 parents. The merge commit creation happens in the CLI layer. (FR-041)
- [X] T004 [P] [US3] Implement subtree merge strategy with shift detection in `crates/git-merge/src/strategy/subtree.rs`. Currently delegates to ORT ignoring subtree shift. Must: (1) if `--subtree=<prefix>` option is set, use that prefix directly, (2) otherwise auto-detect by iterating subdirectories in "ours" tree, comparing each against root of "theirs" tree, scoring by matching entry count, selecting highest-scoring subdirectory above 0.5 threshold, (3) shift all paths in "theirs" tree under the detected prefix, (4) delegate to ORT with shifted paths. (FR-040)
- [X] T005 [US3] Verify cherry-pick --abort and revert --abort CLI handlers correctly invoke the fixed sequencer abort in `crates/git-cli/src/commands/cherry_pick.rs` and `crates/git-cli/src/commands/revert.rs`. Both already have abort handlers — verify they call sequencer.abort() (not just cleanup) and that ORIG_HEAD is also properly restored. Fix if needed. (FR-042)

**Checkpoint**: Core engine bugs fixed. Sequencer abort restores HEAD/index, octopus handles 3+ branches, subtree detects and shifts subdirectories.

---

## Phase 3: US1 — No-Op Flag Completion (Priority: P1)

**Goal**: Make 9 currently no-op flags produce correct output matching git — log --follow, --left-right, --cherry-pick, --cherry-mark, --ancestry-path, --source; diff --no-index, --check; pull --rebase. These flags are currently accepted but silently do nothing, producing incorrect/misleading output.

**Independent Test**: Run `gitr log --follow -- renamed-file.txt` in a repo where the file was renamed and verify history crosses the rename boundary; run `gitr diff --no-index a.txt b.txt` outside a repo and verify unified diff output; run `gitr pull --rebase` and verify local commits are rebased (not merged).

### Implementation for US1

#### RevWalk Library (crates/git-revwalk/)

- [X] T006 [US1] Add walk option fields to the RevWalk/WalkOptions in `crates/git-revwalk/src/lib.rs` — add `left_right: bool`, `cherry_pick: bool`, `cherry_mark: bool`, `ancestry_path: bool`, `source: bool`, `follow_path: Option<BString>`. Also add `SymmetricDiffCommit` struct (commit_id, side: Left/Right, patch_id, is_equivalent, source_ref) and `DiffSide` enum to lib.rs exports. These types are consumed by the walk engine and the CLI formatter. (FR-001–FR-006)
- [X] T007 [US1] Implement `--follow` rename tracking during rev-walk in `crates/git-revwalk/src/walk.rs`. When `follow_path` is set: (1) for each commit, compute diff against parent using git-diff's rename detection, (2) if a rename is detected from old_path→tracked_path, switch `follow_path` to old_path for subsequent (older) commits, (3) only include commits that touch the currently tracked path, (4) error if multiple paths are given with --follow ("--follow requires exactly one path"). Single-file-at-a-time limitation matches git. (FR-001)
- [X] T008 [US1] Implement symmetric diff walk mode with left-right side tagging in `crates/git-revwalk/src/walk.rs`. When `left_right` is true and the range is `A...B` (symmetric): (1) compute commits reachable from A but not B → tag as Left, (2) compute commits reachable from B but not A → tag as Right, (3) emit all in topological order with side annotation. Store the side in the SymmetricDiffCommit result. The `<` and `>` prefix rendering happens in the format layer. (FR-002)
- [X] T009 [US1] Implement cherry equivalence detection in `crates/git-revwalk/src/cherry.rs`. Extend existing cherry module: (1) for each commit in symmetric diff, compute its patch-id (hash of the diff content, ignoring whitespace and line numbers), (2) build a map of patch-ids for left-side and right-side commits, (3) mark commits whose patch-id appears on both sides as `is_equivalent = true`. When `cherry_pick` flag is set: filter out equivalent commits from output. When `cherry_mark` flag is set: keep all commits but annotate — prefix `=` for equivalent, `+` for unique. (FR-003, FR-004)
- [X] T010 [US1] Implement ancestry-path filtering in `crates/git-revwalk/src/walk.rs`. When `ancestry_path` is true and range `A..B` is given: (1) pre-compute the set of commits that are both ancestors of B and descendants of A (the direct ancestry chain), (2) during walk output, include only commits in this set (side branches merged into the path are excluded). Error if no range is given: "--ancestry-path requires a commit range". (FR-005)
- [X] T011 [US1] Implement source ref tracking in `crates/git-revwalk/src/walk.rs`. When `source` is true: (1) when initializing the walk queue from ref tips (e.g., --all), tag each starting commit with its ref name, (2) as the walk proceeds to parent commits, propagate the source ref (first-wins: if a commit is reachable from multiple refs, the first to reach it wins), (3) store source_ref in the walk output for each commit. (FR-006)
- [X] T012 [US1] Add left-right prefix, cherry-mark prefix, and source annotation rendering in `crates/git-revwalk/src/format.rs`. When formatting commit output: (1) if left_right, prepend `< ` or `> ` before commit hash, (2) if cherry_mark, prepend `= ` or `+ ` before commit hash, (3) if source, append `\t<ref-name>` after commit hash. Ensure these prefixes interact correctly with existing format options (--oneline, --format, --graph). (FR-002, FR-004, FR-006)

#### CLI Integration (crates/git-cli/)

- [X] T013 [US1] Wire all 6 log flags in `crates/git-cli/src/commands/log.rs`. Remove the "warning: --<flag> is accepted but not yet implemented" messages for follow, left_right, cherry_pick, cherry_mark, ancestry_path, and source. Instead: (1) pass each flag through to WalkOptions when creating the rev-walk, (2) for --follow, pass the single pathspec as follow_path and validate only one path is given, (3) for --left-right/--cherry-pick/--cherry-mark, verify the revision range is symmetric (A...B), (4) pass walk results through the format layer for prefix rendering. (FR-001–FR-006)
- [X] T014 [P] [US1] Implement `diff --no-index` for non-repository file comparison in `crates/git-cli/src/commands/diff.rs`. Remove the existing warning message. When `--no-index` is set: (1) do NOT require a git repository (skip repo discovery), (2) read two path arguments directly from filesystem, (3) if either path is a directory, recursively diff all files in both directories (matching git behavior), (4) compute unified diff using git-diff engine, (5) exit code 0 if identical, 1 if different, 2 on error, (6) support `--no-index` being auto-inferred when run outside a git repo (matching git). Handle edge case: `--no-index` with `-` reads from stdin. (FR-007)
- [X] T015 [P] [US1] Implement `diff --check` whitespace error detection in `crates/git-cli/src/commands/diff.rs`. Remove the existing warning message. When `--check` is set: (1) compute the diff normally, (2) scan each added line for whitespace errors: trailing whitespace (spaces/tabs before newline), space-before-tab in indent, blank lines at EOF, (3) read `core.whitespace` config for configurable checks, (4) output each error as `{file}:{line}: {description}.` format, (5) exit code 2 if any errors found, 0 otherwise. Do NOT output the normal diff — only error messages. (FR-008)
- [X] T016 [P] [US1] Implement `pull --rebase` in `crates/git-cli/src/commands/pull.rs`. Remove the "not yet fully implemented" warning. When `--rebase` is set: (1) run fetch as normal, (2) determine the upstream branch (from tracking config or arguments), (3) instead of calling merge, invoke the rebase logic to rebase local commits onto the fetched upstream, (4) pass through `--autostash`, `--strategy`, `--strategy-option` if provided, (5) handle conflict: rebase pauses, user resolves via `gitr rebase --continue`. Also respect `pull.rebase` config as default. (FR-009)

**Checkpoint**: All 9 previously no-op flags now produce correct output matching git's behavior.

---

## Phase 4: US2 — Stub Subcommand Implementation (Priority: P1)

**Goal**: Replace "not yet implemented" messages in 9 stub subcommands with real functionality — 5 remote subcommands, 2 reflog subcommands, 2 maintenance subcommands. These are immediate workflow blockers for users/scripts that depend on them.

**Independent Test**: Run `gitr remote get-url origin` and verify URL is printed; run `gitr remote prune origin` after deleting a remote branch and verify stale tracking ref is removed; run `gitr reflog expire --expire=90.days.ago --all` and verify old entries are removed.

### Implementation for US2

#### Remote subcommands (crates/git-cli/src/commands/remote.rs)

- [X] T017 [US2] Implement `remote get-url` in `crates/git-cli/src/commands/remote.rs`. Read `remote.<name>.url` from config (or `remote.<name>.pushurl` if `--push` is specified). If `--all` is specified, print all URLs (one per line). If `--push` is specified but no pushurl is configured, fall back to fetch URL (matching git). Error if remote doesn't exist. (FR-014)
- [X] T018 [US2] Implement `remote set-head` in `crates/git-cli/src/commands/remote.rs`. Create/update/delete the symbolic ref at `refs/remotes/<remote>/HEAD`. Three modes: (1) explicit: `set-head <remote> <branch>` → set symbolic ref to `refs/remotes/<remote>/<branch>`, (2) `--auto`: query the remote (via ls-remote or fetch) for its default branch and set accordingly, (3) `--delete`: remove the symbolic ref. Use git-ref's symbolic ref API. (FR-010)
- [X] T019 [US2] Implement `remote prune` in `crates/git-cli/src/commands/remote.rs`. Compare local tracking refs (`refs/remotes/<remote>/*`) against the remote's actual branches (from the last fetch, stored in packed-refs or loose refs from fetch info). Remove any local tracking ref whose corresponding remote branch no longer exists. With `--dry-run`: print what would be pruned without removing. Output format matches git: `* [pruned] origin/old-branch`. Silent success if no stale refs found. (FR-011)
- [X] T020 [US2] Implement `remote update` in `crates/git-cli/src/commands/remote.rs`. Fetch from all configured remotes (or a specified remote group from `remotes.<group>` config). Equivalent to `gitr fetch --all` when no group specified. With `--prune`: also prune stale tracking refs after each fetch. Iterate `remote.*` config sections to get list of all remotes. (FR-012)
- [X] T021 [US2] Implement `remote set-branches` in `crates/git-cli/src/commands/remote.rs`. Update the fetch refspec for the named remote. Default mode: replace `remote.<name>.fetch` config entries with refspecs for the specified branches only (e.g., `+refs/heads/main:refs/remotes/origin/main`). With `--add`: append new refspecs without removing existing ones. Use git-config API to write the updated refspecs. (FR-013)

#### Reflog library + CLI

- [X] T022 [P] [US2] Add `expire()` and `delete()` methods for reflog entries in `crates/git-ref/src/reflog.rs` (or appropriate module in git-ref crate). `expire(ref_name, expire_time, expire_unreachable_time, all_refs)`: read reflog file, filter out entries whose timestamp is older than threshold (keeping the most recent/tip entry always), write filtered entries back via atomic rewrite-and-rename. `delete(ref_entry)`: parse qualified entry like `HEAD@{3}`, read reflog, remove the entry at that index, rewrite file. Both methods must handle the reflog file format correctly and preserve entry ordering. (FR-015, FR-016)
- [X] T023 [US2] Wire `reflog expire` and `reflog delete` subcommands in `crates/git-cli/src/commands/reflog.rs`. For expire: parse `--expire=<time>`, `--expire-unreachable=<time>`, `--all` flags, resolve time expressions (e.g., "90.days.ago", "now"), call the git-ref expire() method. For delete: parse the ref entry argument (e.g., `HEAD@{3}`), call the git-ref delete() method. Handle edge cases: `--expire=now` removes all entries except current tip; error if reflog doesn't exist. (FR-015, FR-016)

#### Maintenance subcommands

- [X] T024 [P] [US2] Implement `maintenance start` and `maintenance stop` in `crates/git-cli/src/commands/maintenance.rs`. For `start`: (1) detect platform (macOS vs Linux), (2) on macOS: generate launchd plist XML at `~/Library/LaunchAgents/org.git-scm.git.maintenance.plist` with hourly/daily/weekly schedules running `gitr maintenance run --auto`, load via `launchctl load`, (3) on Linux: add crontab entries for hourly/daily/weekly schedules via `crontab -l | crontab -`, (4) register repo in `~/.config/git/maintenance.repos`. For `stop`: (1) on macOS: `launchctl unload` and remove plist, (2) on Linux: remove crontab entries, (3) unregister repo from maintenance.repos. Error on unsupported platforms. Idempotent: running start twice is safe. (FR-017, FR-018)

**Checkpoint**: All 9 previously stub subcommands now perform real work matching git's behavior.

---

## Phase 5: US4 — Interactive Patch Modes (Priority: P2)

**Goal**: Implement interactive hunk selection for `add -p`, `reset -p`, `checkout -p`, `restore -p`, `stash -p`, and interactive file selection for `clean -i`. Complete the missing manual hunk edit (`e`) action. Prompts, hunk display, and key bindings match git's behavior.

**Independent Test**: Modify a file with 3 separate hunks, run `gitr add -p`, stage only the first hunk (y/n/n), then run `gitr diff --cached` to verify only the first hunk is staged.

**Depends on**: None (interactive.rs foundation already exists)

### Implementation for US4

- [X] T025 [US4] Complete manual hunk edit (`e` action) in `crates/git-cli/src/interactive.rs`. Currently prints "not yet supported". Must: (1) resolve editor using cascade: `$GIT_EDITOR` > `core.editor` config > `$VISUAL` > `$EDITOR` > `vi`, (2) write the current hunk in unified diff format to a temp file (with instructions as comments matching git's format), (3) open editor via `std::process::Command`, (4) on save, parse the edited hunk back, validate it (correct @@ header, line counts match, only +/- lines changed), (5) if invalid, show error and re-prompt (matching git: "Your edited hunk does not apply"), (6) replace original hunk with edited version. (FR-022)
- [X] T026 [P] [US4] Integrate interactive hunk selection with `add -p` in `crates/git-cli/src/commands/add.rs`. When `-p`/`--patch` flag is set: (1) for each modified file, compute diff between worktree and index, (2) split diff into hunks, (3) call `InteractiveHunkSelector::select_hunks()` with prompt "Stage this hunk [y,n,q,a,d,s,e,?]?", (4) apply selected hunks to the index (using `apply_hunks_to_content()` to patch the index entry), (5) update the index file. Handle: no changes → "No changes.", file-level (new/deleted files) vs hunk-level selection. (FR-020, FR-021, FR-022)
- [X] T027 [P] [US4] Integrate interactive hunk selection with `reset -p` in `crates/git-cli/src/commands/reset.rs`. When `-p`/`--patch` flag is set: (1) compute diff between index and HEAD tree, (2) present each staged hunk with prompt "Unstage this hunk [y,n,q,a,d,s,e,?]?", (3) for selected hunks, reverse-apply them from the index (using `reverse_apply_hunks_to_content()`), (4) update the index file to reflect unstaged hunks. (FR-023)
- [X] T028 [P] [US4] Integrate interactive hunk selection with `checkout -p` in `crates/git-cli/src/commands/checkout.rs`. When `-p`/`--patch` flag is set: (1) compute diff between worktree and index (or HEAD if --no-index), (2) present each hunk with prompt "Discard this hunk from worktree [y,n,q,a,d,s,e,?]?", (3) for selected hunks, reverse-apply them to the worktree file (overwrite working copy with index/HEAD version for those hunks). (FR-024)
- [X] T029 [P] [US4] Integrate interactive hunk selection with `restore -p` in `crates/git-cli/src/commands/restore.rs`. When `-p`/`--patch` flag is set: (1) determine source (`--source=<tree>` or default to HEAD/index), (2) compute diff between worktree and source, (3) present each hunk with prompt "Apply this hunk to worktree [y,n,q,a,d,s,e,?]?", (4) for selected hunks, reverse-apply to worktree. Behavior should match checkout -p but with explicit source support. (FR-025)
- [X] T030 [P] [US4] Integrate interactive hunk selection with `stash -p` in `crates/git-cli/src/commands/stash.rs`. When `-p`/`--patch` flag is set: (1) compute diff between worktree and index, (2) present each hunk for selection, (3) create a stash entry containing only the selected hunks (apply selected hunks to create the stash tree), (4) reverse-apply the selected hunks from the worktree (remove stashed changes from working copy), (5) leave unselected hunks in the worktree unchanged. (FR-026)
- [X] T031 [P] [US4] Implement `clean -i` interactive file selection menu in `crates/git-cli/src/commands/clean.rs`. When `-i`/`--interactive` flag is set: (1) list all untracked files with sequential numbers, (2) display interactive menu matching git's format: "Would you like to [c]lean, [f]ilter by pattern, [s]elect by numbers, or [q]uit?", (3) `select by numbers`: accept number ranges (e.g., "1-3,5"), toggle selection on/off, (4) `filter by pattern`: accept glob pattern to select matching files, (5) `clean`: confirm "Remove these items? [y/n]" then delete selected files, (6) `quit`: exit without cleaning. Use `/dev/tty` for input like interactive.rs. (FR-027)

**Checkpoint**: All 6 interactive patch modes handle hunk-level granularity, and clean -i provides interactive file selection.

---

## Phase 6: US5 — Interactive Rebase (Priority: P2)

**Goal**: Implement `rebase -i` with full todo list editor workflow, all standard actions (pick, reword, edit, squash, fixup, drop, exec, break), autosquash, and conflict resolution. The todo list format, available actions, and flow match git's interactive rebase exactly.

**Independent Test**: Run `gitr rebase -i HEAD~3`, change the second commit's action from `pick` to `squash` in the editor, save and close, verify the resulting history has 2 commits with the squashed commit's changes combined.

**Depends on**: US3 (T002 — sequencer abort must work for `rebase --abort`)

### Implementation for US5

- [X] T032 [US5] Extend sequencer with interactive rebase types in `crates/git-merge/src/sequencer.rs`. Add `RebaseAction` enum variants: `Reword`, `Drop`, `Break`, `Label(String)`, `Reset(String)`, `Merge` (existing: Pick, Edit, Squash, Fixup, Exec). Add `RebaseTodoEntry` struct (action, commit_id, short_hash, subject, original_line). Add `RebaseState` struct (todo_entries, done_count, original_head, onto, head_name, current_fixup_message, autosquash). Ensure sequencer can operate in Rebase mode alongside existing CherryPick/Revert modes. (FR-031)
- [X] T033 [US5] Implement RebaseState persistence — read/write `.git/rebase-merge/` state files in `crates/git-cli/src/commands/rebase.rs`. Files: `interactive` (marker), `todo` (remaining entries), `done` (completed entries), `head-name`, `onto`, `orig-head`, `message` (squash/fixup accumulated message), `author-script`, `amend` (marker). Implement `save_state()` and `load_state()` functions. Format matches git: each todo line is `<action> <short-hash> <subject>`. (FR-030)
- [X] T034 [US5] Implement todo list generation from commit range in `crates/git-cli/src/commands/rebase.rs`. Given `rebase -i <upstream>` (or `HEAD~N`): (1) walk commits from HEAD to upstream (exclusive), (2) reverse to oldest-first order, (3) generate todo entries with `pick` action for each, (4) format as `pick <7-char-hash> <subject>` per line. Add comment footer with usage instructions matching git's format (list of available actions, blank line = pick, etc.). (FR-030)
- [X] T035 [US5] Implement editor integration for interactive rebase in `crates/git-cli/src/commands/rebase.rs`. Editor cascade: `$GIT_SEQUENCE_EDITOR` > `sequence.editor` config > `$GIT_EDITOR` > `core.editor` > `$VISUAL` > `$EDITOR` > `vi`. (1) Write todo list to `.git/rebase-merge/git-rebase-todo`, (2) open editor with that file, (3) parse edited file back into RebaseTodoEntry list, (4) validate: reject unknown actions, reject squash/fixup as first entry, empty list = abort rebase. Handle `--exec <cmd>`: insert `exec <cmd>` after each `pick` line before opening editor. (FR-030, FR-033)
- [X] T036 [US5] Implement todo list parser in `crates/git-cli/src/commands/rebase.rs`. Parse each non-comment, non-empty line from the edited todo file: (1) extract action keyword (pick/p, reword/r, edit/e, squash/s, fixup/f, drop/d, exec/x, break/b, label/l, reset/t, merge/m), (2) extract commit hash and subject for commit-based actions, (3) extract command string for exec, label string for label/reset, (4) skip comment lines (starting with `#`), (5) deleted lines = drop action, (6) return Vec<RebaseTodoEntry>. Handle abbreviated action names matching git. (FR-031)
- [X] T037 [US5] Implement action replay engine for pick, reword, and edit in `crates/git-cli/src/commands/rebase.rs`. Process todo entries sequentially: (1) `pick`: cherry-pick the commit onto current HEAD, advance to next entry, (2) `reword`: cherry-pick the commit, then open editor (message editor, not sequence editor) for the user to modify the commit message, amend the commit with new message, (3) `edit`: cherry-pick the commit, save state, print "Stopped at <hash>... You can amend the commit now" and exit — `rebase --continue` resumes from next entry. On conflict during any action: save state, set MERGE_MSG, print instructions, exit for user resolution. (FR-031, FR-034, FR-035)
- [X] T038 [US5] Implement action replay engine for squash, fixup, drop, exec, and break in `crates/git-cli/src/commands/rebase.rs`. (1) `squash`: cherry-pick commit, accumulate its message with previous commit's message, open editor with combined message, amend into previous commit, (2) `fixup`: same as squash but keep only previous commit's message (no editor), (3) `drop`: skip commit entirely (advance to next entry), (4) `exec`: run shell command via `sh -c "<cmd>"`, if command fails pause rebase for user to fix, (5) `break`: save state and exit for user interaction — `rebase --continue` resumes. Handle squash/fixup chains (multiple consecutive squash/fixup entries accumulate into one commit). (FR-031)
- [X] T039 [US5] Implement `--autosquash` support in `crates/git-cli/src/commands/rebase.rs`. When `--autosquash` is set (or `rebase.autosquash` config is true): (1) scan todo entries for commits whose subject starts with `fixup! <target-subject>` or `squash! <target-subject>`, (2) for each, find the target commit in the todo list by matching `<target-subject>` against other entries' subjects, (3) move the fixup/squash commit to immediately after its target, (4) change its action from `pick` to `fixup` or `squash` respectively, (5) apply reordering before opening editor so user can review. (FR-032)
- [X] T040 [US5] Wire interactive rebase into `--continue`, `--skip`, and `--abort` handlers in `crates/git-cli/src/commands/rebase.rs`. (1) `--continue`: load RebaseState from `.git/rebase-merge/`, if current entry was edit/conflict, finalize it, advance to next entry, resume replay loop, (2) `--skip`: discard current entry's changes, reset to pre-entry state, advance to next entry, resume replay, (3) `--abort`: call sequencer abort (restores HEAD to orig-head), remove `.git/rebase-merge/` directory, restore original branch ref. After all entries processed: update branch ref to final HEAD, remove `.git/rebase-merge/`, print "Successfully rebased and updated refs/heads/<branch>". (FR-035)

**Checkpoint**: Full interactive rebase with all standard actions, autosquash, conflict handling, and abort/continue/skip support.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: E2E validation and quality assurance across all user stories

- [X] T041 [P] E2E interop tests for core engine completion (US3) — test octopus merge (3+ heads, conflict abort), subtree merge (shift detection), sequencer abort (HEAD/index restoration) by comparing gitr vs git output on identical repositories
- [X] T042 [P] E2E interop tests for no-op flag completion (US1) — test all 9 flags (log --follow, --left-right, --cherry-pick, --cherry-mark, --ancestry-path, --source; diff --no-index, --check; pull --rebase) by comparing gitr vs git output
- [X] T043 [P] E2E interop tests for stub subcommands (US2) — test all 9 subcommands (remote get-url/set-head/prune/update/set-branches; reflog expire/delete; maintenance start/stop) by comparing gitr vs git behavior
- [X] T044 Run `cargo clippy --workspace -- -D warnings` and fix any new warnings, then run `cargo test --workspace` to verify zero regressions across all crates

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **US3 (Phase 2)**: Depends on Setup — fixes foundation (sequencer, octopus, subtree)
- **US1 (Phase 3)**: Depends on Setup — can start in parallel with US2/US3 (except T016 pull --rebase benefits from working rebase)
- **US2 (Phase 4)**: Depends on Setup — can start in parallel with US1/US3
- **US4 (Phase 5)**: Depends on Setup — independent of all P1 stories
- **US5 (Phase 6)**: Depends on US3 (T002 sequencer abort) — interactive rebase needs sequencer abort/continue/skip
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **US3 (P1)**: Foundation — no dependencies on other stories. BLOCKS US5.
- **US1 (P1)**: Independent. T016 (pull --rebase) benefits from working rebase but can use existing non-interactive rebase.
- **US2 (P1)**: Fully independent — can run in parallel with US1 and US3.
- **US4 (P2)**: Fully independent — can start any time after Setup.
- **US5 (P2)**: Depends on US3 (T002 specifically). Cannot start until sequencer abort is fixed.

### Within Each User Story

- Library tasks before CLI wiring tasks
- RevWalk struct definitions (T006) before walk implementations (T007–T011)
- Walk implementations before CLI integration (T013)
- interactive.rs edit action (T025) before command integrations (T026–T031)
- Sequencer types (T032) before state persistence (T033) before replay engine (T037–T038)

### Parallel Opportunities

**Cross-story parallelism** (after Setup):
- US1 (revwalk work), US2 (remote/reflog/maintenance), and US3 (merge strategies) can all proceed in parallel
- US4 (interactive modes) can proceed in parallel with all P1 stories

**Within US1**:
- T014 (diff --no-index), T015 (diff --check), T016 (pull --rebase) are all [P] — different files from revwalk work

**Within US2**:
- T022 (reflog library) and T024 (maintenance) are [P] — can run in parallel with remote subcommand tasks

**Within US4**:
- T026–T031 are all [P] — each command integration is a different file, all depend only on T025

**Within Phase 7**:
- T041, T042, T043 are all [P] — independent test suites

---

## Parallel Example: US4 (Interactive Patch Modes)

```
# After T025 (interactive.rs edit action) is complete, launch all command integrations in parallel:
Task: "T026 [P] [US4] Integrate with add -p in add.rs"
Task: "T027 [P] [US4] Integrate with reset -p in reset.rs"
Task: "T028 [P] [US4] Integrate with checkout -p in checkout.rs"
Task: "T029 [P] [US4] Integrate with restore -p in restore.rs"
Task: "T030 [P] [US4] Integrate with stash -p in stash.rs"
Task: "T031 [P] [US4] Implement clean -i in clean.rs"
```

---

## Implementation Strategy

### MVP First (US3 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: US3 — Core Engine Completion
3. **STOP and VALIDATE**: Sequencer abort restores HEAD, octopus merges 3+ branches, subtree shifts correctly
4. These are the most critical correctness fixes

### Incremental Delivery

1. Setup → Foundation ready
2. US3 (core engines) → Correctness bugs fixed (MVP)
3. US1 (no-op flags) → Silent divergence eliminated
4. US2 (stub subcommands) → Workflow blockers removed
5. US4 (interactive patch) → Power-user features enabled
6. US5 (interactive rebase) → Full git parity achieved
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup together
2. Once Setup is done:
   - Developer A: US3 (core engines) — highest priority, blocks US5
   - Developer B: US1 (revwalk flags + diff flags)
   - Developer C: US2 (remote + reflog + maintenance)
   - Developer D: US4 (interactive modes) — independent
3. After US3 completes:
   - Developer A moves to US5 (interactive rebase)
4. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies on other tasks in same phase
- [Story] label maps task to specific user story for traceability
- All exit codes must match git exactly (diff --check = 2, diff --no-index = 0/1, etc.)
- All interactive modes use `/dev/tty` for terminal input, matching git's approach
- Editor cascade for rebase -i: `$GIT_SEQUENCE_EDITOR` > `sequence.editor` > `$GIT_EDITOR` > `core.editor` > `$VISUAL` > `$EDITOR` > `vi`
- Reflog rewrite uses atomic rename for data safety (matching git's approach)
- Maintenance start/stop is platform-specific: launchd on macOS, cron on Linux
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently

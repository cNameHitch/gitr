# Tasks: Git Behavioral Parity — Phase 2

**Input**: Design documents from `/specs/025-git-parity-phase2/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: E2E comparison tests are included as the spec explicitly requires interop testing (SC-001, SC-002).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify build, branch readiness, and no pre-existing failures

- [X] T001 Verify clean build with `cargo build --workspace` and `cargo test --workspace` on branch `025-git-parity-phase2`
- [X] T002 Verify `cargo clippy --workspace -- -D warnings` passes with no new warnings

**Checkpoint**: Branch is clean, all existing tests pass, ready for implementation

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Library-level changes in non-CLI crates that multiple user stories depend on

**⚠️ CRITICAL**: These library changes must be complete before CLI-level tasks can compile and work

- [X] T003 [US1] Add `Custom(String)` variant to `DateFormat` enum and implement `format()` for it in `crates/git-utils/src/date.rs` (FR-007 dependency)
- [X] T004 [P] [US1] Add `unset(key, scope)` method to `GitConfig` in `crates/git-config/src/lib.rs` — remove entry from in-memory config and write updated file; return Ok(()) if key doesn't exist (FR-005 dependency)
- [X] T005 [P] [US1] Add `get_string_from_scope(key, scope)` method to `GitConfig` in `crates/git-config/src/lib.rs` if not present, and ensure `set()` supports `ConfigScope::Global` with file creation (FR-006 dependency)
- [X] T006 [P] [US1] Add `WordDiff` variant to `DiffOutputFormat` in `crates/git-diff/src/lib.rs` and create `crates/git-diff/src/format/word_diff.rs` with `format_word_diff(result: &DiffResult) -> String` using `[-removed-]{+added+}` markers (FR-011 dependency)
- [X] T007 [P] [US2] Fix `GraphDrawer::draw_commit()` in `crates/git-revwalk/src/graph.rs` to not emit extra `|` lines between commits on linear history (FR-018)
- [X] T008 [P] [US4] Add `Merge:` line (abbreviated parent hashes, 7 chars) after `commit <oid>` line in `Fuller` format in `crates/git-revwalk/src/pretty.rs` for merge commits (FR-031)
- [X] T009 [US3] Ensure `GitDate::parse_raw()` in `crates/git-utils/src/date.rs` falls back to `GitDate::parse()` for ISO 8601 strings so `GIT_AUTHOR_DATE` / `GIT_COMMITTER_DATE` env vars work (FR-014)

**Checkpoint**: Foundation ready — all library crate changes compiled and unit-tested. CLI story implementation can begin.

---

## Phase 3: User Story 3 — Date Parsing & Reflog (Priority: P1) 🎯 MVP

**Goal**: ISO 8601 date env vars work for commits; reflog entries are recorded for all HEAD-modifying operations

**Independent Test**: Create commits with ISO dates in env vars → verify correct timestamps. Run operations → verify `gitr reflog` shows entries.

### Implementation for User Story 3

- [X] T010 [US3] Wire ISO 8601 date parsing into `get_signature()` in `crates/git-cli/src/commands/commit.rs` — verify `GitDate::parse_raw()` fallback works for env var dates (FR-014)
- [X] T011 [US3] Wire `append_reflog_entry()` from `crates/git-ref/src/reflog.rs` into `commit` operation in `crates/git-cli/src/commands/commit.rs` with message format `commit: <subject>` or `commit (initial): <subject>` (FR-015)
- [X] T012 [US3] Wire `append_reflog_entry()` into `checkout` and `switch` operations in `crates/git-cli/src/commands/switch.rs` with message format `checkout: moving from <old> to <new>` (FR-015)
- [X] T013 [P] [US3] Wire `append_reflog_entry()` into `reset` operation in `crates/git-cli/src/commands/reset.rs` with message format `reset: moving to <ref>` (FR-015)
- [X] T014 [P] [US3] Wire `append_reflog_entry()` into `merge` operation in `crates/git-cli/src/commands/merge.rs` with message format `merge <branch>: Fast-forward` or `merge <branch>: Merge made by the 'ort' strategy.` (FR-015)
- [X] T015 [P] [US3] Wire `append_reflog_entry()` into `rebase` operation in `crates/git-cli/src/commands/rebase.rs` with message formats `rebase (start): checkout <upstream>`, `rebase: <subject>`, `rebase (finish): returning to <branch>` (FR-015)
- [X] T016 [P] [US3] Wire `append_reflog_entry()` into `cherry-pick` in `crates/git-cli/src/commands/cherry_pick.rs` with message format `cherry-pick: <subject>` (FR-015)
- [X] T017 [P] [US3] Wire `append_reflog_entry()` into `stash pop` in `crates/git-cli/src/commands/stash.rs` with message format `checkout: moving from <branch> to <branch>` (FR-015)

**Checkpoint**: US3 complete — ISO dates work in env vars, reflog records all HEAD-modifying operations

---

## Phase 4: User Story 1 — Missing CLI Flags & Subcommand Arguments (Priority: P1) 🎯 MVP

**Goal**: All 13 missing flags/features accept and behave correctly, eliminating "unexpected argument" errors

**Independent Test**: Run each previously-failing command and verify it completes successfully with correct behavior

### Implementation for User Story 1

- [X] T018 [P] [US1] Add `#[command(version = format!("gitr version {}", env!("CARGO_PKG_VERSION")))]` to `Cli` struct in `crates/git-cli/src/main.rs` (FR-001)
- [X] T019 [P] [US1] Add `--no-edit` flag to `MergeArgs` in `crates/git-cli/src/commands/merge.rs` — flag accepted, merge uses auto-generated message without editor (FR-002)
- [X] T020 [P] [US1] Add `--no-edit` flag to `RevertArgs` in `crates/git-cli/src/commands/revert.rs` — flag accepted, revert uses auto-generated message without editor (FR-003)
- [X] T021 [P] [US1] Add `short = 'c'` to existing `create` field in `SwitchArgs` in `crates/git-cli/src/commands/switch.rs` (FR-004) — SKIPPED: conflicts with global `-c` config flag on Cli struct
- [X] T022 [US1] Add `--unset` flag to `ConfigArgs` and wire to `GitConfig::unset()` in `crates/git-cli/src/commands/config.rs` (FR-005, depends on T004)
- [X] T023 [US1] Add `--global` flag to `ConfigArgs` and wire global scope reads/writes in `crates/git-cli/src/commands/config.rs` (FR-006, depends on T005)
- [X] T024 [US1] Add `--date` flag to `LogArgs` in `crates/git-cli/src/commands/log.rs` — parse format string (`iso`, `relative`, `short`, `default`, `format:<strftime>`) and pass `DateFormat` to `FormatOptions::date_format` (FR-007, depends on T003)
- [X] T025 [P] [US1] Add `--merges` flag to `LogArgs` in `crates/git-cli/src/commands/log.rs` — filter walk loop to skip commits with `parents.len() < 2` (FR-008)
- [X] T026 [P] [US1] Add `--no-merges` flag to `LogArgs` in `crates/git-cli/src/commands/log.rs` — filter walk loop to skip commits with `parents.len() > 1` (FR-009)
- [X] T027 [US1] Wire `-- <path>` filtering in log command in `crates/git-cli/src/commands/log.rs` — rename `_pathspecs` and filter commits by checking if each commit touches matching files (FR-010)
- [X] T028 [US1] Add `--word-diff` flag to `DiffArgs` in `crates/git-cli/src/commands/diff.rs` and wire to `format_word_diff()` output (FR-011, depends on T006)
- [X] T029 [P] [US1] Add `short = 's'` to existing `no_patch` field in `ShowArgs` in `crates/git-cli/src/commands/show.rs` (FR-012)
- [X] T030 [US1] Add `--contains` flag to `BranchArgs` in `crates/git-cli/src/commands/branch.rs` — filter branch listing using `is_ancestor()` to show only branches containing the specified commit (FR-013)

**Checkpoint**: US1 complete — all 13 missing flags work, no more "unexpected argument" errors

---

## Phase 5: User Story 2 — Output Format Parity (Priority: P2)

**Goal**: Command outputs match git v2.39.5 character-for-character for commit, merge, cherry-pick, stash, reset, rebase, gc, and status

**Independent Test**: Run each command in identical git and gitr repos, diff output, verify match

### Implementation for User Story 2

- [X] T031 [US2] Add diffstat summary line to `print_summary()` in `crates/git-cli/src/commands/commit.rs` — compute tree diff between parent and new commit using `diff_trees()` with `DiffOutputFormat::Stat` (FR-016)
- [X] T032 [US2] Add Date line and diffstat to `commit --amend` output in `crates/git-cli/src/commands/commit.rs` (FR-017)
- [X] T033 [US2] Add diffstat after merge strategy message in `crates/git-cli/src/commands/merge.rs` — use `diff_trees()` to compute stat between pre-merge and post-merge trees (FR-019)
- [X] T034 [P] [US2] Include current branch name in cherry-pick output format `[<branch> <short-hash>] <subject>` in `crates/git-cli/src/commands/cherry_pick.rs` (FR-020)
- [X] T035 [US2] Update stash pop output in `crates/git-cli/src/commands/stash.rs` — print full working tree status after pop and use 40-char hash in "Dropped" message (FR-021)
- [X] T036 [P] [US2] Add "Unstaged changes after reset:" message + file list for mixed reset in `crates/git-cli/src/commands/reset.rs` (FR-022)
- [X] T037 [P] [US2] Add `HEAD is now at <short-hash> <subject>` message for hard reset in `crates/git-cli/src/commands/reset.rs` (FR-023)
- [X] T038 [US2] Enable rename detection in `status --short` in `crates/git-cli/src/commands/status.rs` — set `detect_renames` in diff options, format as `R  old -> new` (FR-024)
- [X] T039 [US2] Add `Rebasing (N/M)` progress format and "Successfully rebased and updated refs/heads/<branch>." success message in `crates/git-cli/src/commands/rebase.rs` (FR-025)
- [X] T040 [P] [US2] Suppress default progress output in gc — make silent by default in `crates/git-cli/src/commands/gc.rs` (FR-026)
- [X] T041 [US2] Sort untracked files alphabetically and collapse directories in `find_untracked_recursive()` in `crates/git-cli/src/commands/status.rs` — if all files in a directory are untracked, show `subdir/` instead (FR-027)

**Checkpoint**: US2 complete — all 12 output format differences resolved

---

## Phase 6: User Story 4 — Minor Output Corrections (Priority: P3)

**Goal**: Fix remaining cosmetic output differences in describe, tag, show, log fuller, shortlog, init, and status hint

**Independent Test**: Run each command and compare output with git v2.39.5

### Implementation for User Story 4

- [X] T042 [P] [US4] Fix `describe` error message in `crates/git-cli/src/commands/describe.rs` — remove doubled `fatal:` prefix, match git's exact error wording (FR-028)
- [X] T043 [P] [US4] Show commit subject for lightweight tags in `tag -n` in `crates/git-cli/src/commands/tag.rs` (FR-029)
- [X] T044 [P] [US4] Remove 4-space indent from annotated tag message body in `show` in `crates/git-cli/src/commands/show.rs` (FR-030)
- [X] T045 [P] [US4] Read from stdin in `shortlog` when non-tty (use `std::io::stdin().is_terminal()`) in `crates/git-cli/src/commands/shortlog.rs` (FR-032)
- [X] T046 [P] [US4] Resolve symlinks in `init` output path (use `std::fs::canonicalize()`) and omit `.git` suffix in `crates/git-cli/src/commands/init.rs` (FR-033)
- [X] T047 [P] [US4] Use `git rm --cached <file>` unstage hint for initial commits (no HEAD ref) in `crates/git-cli/src/commands/status.rs` instead of `git restore --staged` (FR-034)

**Checkpoint**: US4 complete — all 7 minor output corrections applied

---

## Phase 7: Testing & Polish

**Purpose**: E2E comparison tests for all 34 FRs, regression verification, cross-cutting quality checks

### E2E Comparison Tests

- [X] T048 Create `crates/git-cli/tests/parity_phase2_tests.rs` with test infrastructure — helper functions for creating temp repos, running `git` and `gitr`, comparing output
- [X] T049 [US1] Add E2E tests for FR-001 through FR-013 (missing flags) in `parity_phase2_tests.rs` — each test creates isolated repo, runs command with both git and gitr, asserts identical output
- [X] T050 [US3] Add E2E tests for FR-014 and FR-015 (ISO dates, reflog) in `parity_phase2_tests.rs`
- [X] T051 [US2] Add E2E tests for FR-016 through FR-027 (output format) in `parity_phase2_tests.rs`
- [X] T052 [US4] Add E2E tests for FR-028 through FR-034 (minor corrections) in `parity_phase2_tests.rs`

### Regression & Quality

- [X] T053 Run full regression suite `cargo test --workspace` to verify no existing tests broken (SC-005)
- [X] T054 Run `cargo clippy --workspace -- -D warnings` and fix any new warnings
- [X] T055 Run quickstart.md validation — verify all commands in quickstart.md work correctly

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **US3 (Phase 3)**: Depends on Phase 2 (T009 for date parsing). Can run in parallel with US1.
- **US1 (Phase 4)**: Depends on Phase 2 (T003, T004, T005, T006 for library changes)
- **US2 (Phase 5)**: Depends on Phase 2 (T007 for graph fix). Can start after foundational.
- **US4 (Phase 6)**: Depends on Phase 2 (T008 for fuller format). Can start after foundational.
- **Testing (Phase 7)**: Depends on all user story phases being complete

### Task Dependencies (within phases)

- T022 (config --unset CLI) depends on T004 (GitConfig::unset method)
- T023 (config --global CLI) depends on T005 (GitConfig global scope)
- T024 (log --date CLI) depends on T003 (DateFormat::Custom variant)
- T028 (diff --word-diff CLI) depends on T006 (word_diff formatter)
- T010 (ISO date wiring) depends on T009 (parse_raw fallback)

### Parallel Opportunities

Within **Phase 2** (Foundational):
- T004, T005, T006, T007, T008 can all run in parallel (different crates/files)
- T003 and T009 are sequential (same file, T009 depends on T003)

Within **Phase 3** (US3):
- T013, T014, T015, T016, T017 can all run in parallel (different command files)
- T011 and T012 are sequential with T010 (same dependency chain)

Within **Phase 4** (US1):
- T018, T019, T020, T021, T025, T026, T029 can all run in parallel (independent files, no library deps)
- T022, T023, T024, T028, T030 depend on foundational tasks

Within **Phase 5** (US2):
- T034, T036, T037, T040 can all run in parallel (different files)
- T031, T032 are sequential (same file: commit.rs)
- T038, T041 touch same file (status.rs) — sequential

Within **Phase 6** (US4):
- T042, T043, T044, T045, T046, T047 can ALL run in parallel (different files)

Within **Phase 7** (Testing):
- T049, T050, T051, T052 are sequential (same test file)

---

## Implementation Strategy

### MVP First (US3 + US1 — Both P1)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational library changes
3. Complete Phase 3: US3 (Date Parsing & Reflog) — critical infrastructure
4. Complete Phase 4: US1 (Missing Flags) — eliminates hard errors
5. **STOP and VALIDATE**: All P1 requirements met, gitr doesn't error on common flags, reflog works

### Incremental Delivery

1. Setup + Foundational → Library ready
2. US3 (Date/Reflog) → Test independently → Reflog works, ISO dates work
3. US1 (Missing Flags) → Test independently → No more "unexpected argument" errors
4. US2 (Output Format) → Test independently → Output matches git
5. US4 (Minor Corrections) → Test independently → Cosmetic parity achieved
6. Testing Phase → Full E2E validation against git v2.39.5

---

## Summary

| Metric | Count |
|--------|-------|
| Total tasks | 55 |
| Phase 1 (Setup) | 2 |
| Phase 2 (Foundational) | 7 |
| Phase 3 (US3 - P1) | 8 |
| Phase 4 (US1 - P1) | 13 |
| Phase 5 (US2 - P2) | 11 |
| Phase 6 (US4 - P3) | 6 |
| Phase 7 (Testing) | 8 |
| Parallelizable tasks | 32 |
| New files | 2 (word_diff.rs, parity_phase2_tests.rs) |
| Modified crates | 6 |
| Functional requirements covered | 34 / 34 |

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Output must match git v2.39.5 character-for-character (excluding hashes/timestamps)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- FR-035 (`add -p`) is explicitly deferred and not included

# Tasks: History & Inspection Commands

**Input**: Design documents from `specs/017-history-and-inspection-commands/`
**Prerequisites**: All library crates, plumbing commands

## Phase 1: User Story 1 - Log (Priority: P1)

- [X] T001 [US1] Implement `log` in `src/commands/log.rs` — --oneline, --format, --graph, --stat, -p, --author, --since/--until, pathspec
- [X] T002 [US1] Implement `rev-list` in `src/commands/rev_list.rs` — commit listing with filtering
- [X] T003 [US1] Add log integration tests

**Checkpoint**: git log works with all format options

---

## Phase 2: User Story 2 & 3 - Show and Diff (Priority: P1)

- [X] T004 [P] [US2] Implement `show` in `src/commands/show.rs` — commits, tags, trees, blobs, tree:path syntax
- [X] T005 [P] [US3] Implement `diff` command in `src/commands/diff.rs` — unstaged, --cached, HEAD, commit..commit, --stat
- [X] T006 Add show and diff integration tests

**Checkpoint**: Show and diff match C git output

---

## Phase 3: User Story 4 - Blame (Priority: P2)

- [X] T007 [US4] Implement blame algorithm (incremental line attribution)
- [X] T008 [US4] Implement `blame` command in `src/commands/blame.rs` — -L, -C, -w, --porcelain
- [X] T009 [US4] Add blame integration tests

**Checkpoint**: Blame output matches C git

---

## Phase 4: User Story 5 - Bisect (Priority: P2)

- [X] T010 [US5] Implement bisect state management in `src/commands/bisect.rs`
- [X] T011 [US5] Implement bisect algorithm (binary search through commits)
- [X] T012 [US5] Implement `bisect run` for automated bisection
- [X] T013 [US5] Add bisect integration tests

**Checkpoint**: Bisect correctly identifies first bad commit

---

## Phase 5: User Story 6 - Other Commands (Priority: P2)

- [X] T014 [P] [US6] Implement `shortlog` in `src/commands/shortlog.rs`
- [X] T015 [P] [US6] Implement `describe` in `src/commands/describe.rs`
- [X] T016 [P] [US6] Implement `grep` in `src/commands/grep.rs`
- [X] T017 [P] [US6] Implement `cherry-pick` command in `src/commands/cherry_pick.rs`
- [X] T018 [P] [US6] Implement `revert` command in `src/commands/revert.rs`
- [X] T019 [P] [US6] Implement `reflog` command in `src/commands/reflog.rs`
- [X] T020 [US6] Add integration tests for each command

**Checkpoint**: All secondary history commands work

---

## Phase 6: Format-Patch and AM (Priority: P2)

- [X] T021 [US6] Implement `format-patch` in `src/commands/format_patch.rs` — email format, cover letter, numbering
- [X] T022 [US6] Implement `am` in `src/commands/am.rs` — parse mbox, apply patches, handle conflicts
- [X] T023 [US6] Add patch round-trip tests (format-patch → am → verify identical)

**Checkpoint**: Patch workflow works end-to-end

---

## Phase 7: Polish

- [X] T024 [P] Run `cargo clippy` and fix warnings
- [X] T025 Run full integration test suite

---

## Dependencies & Execution Order

- Phase 1 first (log is the most important)
- Phase 2 can start in parallel with Phase 1
- Phases 3-6 depend on Phase 1 (share rev-walk infrastructure)
- Within Phase 5, all [P] tasks can run in parallel
- Phase 7 depends on all prior phases

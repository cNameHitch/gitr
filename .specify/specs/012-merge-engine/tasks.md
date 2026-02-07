# Tasks: Merge Engine

**Input**: Design documents from `specs/012-merge-engine/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-merge/Cargo.toml` with deps: git-utils, git-hash, git-object, git-odb, git-index, git-diff, git-repository, bstr, thiserror
- [X] T002 Create `crates/git-merge/src/lib.rs` with MergeOptions, MergeResult, MergeError, ConflictEntry types

**Checkpoint**: `cargo check -p git-merge` compiles

---

## Phase 2: User Story 1 - Content Merge (Priority: P1)

**Goal**: Three-way file merge

- [X] T003 [US1] Implement `merge_content` in `crates/git-merge/src/content.rs` — three-way merge using diff hunks
- [X] T004 [US1] Implement conflict marker generation (merge and diff3 styles)
- [X] T005 [US1] Add content merge tests in `crates/git-merge/tests/content_merge.rs` — clean merges, conflicts, identical changes

**Checkpoint**: Content merge produces correct results for all scenarios

---

## Phase 3: User Story 2 - Tree Merge (Priority: P1)

**Goal**: Full tree-level ORT merge

- [X] T006 [US2] Implement ORT tree merge in `crates/git-merge/src/strategy/ort.rs` — three-way tree diff, classify changes
- [X] T007 [US2] Implement rename-following merge (detect renames, merge content at new path)
- [X] T008 [US2] Implement conflict classification: modify/delete, add/add, rename/rename, dir/file
- [X] T009 [US2] Implement virtual merge base computation (for criss-cross merges)
- [X] T010 [US2] Add tree merge tests in `crates/git-merge/tests/tree_merge.rs`

**Checkpoint**: ORT merge matches C git for all merge scenarios

---

## Phase 4: User Story 3 - Conflict Recording (Priority: P1)

- [X] T011 [US3] Implement conflict recording in `crates/git-merge/src/conflict.rs` — write conflict markers to working tree
- [X] T012 [US3] Implement index conflict stages (1/2/3) update
- [X] T013 [US3] Add conflict resolution tests

**Checkpoint**: Conflicts recorded correctly in index and working tree

---

## Phase 5: User Story 4 - Strategies (Priority: P2)

- [X] T014 [P] [US4] Implement MergeStrategy trait in `crates/git-merge/src/strategy/mod.rs`
- [X] T015 [P] [US4] Implement ours strategy in `crates/git-merge/src/strategy/ours.rs`
- [X] T016 [US4] Implement strategy options (theirs, patience, rename-threshold)

**Checkpoint**: Multiple merge strategies work

---

## Phase 6: User Story 5 - Cherry-Pick and Revert (Priority: P2)

- [X] T017 [US5] Implement cherry-pick in `crates/git-merge/src/cherry_pick.rs`
- [X] T018 [US5] Implement revert in `crates/git-merge/src/revert.rs`
- [X] T019 [US5] Add cherry-pick and revert tests in `crates/git-merge/tests/cherry_pick_tests.rs`

**Checkpoint**: Cherry-pick and revert match C git

---

## Phase 7: User Story 6 - Sequencer (Priority: P3)

- [X] T020 [US6] Implement Sequencer state machine in `crates/git-merge/src/sequencer.rs`
- [X] T021 [US6] Implement save/load for .git/sequencer/ state
- [X] T022 [US6] Implement continue, abort, skip operations
- [X] T023 [US6] Add sequencer tests

**Checkpoint**: Multi-commit operations with interrupt/continue work

---

## Phase 8: Patch Application

- [X] T024 Implement `git apply` in `crates/git-merge/src/apply.rs` — parse unified diff, apply to files
- [X] T025 Add apply tests

---

## Phase 9: Polish

- [X] T026 [P] Run `cargo clippy -p git-merge` and fix warnings
- [X] T027 Run `cargo test -p git-merge` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (content merge before tree merge)
- Phase 4 depends on Phase 3 (conflicts from tree merge)
- Phase 5 depends on Phase 3 (strategies use tree merge)
- Phase 6 depends on Phase 3 (cherry-pick uses merge)
- Phase 7 depends on Phase 6 (sequencer uses cherry-pick/revert)
- T014, T015 can run in parallel

### Cross-Spec Dependencies

- Spec 016 (porcelain) depends on: merge, cherry-pick, revert for git merge/cherry-pick/revert commands
- Spec 017 (history) depends on: cherry-pick, revert, sequencer for interactive rebase

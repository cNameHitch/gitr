# Tasks: Diff Engine

**Input**: Design documents from `specs/011-diff-engine/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [x] T001 Create `crates/git-diff/Cargo.toml` with dependencies: git-utils, git-hash, git-object, git-odb, git-index, git-repository, bstr, thiserror
- [x] T002 Create `crates/git-diff/src/lib.rs` with DiffOptions, DiffResult, FileDiff, Hunk, DiffError types

**Checkpoint**: `cargo check -p git-diff` compiles

---

## Phase 2: User Story 1 - Line Diff Algorithms (Priority: P1)

**Goal**: Core diff algorithms

- [x] T003 [US1] Implement Myers diff algorithm in `crates/git-diff/src/algorithm/myers.rs`
- [x] T004 [P] [US1] Implement histogram diff algorithm in `crates/git-diff/src/algorithm/histogram.rs`
- [x] T005 [P] [US1] Implement patience diff algorithm in `crates/git-diff/src/algorithm/patience.rs`
- [x] T006 [US1] Implement `diff_lines` function using DiffAlgorithm enum dispatch
- [x] T007 [US1] Add algorithm correctness tests in `crates/git-diff/tests/algorithm_tests.rs`

**Checkpoint**: All three algorithms produce correct minimal edit scripts

---

## Phase 3: User Story 2 - Tree Diff (Priority: P1)

- [x] T008 [US2] Implement tree-to-tree diff in `crates/git-diff/src/tree.rs` — walk both trees in parallel, identify changes
- [x] T009 [US2] Implement recursive tree diffing (handle nested trees)
- [x] T010 [US2] Implement binary file detection in `crates/git-diff/src/binary.rs`
- [x] T011 [US2] Add tree diff tests against C git output

**Checkpoint**: Tree diff identifies all changed files correctly

---

## Phase 4: User Story 4 - Output Formats (Priority: P1)

- [x] T012 [P] [US4] Implement unified diff format in `crates/git-diff/src/format/unified.rs`
- [x] T013 [P] [US4] Implement --stat format in `crates/git-diff/src/format/stat.rs`
- [x] T014 [P] [US4] Implement --raw format in `crates/git-diff/src/format/raw.rs`
- [x] T015 [P] [US4] Implement --name-only and --name-status in `crates/git-diff/src/format/nameonly.rs`
- [x] T016 [US4] Add output format compatibility tests in `crates/git-diff/tests/output_compat.rs`

**Checkpoint**: All output formats match C git byte-for-byte

---

## Phase 5: User Story 5 - Working Tree Diff (Priority: P1)

- [x] T017 [US5] Implement `diff_index_to_worktree` in `crates/git-diff/src/worktree.rs`
- [x] T018 [US5] Implement `diff_head_to_index` (staged changes)
- [x] T019 [US5] Implement stat-based change detection (check stat before reading content)

**Checkpoint**: `git diff` and `git diff --cached` equivalents work

---

## Phase 6: User Story 3 - Rename Detection (Priority: P2)

- [x] T020 [US3] Implement similarity scoring in `crates/git-diff/src/rename.rs`
- [x] T021 [US3] Implement exact rename detection (same OID)
- [x] T022 [US3] Implement fuzzy rename detection (similarity threshold)
- [x] T023 [US3] Implement copy detection (-C flag)
- [x] T024 [US3] Add rename detection tests in `crates/git-diff/tests/rename_tests.rs`

**Checkpoint**: Rename detection matches C git

---

## Phase 7: User Story 6 - Diffcore Pipeline (Priority: P2)

- [x] T025 [US6] Implement diffcore pipeline in `crates/git-diff/src/diffcore.rs` — break, rename, merge-broken, pickaxe, order
- [x] T026 [US6] Implement combined diff format for merge commits in `crates/git-diff/src/format/combined.rs`

**Checkpoint**: Full diffcore pipeline works

---

## Phase 8: Polish

- [x] T027 [P] Run `cargo clippy -p git-diff` and fix warnings
- [ ] T028 Create benchmarks in `crates/git-diff/benches/diff_bench.rs`
- [x] T029 Run `cargo test -p git-diff` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 (algorithms first)
- Phase 3 depends on Phase 2 (tree diff uses line diff)
- Phase 4 depends on Phase 2 (formatting needs hunks)
- Phase 5 depends on Phases 3+4 (working tree diff combines tree diff and formatting)
- Phase 6 depends on Phase 3 (rename detection works on tree diff results)
- Phase 7 depends on Phase 6
- T003, T004, T005 can run in parallel (independent algorithms)
- T012, T013, T014, T015 can run in parallel (independent formatters)

### Cross-Spec Dependencies

- Spec 012 (merge) depends on: line diff algorithms for content merge
- Spec 016 (porcelain) depends on: all diff operations for `git diff`, `git status`
- Spec 017 (history) depends on: tree diff for `git log -p`

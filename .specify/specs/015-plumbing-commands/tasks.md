# Tasks: Plumbing Commands

**Input**: Design documents from `specs/015-plumbing-commands/`
**Prerequisites**: All library crates (001-014) must be substantially complete

## Phase 1: CLI Framework Setup

- [X] T001 Create `src/main.rs` with clap CLI parser, global options (-C, -c, --git-dir)
- [X] T002 Create `src/commands/mod.rs` with command registry
- [X] T003 Implement repository opening logic (discover, apply -C and --git-dir overrides)

**Checkpoint**: CLI binary compiles and can open a repository

---

## Phase 2: User Story 1 - Object Commands (Priority: P1)

- [X] T004 [P] [US1] Implement `cat-file` in `src/commands/cat_file.rs` — -t, -s, -p, <type>, --batch, --batch-check
- [X] T005 [P] [US1] Implement `hash-object` in `src/commands/hash_object.rs` — --stdin, -w, -t
- [X] T006 [US1] Add integration tests for cat-file and hash-object

**Checkpoint**: Object inspection and creation work

---

## Phase 3: User Story 2 - Ref Commands (Priority: P1)

- [X] T007 [P] [US2] Implement `rev-parse` in `src/commands/rev_parse.rs` — revision resolution + --git-dir, --show-toplevel, etc.
- [X] T008 [P] [US2] Implement `update-ref` in `src/commands/update_ref.rs` — create, update, delete, --stdin transaction
- [X] T009 [P] [US2] Implement `for-each-ref` in `src/commands/for_each_ref.rs` — --format, --sort, --count
- [X] T010 [P] [US2] Implement `show-ref` in `src/commands/show_ref.rs`
- [X] T011 [P] [US2] Implement `symbolic-ref` in `src/commands/symbolic_ref.rs`
- [X] T012 [US2] Add integration tests for all ref commands

**Checkpoint**: All ref plumbing commands work

---

## Phase 4: User Story 3 - Index/Tree Commands (Priority: P1)

- [X] T013 [P] [US3] Implement `ls-files` in `src/commands/ls_files.rs` — --stage, --cached, --deleted, --modified, --others
- [X] T014 [P] [US3] Implement `ls-tree` in `src/commands/ls_tree.rs` — -r, -d, -t, --name-only
- [X] T015 [P] [US3] Implement `update-index` in `src/commands/update_index.rs` — --add, --remove, --cacheinfo
- [X] T016 [P] [US3] Implement `check-ignore` in `src/commands/check_ignore.rs`
- [X] T017 [P] [US3] Implement `check-attr` in `src/commands/check_attr.rs`
- [X] T018 [P] [US3] Implement `write-tree` in `src/commands/write_tree.rs`
- [X] T019 [US3] Add integration tests for index/tree commands

**Checkpoint**: All index/tree plumbing commands work

---

## Phase 5: User Story 4 - Object Creation Commands (Priority: P2)

- [X] T020 [P] [US4] Implement `mktree` in `src/commands/mktree.rs`
- [X] T021 [P] [US4] Implement `mktag` in `src/commands/mktag.rs`
- [X] T022 [P] [US4] Implement `commit-tree` in `src/commands/commit_tree.rs`
- [X] T023 [US4] Add integration tests

**Checkpoint**: Object creation commands work

---

## Phase 6: User Story 5 - Verification Commands (Priority: P2)

- [X] T024 [P] [US5] Implement `verify-pack` in `src/commands/verify_pack.rs`
- [X] T025 [P] [US5] Implement `check-ref-format` in `src/commands/check_ref_format.rs`
- [X] T026 [P] [US5] Implement `var` in `src/commands/var.rs`
- [X] T027 [US5] Add integration tests

**Checkpoint**: Verification commands work

---

## Phase 7: Polish

- [X] T028 Ensure all commands have correct exit codes (0, 1, 128)
- [X] T029 [P] Run `cargo clippy` on CLI and fix warnings
- [X] T030 Run full integration test suite comparing against C git

---

## Dependencies & Execution Order

- Phase 1 must complete first (CLI framework)
- Phases 2-6 can proceed in parallel after Phase 1 (commands are independent)
- All T00N marked [P] within a phase can run in parallel
- Phase 7 depends on all prior phases

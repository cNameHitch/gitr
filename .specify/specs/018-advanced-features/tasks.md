# Tasks: Advanced Features

**Input**: Design documents from `specs/018-advanced-features/`
**Prerequisites**: All library crates and command specs (001-017)

## Phase 1: User Story 1 - Garbage Collection (Priority: P1)

- [X] T001 [US1] Implement `gc` in `src/commands/gc.rs` — repack + prune + pack-refs + reflog expire
- [X] T002 [US1] Implement `repack` in `src/commands/repack.rs` — -a, -d, -f, bitmap generation
- [X] T003 [US1] Implement `prune` in `src/commands/prune.rs` — remove unreachable objects
- [X] T004 [P] [US1] Implement `pack-objects` in `src/commands/pack_objects.rs` — low-level pack creation
- [X] T005 [P] [US1] Implement `index-pack` in `src/commands/index_pack.rs` — create index from pack
- [ ] T006 [US1] Add GC integration tests

**Checkpoint**: GC produces equivalent results to C git

---

## Phase 2: User Story 2 - Fsck (Priority: P1)

- [X] T007 [US2] Implement fsck object validation (blob, tree, commit, tag checks)
- [X] T008 [US2] Implement fsck connectivity check (all referenced objects exist)
- [X] T009 [US2] Implement `fsck` command in `src/commands/fsck.rs`
- [ ] T010 [US2] Add fsck integration tests

**Checkpoint**: Fsck detects all corruption types

---

## Phase 3: User Story 8 - Hooks (Priority: P2)

- [X] T011 [US8] Implement HookRunner in library code (git-repository or git-utils)
- [X] T012 [US8] Wire hooks into commit, merge, push, checkout, rebase operations
- [ ] T013 [US8] Add hook execution tests

**Checkpoint**: All standard hooks execute at correct points

---

## Phase 4: User Story 7 - Security (Priority: P2)

- [X] T014 [US7] Implement GPG signing (subprocess calling gpg/gpg2) — sign_buffer, verify_signature
- [X] T015 [US7] Wire GPG signing into commit and tag creation
- [X] T016 [US7] Implement `verify-commit` and `verify-tag` commands
- [X] T017 [US7] Implement credential helper protocol in `src/commands/credential.rs`
- [ ] T018 [US7] Add GPG signing/verification tests

**Checkpoint**: GPG signing interoperates with C git

---

## Phase 5: User Story 6 - Archive (Priority: P2)

- [X] T019 [US6] Create `crates/git-archive/` crate with tar and zip generation
- [X] T020 [US6] Implement `archive` command in `src/commands/archive.rs` — --format, --prefix, --output
- [ ] T021 [US6] Add archive tests (compare against C git output)

**Checkpoint**: Archives are byte-identical to C git

---

## Phase 6: User Story 3 - Submodules (Priority: P2)

- [X] T022 [US3] Create `crates/git-submodule/` crate — .gitmodules parsing, submodule state
- [X] T023 [US3] Implement submodule add, init, update, status
- [X] T024 [US3] Implement `submodule` command in `src/commands/submodule.rs`
- [ ] T025 [US3] Add submodule integration tests

**Checkpoint**: Submodule operations match C git

---

## Phase 7: User Story 4 - Worktrees (Priority: P2)

- [X] T026 [US4] Implement `worktree` command in `src/commands/worktree.rs` — add, list, remove, lock, unlock
- [ ] T027 [US4] Add worktree integration tests

**Checkpoint**: Worktree management works

---

## Phase 8: User Story 5 - Notes and Replace (Priority: P3)

- [X] T028 [P] [US5] Implement `notes` command in `src/commands/notes.rs` — add, show, list, remove
- [X] T029 [P] [US5] Implement `replace` command in `src/commands/replace.rs` — create, delete, list
- [ ] T030 [US5] Add notes and replace tests

**Checkpoint**: Notes and replace work

---

## Phase 9: Remaining Commands

- [X] T031 [P] Implement `fast-import` in `src/commands/fast_import.rs` — streaming import format
- [X] T032 [P] Implement `bundle` in `src/commands/bundle.rs` — create, unbundle, verify
- [X] T033 Implement fsmonitor integration
- [X] T034 Implement `daemon` in `src/commands/daemon.rs` (basic git:// server)

---

## Phase 10: Polish

- [X] T035 [P] Run `cargo clippy` on all new code
- [X] T036 Run full integration test suite
- [ ] T037 Verify all 128 builtin commands are accounted for across specs 015-018

---

## Dependencies & Execution Order

- Phase 1 first (GC is most important advanced feature)
- Phase 2 can start in parallel with Phase 1
- Phases 3-8 can proceed in any order after Phase 1
- Phase 9 is lowest priority
- T004, T005 can run in parallel
- T028, T029 can run in parallel
- T031, T032 can run in parallel

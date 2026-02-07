# Tasks: Reference System

**Input**: Design documents from `specs/008-reference-system/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-ref/Cargo.toml` with dependencies: git-utils, git-hash, bstr, thiserror
- [X] T002 Create `crates/git-ref/src/lib.rs` with Reference enum, RefName, RefError types

**Checkpoint**: `cargo check -p git-ref` compiles

---

## Phase 2: Core Types (Blocking)

- [X] T003 Implement `RefName` validation in `crates/git-ref/src/name.rs` — all git-check-ref-format rules
- [X] T004 Implement `Reference` type with Direct/Symbolic variants
- [X] T005 Define `RefStore` trait in `crates/git-ref/src/store.rs`
- [X] T006 Define `RefTransaction`, `RefUpdate`, `RefUpdateAction` types

**Checkpoint**: All core types compile and are validated

---

## Phase 3: User Story 1 - Resolve Refs (Priority: P1)

**Goal**: Resolve ref names to OIDs

- [X] T007 [US1] Implement loose ref resolution in `crates/git-ref/src/files/loose.rs` — read file, parse OID or symref
- [X] T008 [US1] Implement packed-refs parsing in `crates/git-ref/src/files/packed.rs` — parse file, binary search, peeled refs
- [X] T009 [US1] Implement `FilesRefStore::resolve` — check loose first, fall back to packed
- [X] T010 [US1] Implement symbolic ref chain following (with loop detection)
- [X] T011 [US1] Handle special refs: HEAD, MERGE_HEAD, CHERRY_PICK_HEAD, etc.
- [X] T012 [US1] Add interop tests in `crates/git-ref/tests/resolve_interop.rs`

**Checkpoint**: All ref types resolve correctly, matching C git

---

## Phase 4: User Story 3 - Enumerate Refs (Priority: P1)

- [X] T013 [US3] Implement ref iteration in `crates/git-ref/src/iter.rs` — merge loose and packed refs, deduplicate
- [X] T014 [US3] Implement prefix filtering for iteration
- [X] T015 [US3] Add enumeration tests (list branches, tags, all refs)

**Checkpoint**: Ref listing matches `git for-each-ref`

---

## Phase 5: User Story 2 - Update Refs (Priority: P1)

**Goal**: Atomic ref creation, update, deletion

- [X] T016 [US2] Implement single ref write in `crates/git-ref/src/files/loose.rs` — lock file, write OID, commit
- [X] T017 [US2] Implement `RefTransaction::commit` in `crates/git-ref/src/files/transaction.rs` — lock all, verify CAS, write all, commit all
- [X] T018 [US2] Implement symbolic ref write (create file with `ref: <target>`)
- [X] T019 [US2] Implement ref deletion (remove loose file, update packed-refs)
- [X] T020 [US2] Add update interop tests in `crates/git-ref/tests/update_interop.rs`

**Checkpoint**: Ref updates are atomic and interoperable with C git

---

## Phase 6: User Story 6 - Packed Refs (Priority: P1)

- [X] T021 [US6] Implement packed-refs writing (add ref, remove ref) with lock file
- [X] T022 [US6] Implement pack operation: move loose ref into packed-refs
- [X] T023 [US6] Verify loose-over-packed precedence

**Checkpoint**: Packed refs fully functional

---

## Phase 7: User Story 4 - Reflog (Priority: P2)

- [X] T024 [US4] Implement `ReflogEntry::parse` and `to_bytes` in `crates/git-ref/src/reflog.rs`
- [X] T025 [US4] Implement reflog reading (parse `.git/logs/refs/...`)
- [X] T026 [US4] Implement reflog appending on ref updates
- [X] T027 [US4] Implement `@{N}` and `@{date}` reflog lookup
- [X] T028 [US4] Add reflog interop tests in `crates/git-ref/tests/reflog_interop.rs`

**Checkpoint**: Reflogs match C git format and content

---

## Phase 8: Polish

- [X] T029 Add concurrent update stress test in `crates/git-ref/tests/concurrent.rs`
- [X] T030 [P] Run `cargo clippy -p git-ref` and fix warnings
- [X] T031 Run `cargo test -p git-ref` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (sequential core)
- Phase 4 depends on Phase 3 (needs resolve for iteration)
- Phase 5 depends on Phase 3 (needs resolve for CAS checks)
- Phase 6 depends on Phase 3 (needs packed-refs parsing)
- Phase 7 depends on Phase 5 (reflogs written during updates)
- Phases 4 and 5 can start in parallel after Phase 3

### Cross-Spec Dependencies

- Spec 010 (repository) depends on: HEAD resolution, ref store initialization
- Spec 013 (rev-walk) depends on: ref resolution for starting points
- Spec 016 (porcelain) depends on: all ref operations (branch, tag, checkout)

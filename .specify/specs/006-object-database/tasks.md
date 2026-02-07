# Tasks: Object Database

**Input**: Design documents from `specs/006-object-database/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [x]T001 Create `crates/git-odb/Cargo.toml` with dependencies: git-utils, git-hash, git-object, git-loose, git-pack, thiserror
- [x]T002 Create `crates/git-odb/src/lib.rs` with OdbError, ObjectDatabase struct shell

**Checkpoint**: `cargo check -p git-odb` compiles

---

## Phase 2: Backend Trait (Blocking)

- [x]T003 Define `OdbBackend` trait in `crates/git-odb/src/backend.rs`
- [x]T004 Implement `OdbBackend` for `LooseObjectStore`
- [x]T005 Implement `OdbBackend` for pack file collection

**Checkpoint**: Both backends implement the trait

---

## Phase 3: User Story 1 - Unified Read (Priority: P1)

**Goal**: Single interface to read from any storage

- [x]T006 [US1] Implement `ObjectDatabase::open` in `crates/git-odb/src/lib.rs` — discover loose store and pack files
- [x]T007 [US1] Implement multi-source search in `crates/git-odb/src/search.rs` — loose → packs → alternates
- [x]T008 [US1] Implement `ObjectDatabase::read` — delegate to search, return first found
- [x]T009 [US1] Implement `ObjectDatabase::read_header` — type+size without full read
- [x]T010 [US1] Add tests: read from mixed loose/packed storage in `crates/git-odb/tests/unified_read.rs`

**Checkpoint**: Objects readable from any storage backend

---

## Phase 4: User Story 2 & 4 - Write and Existence (Priority: P1)

- [x]T011 [P] [US2] Implement `ObjectDatabase::write` and `write_raw` — delegate to loose store
- [x]T012 [P] [US4] Implement `ObjectDatabase::contains` — check loose, then packs, then alternates

**Checkpoint**: Write and existence checks work

---

## Phase 5: User Story 3 - Alternates (Priority: P2)

- [x]T013 [US3] Implement alternates file parsing in `crates/git-odb/src/alternates.rs`
- [x]T014 [US3] Implement recursive alternates loading with circular detection
- [x]T015 [US3] Wire alternates into search order
- [x]T016 [US3] Add alternates tests in `crates/git-odb/tests/alternates.rs`

**Checkpoint**: Alternates chain resolution works

---

## Phase 6: Prefix Resolution and Extras

- [x]T017 [US1] Implement `ObjectDatabase::resolve_prefix` in `crates/git-odb/src/prefix.rs` — search all backends, collect matches, ambiguity detection
- [x]T018 Implement `ObjectDatabase::refresh` — re-scan pack directory
- [x]T019 Implement `ObjectDatabase::iter_all_oids` — combine loose iter + pack iters
- [x]T020 Add caching integration (ObjectCache from git-object)

**Checkpoint**: Full ODB API complete

---

## Phase 7: Thread Safety and Polish

- [x]T021 Add concurrent read stress test in `crates/git-odb/tests/concurrent.rs`
- [x]T022 [P] Run `cargo clippy -p git-odb` and fix warnings
- [x]T023 Run `cargo test -p git-odb` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (sequential core)
- Phase 4 can start in parallel with Phase 3 (after Phase 2)
- Phase 5 depends on Phase 3 (needs search logic)
- Phase 6 depends on Phase 3
- T011, T012 can run in parallel

### Cross-Spec Dependencies

- Spec 007 (index) depends on: ODB for reading tree objects, writing blobs
- Spec 008 (refs) depends on: ODB for verifying ref targets exist
- Spec 010 (repository) depends on: ObjectDatabase as central component
- Spec 013 (rev-walk) depends on: reading commits from ODB
- All command specs depend on: ODB for object access

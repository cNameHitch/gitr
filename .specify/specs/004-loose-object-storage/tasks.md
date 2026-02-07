# Tasks: Loose Object Storage

**Input**: Design documents from `specs/004-loose-object-storage/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-loose/Cargo.toml` with dependencies: git-utils, git-hash, git-object, flate2, thiserror
- [X] T002 Create `crates/git-loose/src/lib.rs` with `LooseObjectStore` struct shell and `LooseError` types

**Checkpoint**: `cargo check -p git-loose` compiles

---

## Phase 2: User Story 1 - Read Loose Objects (Priority: P1)

**Goal**: Read objects from the loose store

- [X] T003 [US1] Implement path calculation: OID → `.git/objects/XX/YYY...` in `crates/git-loose/src/lib.rs`
- [X] T004 [US1] Implement `LooseObjectStore::read` in `crates/git-loose/src/read.rs` — open file, zlib decompress, parse header, parse object
- [X] T005 [US1] Implement `LooseObjectStore::read_header` — partial decompression for type+size only
- [X] T006 [US1] Implement `LooseObjectStore::contains` — file existence check
- [X] T007 [US1] Add interop tests: create objects with C git, read with gitr in `crates/git-loose/tests/interop.rs`

**Checkpoint**: Can read any loose object created by C git

---

## Phase 3: User Story 2 - Write Loose Objects (Priority: P1)

**Goal**: Write objects to the loose store

- [X] T008 [US2] Implement `LooseObjectStore::write` in `crates/git-loose/src/write.rs` — hash content, check existence, create temp, compress, rename
- [X] T009 [US2] Implement `LooseObjectStore::write_raw` for writing from type + raw content
- [X] T010 [US2] Implement fan-out directory creation (mkdir for XX/ if needed)
- [X] T011 [US2] Add interop tests: write objects with gitr, read with C git cat-file

**Checkpoint**: Objects written by gitr are readable by C git

---

## Phase 4: User Story 3 - Enumerate Objects (Priority: P2)

- [X] T012 [US3] Implement `LooseObjectStore::iter` in `crates/git-loose/src/enumerate.rs` — walk objects/ subdirectories
- [X] T013 [US3] Implement `LooseObjectIter` — iterate through fan-out dirs 00-ff, parse filenames to OIDs
- [X] T014 [US3] Add enumeration tests

**Checkpoint**: All loose objects in a repository are enumerable

---

## Phase 5: User Story 4 - Streaming (Priority: P2)

- [X] T015 [US4] Implement `LooseObjectStore::stream` in `crates/git-loose/src/stream.rs` — open file, decompress header, return streaming reader
- [X] T016 [US4] Implement `LooseObjectStream` with `Read` trait
- [X] T017 [US4] Implement `LooseObjectStore::write_stream` for writing from a reader
- [X] T018 [US4] Add streaming tests in `crates/git-loose/tests/interop.rs`

**Checkpoint**: Large objects can be read/written without loading fully into memory

---

## Phase 6: Polish

- [X] T019 [P] Run `cargo clippy -p git-loose` and fix warnings
- [X] T020 [P] Add hash verification on read (`read_verified()` method)
- [X] T021 Create benchmarks in `crates/git-loose/benches/loose_bench.rs`
- [X] T022 Run `cargo test -p git-loose` — all 23 tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (sequential core path)
- Phase 4 depends on Phase 1 (path calculation)
- Phase 5 depends on Phase 2 (read infrastructure)
- Phases 3, 4 can start in parallel after Phase 2

### Cross-Spec Dependencies

- Spec 006 (ODB) depends on: LooseObjectStore (all of it)
- Spec 007 (index) depends on: write_raw for blob creation during staging
- Spec 015 (plumbing) depends on: read/write for cat-file, hash-object commands

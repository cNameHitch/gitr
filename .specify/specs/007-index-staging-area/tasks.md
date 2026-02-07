# Tasks: Index / Staging Area

**Input**: Design documents from `specs/007-index-staging-area/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [x] T001 Create `crates/git-index/Cargo.toml` with dependencies: git-utils, git-hash, git-object, git-odb, bstr, thiserror
- [x] T002 Create `crates/git-index/src/lib.rs` with Index struct shell, IndexError, Stage enum

**Checkpoint**: `cargo check -p git-index` compiles

---

## Phase 2: Core Entry Types (Blocking)

- [x] T003 Implement `IndexEntry`, `StatData`, `EntryFlags` in `crates/git-index/src/entry.rs`
- [x] T004 Implement `Stage` enum with as_u8, from_u8
- [x] T005 Implement `StatData::from_metadata` and `matches`

**Checkpoint**: Entry types compile with all fields

---

## Phase 3: User Story 1 - Read Index (Priority: P1)

**Goal**: Parse the index file

- [x] T006 [US1] Implement index file reader in `crates/git-index/src/read.rs` — parse header, version, entry count
- [x] T007 [US1] Implement v2 cache entry parsing — stat data, OID, flags, path, padding
- [x] T008 [US1] Implement v3 extended flags parsing (intent-to-add, skip-worktree)
- [x] T009 [US1] Implement v4 path prefix compression
- [x] T010 [US1] Implement extension parsing in `crates/git-index/src/extensions/mod.rs` — dispatch by signature
- [x] T011 [US1] Implement checksum verification
- [x] T012 [US1] Add read tests: parse C git-generated indexes in `crates/git-index/tests/read_write_roundtrip.rs`

**Checkpoint**: Can read any C git index file

---

## Phase 4: User Story 2 - Write Index (Priority: P1)

- [x] T013 [US2] Implement index file writer in `crates/git-index/src/write.rs` — header, entries, extensions, checksum
- [x] T014 [US2] Implement atomic write with lock file
- [x] T015 [US2] Ensure entries are written in sorted order
- [x] T016 [US2] Add round-trip tests: read → write → read, verify identical

**Checkpoint**: Written index files are readable by C git

---

## Phase 5: User Story 3 - Stage Operations (Priority: P1)

- [x] T017 [US3] Implement `Index::add` — insert/update entry, maintain sort
- [x] T018 [US3] Implement `Index::remove` — delete entry by path/stage
- [x] T019 [US3] Implement `Index::get`, `get_all`, conflict detection
- [x] T020 [US3] Implement `Index::write_tree` — create tree objects from index entries

**Checkpoint**: Full staging workflow works

---

## Phase 6: User Story 7 - Cache Tree (Priority: P2)

- [x] T021 [US7] Implement `CacheTree::parse` and `serialize` in `crates/git-index/src/extensions/tree.rs`
- [x] T022 [US7] Implement `CacheTree::invalidate` — invalidate path and ancestors
- [x] T023 [US7] Wire cache tree into `Index::write_tree` for optimized tree creation

**Checkpoint**: Cache tree speeds up commit operations

---

## Phase 7: User Story 4 - Gitignore (Priority: P2)

- [x] T024 [US4] Implement `.gitignore` parsing in `crates/git-index/src/ignore.rs` — patterns, negation, anchoring, directory-only
- [x] T025 [US4] Implement `IgnoreStack::load` — load from all sources (core.excludesFile, info/exclude, .gitignore files)
- [x] T026 [US4] Implement `IgnoreStack::is_ignored` — evaluate patterns in priority order
- [x] T027 [US4] Add ignore compatibility tests in `crates/git-index/tests/ignore_compat.rs`

**Checkpoint**: Gitignore matching is identical to C git

---

## Phase 8: User Story 6 - Pathspec (Priority: P2)

- [x] T028 [US6] Implement `Pathspec::parse` in `crates/git-index/src/pathspec.rs` — magic signatures, pattern extraction
- [x] T029 [US6] Implement `Pathspec::matches` — evaluate against paths
- [x] T030 [US6] Implement `Index::iter_matching` using Pathspec
- [x] T031 [US6] Add pathspec tests in `crates/git-index/tests/pathspec_compat.rs`

**Checkpoint**: Pathspec matching works for all magic types

---

## Phase 9: User Story 5 - Gitattributes (Priority: P2)

- [x] T032 [US5] Implement `.gitattributes` parsing in `crates/git-index/src/attributes.rs`
- [x] T033 [US5] Implement attribute lookup for a given path

**Checkpoint**: Gitattributes are readable and queryable

---

## Phase 10: Extensions and Polish

- [x] T034 [P] Implement REUC (resolve-undo) extension in `crates/git-index/src/extensions/resolve_undo.rs`
- [x] T035 [P] Implement UNTR (untracked cache) extension in `crates/git-index/src/extensions/untracked.rs`
- [x] T036 [P] Run `cargo clippy -p git-index` and fix warnings
- [x] T037 Run `cargo test -p git-index` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 → Phase 4 (sequential core)
- Phase 5 depends on Phases 3+4 (needs read/write)
- Phase 6 depends on Phase 3 (needs parsed extensions)
- Phases 7, 8, 9 can start in parallel after Phase 5
- T024, T028, T032 can run in parallel (different modules)

### Cross-Spec Dependencies

- Spec 010 (repository) depends on: Index reading for status
- Spec 011 (diff) depends on: Index for diff --cached
- Spec 012 (merge) depends on: Index for merge conflict resolution
- Spec 016 (porcelain) depends on: Index for add, rm, status, commit

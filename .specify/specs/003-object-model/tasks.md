# Tasks: Object Model

**Input**: Design documents from `specs/003-object-model/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-object/Cargo.toml` with dependencies: git-utils, git-hash, bstr, thiserror, lru
- [X] T002 Create `crates/git-object/src/lib.rs` with module declarations, ObjectType enum, Object enum shells

**Checkpoint**: `cargo check -p git-object` compiles

---

## Phase 2: Core Types (Blocking)

**Purpose**: Types everything else depends on

- [X] T003 Implement `ObjectError` error types in `crates/git-object/src/lib.rs`
- [X] T004 Implement `ObjectType` with from_bytes, as_bytes, Display in `crates/git-object/src/lib.rs`
- [X] T005 Implement `FileMode` enum in `crates/git-object/src/tree.rs` — from_bytes, as_bytes, raw, is_tree, is_blob
- [X] T006 Implement `parse_header` and `write_header` in `crates/git-object/src/header.rs`

**Checkpoint**: Object type and header parsing works

---

## Phase 3: User Story 1 - Parse Objects (Priority: P1)

**Goal**: Parse all four object types from raw bytes

- [X] T007 [US1] Implement `Blob::parse` in `crates/git-object/src/blob.rs`
- [X] T008 [US1] Implement `TreeEntry` and `Tree::parse` in `crates/git-object/src/tree.rs` — parse binary tree entry format, collect entries
- [X] T009 [US1] Implement `Commit::parse` in `crates/git-object/src/commit.rs` — extract tree, parents, author, committer, encoding, gpgsig, extra_headers, message
- [X] T010 [US1] Implement `Tag::parse` in `crates/git-object/src/tag.rs` — extract target, type, name, tagger, message
- [X] T011 [US1] Implement `Object::parse` and `Object::parse_content` in `crates/git-object/src/lib.rs` — delegate to type-specific parsers

**Checkpoint**: All four object types parse correctly

---

## Phase 4: User Story 2 - Serialize Objects (Priority: P1)

**Goal**: Byte-identical serialization

- [X] T012 [P] [US2] Implement `Blob::serialize_content` in `crates/git-object/src/blob.rs`
- [X] T013 [US2] Implement `Tree::serialize_content` in `crates/git-object/src/tree.rs` — sort entries, write binary format
- [X] T014 [US2] Implement `TreeEntry::cmp_entries` for git's tree sort order (dirs sort as name + '/')
- [X] T015 [US2] Implement `Commit::serialize_content` in `crates/git-object/src/commit.rs`
- [X] T016 [US2] Implement `Tag::serialize_content` in `crates/git-object/src/tag.rs`
- [X] T017 [US2] Implement `Object::serialize`, `serialize_content`, `compute_oid` in `crates/git-object/src/lib.rs`
- [X] T018 [US2] Add round-trip tests in `crates/git-object/tests/serialize_roundtrip.rs` — parse → serialize → parse, verify identical
- [X] T019 [US2] Add C-git compatibility tests — serialize objects and compare OID against `git hash-object`

**Checkpoint**: Serialized objects hash identically to C git

---

## Phase 5: User Story 3 - Commit/Tree Helpers (Priority: P1)

- [X] T020 [US3] Implement Commit helper methods: first_parent, is_merge, is_root, summary, body
- [X] T021 [US3] Implement Tree helper methods: sort, find, iter
- [X] T022 [P] [US3] Add tree sorting edge case tests in `crates/git-object/tests/tree_sorting.rs`

**Checkpoint**: All helper methods work correctly

---

## Phase 6: User Story 4 - Object Name Resolution (Priority: P2)

**Goal**: Rev-parse logic for resolving object names

- [X] T023 [US4] Design name resolution trait/interface in `crates/git-object/src/name.rs` — will need ODB and ref store (define trait, defer impl)
- [X] T024 [US4] Implement hex prefix resolution (full hex → OID, short hex → lookup)
- [X] T025 [US4] Implement revision suffix parsing: ^, ~N, ^{type}, ^{/regex}
- [X] T026 [US4] Add name resolution tests with mock ODB

**Checkpoint**: Basic rev-parse syntax works

---

## Phase 7: User Story 5 - Object Cache (Priority: P3)

- [X] T027 [US5] Implement `ObjectCache` in `crates/git-object/src/cache.rs` — LRU cache with configurable capacity
- [X] T028 [US5] Add cache tests (hit, miss, eviction, clear)

**Checkpoint**: Cache works correctly with LRU eviction

---

## Phase 8: Polish

- [X] T029 [P] Run `cargo clippy -p git-object` and fix warnings
- [X] T030 Create benchmarks in `crates/git-object/benches/parse_bench.rs` — commit parse, tree parse, serialize
- [X] T031 Run `cargo test -p git-object` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 → Phase 4 (sequential core path)
- Phase 5 depends on Phases 3+4
- Phase 6 depends on Phase 2 (types only, actual resolution needs ODB from spec 006)
- Phase 7 can start after Phase 2
- Phases 6 and 7 can run in parallel

### Cross-Spec Dependencies

- Spec 004 (loose storage) depends on: Object parse/serialize (T011, T017), ObjectType (T004)
- Spec 005 (packfile) depends on: Object parse (T011), header parsing (T006)
- Spec 006 (ODB) depends on: full Object API
- Spec 007 (index) depends on: FileMode (T005), TreeEntry for cache-tree
- Spec 013 (rev-walk) depends on: Commit type (T009), name resolution (T023-T026)

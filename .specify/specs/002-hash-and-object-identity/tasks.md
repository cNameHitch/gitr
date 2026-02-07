# Tasks: Hash & Object Identity

**Input**: Design documents from `specs/002-hash-and-object-identity/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-hash/Cargo.toml` with dependencies: sha1, sha2, thiserror, git-utils (path dep)
- [X] T002 Create `crates/git-hash/src/lib.rs` with module declarations

**Checkpoint**: `cargo check -p git-hash` compiles

---

## Phase 2: Core OID Type (Blocking)

**Purpose**: The ObjectId type everything else depends on

- [X] T003 [US1] Implement `HashAlgorithm` enum in `crates/git-hash/src/algorithm.rs` — Sha1/Sha256, digest_len, hex_len, null_oid
- [X] T004 [US1] Implement `ObjectId` enum in `crates/git-hash/src/oid.rs` — from_bytes, from_hex, as_bytes, to_hex, is_null, Display, FromStr, Debug, Eq, Ord, Hash
- [X] T005 [US1] Implement `HashError` in `crates/git-hash/src/lib.rs` or dedicated error module
- [X] T006 [US3] Implement hex encoding/decoding in `crates/git-hash/src/hex.rs` — hex_encode, hex_decode, hex_to_string, hex_to_bytes

**Checkpoint**: ObjectId can be created from hex, displayed, compared, and used as HashMap key

---

## Phase 3: User Story 2 - Hash Computation (Priority: P1)

**Goal**: Compute hashes from content

- [X] T007 [US2] Implement `Hasher` in `crates/git-hash/src/hasher.rs` — new, update, finalize, digest, hash_object, Write impl
- [X] T008 [US2] Add hash test vectors in `crates/git-hash/tests/hash_vectors.rs` — empty string, known content, git object format
- [X] T009 [P] [US2] Add SHA-256 test vectors alongside SHA-1 tests

**Checkpoint**: Hash computation matches `git hash-object` output

---

## Phase 4: User Story 4 - OID Collections (Priority: P2)

**Goal**: Efficient OID storage and lookup

- [X] T010 [P] [US4] Implement `OidArray` in `crates/git-hash/src/collections/oid_array.rs` — push, sort, contains, lookup, for_each_unique
- [X] T011 [P] [US4] Implement `OidMap<V>` in `crates/git-hash/src/collections/oid_map.rs`
- [X] T012 [P] [US4] Implement `OidSet` in `crates/git-hash/src/collections/oid_set.rs`
- [X] T013 [US4] Implement `FanoutTable` in `crates/git-hash/src/fanout.rs` — build, range, from_bytes, to_bytes
- [X] T014 [US4] Add collection tests in `crates/git-hash/tests/collection_tests.rs`

**Checkpoint**: All OID collections pass tests with 10K+ OIDs

---

## Phase 5: User Story 5 - Algorithm Pluggability (Priority: P2)

- [X] T015 [US5] Ensure all APIs accept `HashAlgorithm` parameter where appropriate
- [X] T016 [US5] Add integration tests that run the full test suite under both SHA-1 and SHA-256

**Checkpoint**: Full test suite passes for both hash algorithms

---

## Phase 6: Polish

- [X] T017 [P] Property-based tests for hex round-trip in `crates/git-hash/tests/hex_roundtrip.rs`
- [X] T018 [P] Create benchmarks in `crates/git-hash/benches/hash_bench.rs` — hash throughput, hex encode/decode, OID comparison
- [X] T019 Run `cargo clippy -p git-hash` and fix warnings
- [X] T020 Run `cargo test -p git-hash` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (sequential, each depends on prior)
- Phase 4 depends on Phase 2 (needs ObjectId type)
- Phase 5 depends on Phases 3+4
- Phase 3 and Phase 4 can run in parallel after Phase 2
- T010, T011, T012 can all run in parallel (different files)

### Cross-Spec Dependencies

- Spec 003 (object model) depends on: ObjectId (T004), Hasher (T007)
- Spec 004 (loose storage) depends on: ObjectId (T004), Hasher (T007), hex (T006)
- Spec 005 (packfile) depends on: OidArray (T010), FanoutTable (T013), Hasher (T007)
- Spec 006 (ODB) depends on: all of this crate

# Tasks: Packfile System

**Input**: Design documents from `specs/005-packfile-system/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-pack/Cargo.toml` with dependencies: git-utils, git-hash, git-object, flate2, memmap2, crc32fast, thiserror
- [X] T002 Create `crates/git-pack/src/lib.rs` with module declarations, PackError types, PackEntryType enum

**Checkpoint**: `cargo check -p git-pack` compiles

---

## Phase 2: Pack Index (Blocking)

**Purpose**: Index reading is needed for any pack object lookup

- [X] T003 [US2] Implement `PackIndex::open` in `crates/git-pack/src/index.rs` — mmap index file, validate header, read fanout
- [X] T004 [US2] Implement `PackIndex::lookup` — fan-out + binary search for OID → offset
- [X] T005 [US2] Implement `PackIndex::oid_at_index`, `offset_at_index`, `crc32_at_index`
- [X] T006 [US2] Implement 64-bit offset handling for large packs
- [X] T007 [US2] Add index tests: lookup known OIDs in C git-generated indexes

**Checkpoint**: Pack index lookup works for v2 indexes

---

## Phase 3: User Story 3 - Delta Encoding (Priority: P1)

**Goal**: Apply delta instructions to reconstruct objects

- [X] T008 [P] [US3] Implement `parse_delta_instructions` in `crates/git-pack/src/delta/mod.rs` — parse source/target size, copy/insert instructions
- [X] T009 [US3] Implement `apply_delta` in `crates/git-pack/src/delta/apply.rs` — apply instruction stream to base, bounds checking
- [X] T010 [US3] Add delta test vectors in `crates/git-pack/tests/delta_vectors.rs`

**Checkpoint**: Delta application produces correct results for all test vectors

---

## Phase 4: User Story 1 - Read Pack Objects (Priority: P1)

**Goal**: Read any object from a packfile

- [X] T011 [US1] Implement pack entry header parsing in `crates/git-pack/src/entry.rs` — variable-length type+size, OFS_DELTA offset, REF_DELTA OID
- [X] T012 [US1] Implement `PackFile::open` in `crates/git-pack/src/pack.rs` — mmap pack, validate header/trailer, open index
- [X] T013 [US1] Implement `PackFile::read_at_offset` — read entry header, decompress, resolve deltas
- [X] T014 [US1] Implement delta chain resolution (iterative, not recursive) for arbitrary-depth chains
- [X] T015 [US1] Implement `PackFile::read_object` — use index to find offset, then read_at_offset
- [X] T016 [US1] Add tests reading objects from C git-generated packfiles in `crates/git-pack/tests/read_real_packs.rs`

**Checkpoint**: All objects in C git packs are readable

---

## Phase 5: User Story 4 - Pack Generation (Priority: P2)

- [X] T017 [US4] Implement `PackWriter` in `crates/git-pack/src/write.rs` — header, add objects, checksummed trailer
- [X] T018 [US4] Implement `compute_delta` in `crates/git-pack/src/delta/compute.rs` — diff-delta algorithm
- [X] T019 [US4] Implement `build_pack_index` — create .idx v2 from .pack file
- [X] T020 [US4] Add round-trip tests: write pack → read back → verify all objects
- [X] T020a [US4] Implement thin pack support in `PackWriter` — `set_thin(true)` allows REF_DELTA entries whose base OID is not in the pack
- [X] T020b [US4] Implement `generate_pack` in `crates/git-pack/src/generate.rs` — given wants/haves OID sets, walk reachable objects, compute deltas, write thin or full pack to an output stream
- [X] T020c [US4] Add thin pack tests: generate thin pack from wants/haves, verify C git `index-pack --fix-thin` accepts it

**Checkpoint**: Generated packs pass `git verify-pack`; thin packs accepted by C git `receive-pack`

---

## Phase 6: User Story 5 & 6 - MIDX and Bitmap (Priority: P3)

- [X] T021 [P] [US5] Implement `MultiPackIndex` in `crates/git-pack/src/midx.rs` — open, lookup, iterate
- [X] T022 [P] [US6] Implement `BitmapIndex` in `crates/git-pack/src/bitmap.rs` — open, reachable_from (EWAH bitmap)
- [X] T023 [P] Implement `ReverseIndex` in `crates/git-pack/src/revindex.rs` — build from index, open .rev file
- [X] T024 Add MIDX and bitmap tests

**Checkpoint**: Multi-pack index and bitmap queries work

---

## Phase 7: Verification and Polish

- [X] T025 Implement `PackFile::verify_checksum` in `crates/git-pack/src/verify.rs`
- [X] T026 [P] Implement `PackFile::iter` for iterating all objects
- [X] T027 [P] Run `cargo clippy -p git-pack` and fix warnings
- [X] T028 Create benchmarks: pack read throughput, delta application, index lookup
- [X] T029 Run `cargo test -p git-pack` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 (index needed for lookup)
- Phase 3 can start in parallel with Phase 2 (delta is independent)
- Phase 4 depends on Phases 2 + 3 (needs index + delta)
- Phase 5 depends on Phase 4 (needs pack reading to verify writes)
- Phase 6 depends on Phase 2 (needs index format understanding)
- T021, T022, T023 can all run in parallel

### Cross-Spec Dependencies

- Spec 006 (ODB) depends on: PackFile (all read operations)
- Spec 014 (transport) depends on: PackWriter for sending packs, thin pack support
- Spec 018 (advanced) depends on: pack generation for gc/repack

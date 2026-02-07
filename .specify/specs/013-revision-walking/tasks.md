# Tasks: Revision Walking

**Input**: Design documents from `specs/013-revision-walking/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-revwalk/Cargo.toml` with deps: git-utils, git-hash, git-object, git-odb, git-ref, git-repository, bstr, thiserror, memmap2
- [X] T002 Create `crates/git-revwalk/src/lib.rs` with RevWalk struct shell, RevWalkError, SortOrder, WalkOptions

**Checkpoint**: `cargo check -p git-revwalk` compiles

---

## Phase 2: User Story 1 - Basic Traversal (Priority: P1)

**Goal**: Walk commit history in order

- [X] T003 [US1] Implement RevWalk core in `crates/git-revwalk/src/walk.rs` — push, hide, Iterator impl with priority queue
- [X] T004 [US1] Implement chronological sort (max-heap by committer date)
- [X] T005 [US1] Implement topological sort (Kahn's algorithm)
- [X] T006 [US1] Implement author-date sort and reverse
- [X] T007 [US1] Implement --first-parent filtering
- [X] T008 [US1] Add walk order tests in `crates/git-revwalk/tests/walk_order.rs` — compare against git rev-list

**Checkpoint**: Walk order matches git rev-list for all sort modes

---

## Phase 3: User Story 2 - Revision Ranges (Priority: P1)

- [X] T009 [US2] Implement revision range parsing in `crates/git-revwalk/src/range.rs` — A..B, A...B, ^A B
- [X] T010 [US2] Implement push_all, push_branches, push_tags
- [X] T011 [US2] Add range tests

**Checkpoint**: Revision ranges produce correct commit sets

---

## Phase 4: User Story 4 - Merge Base (Priority: P1)

- [X] T012 [US4] Implement merge_base in `crates/git-revwalk/src/merge_base.rs` — paint algorithm
- [X] T013 [US4] Implement merge_base_one (single best base)
- [X] T014 [US4] Implement is_ancestor check
- [X] T015 [US4] Add merge-base tests in `crates/git-revwalk/tests/merge_base_tests.rs`

**Checkpoint**: Merge base matches git merge-base for all test cases

---

## Phase 5: User Story 3 - Commit-Graph (Priority: P2)

- [X] T016 [US3] Implement commit-graph parser in `crates/git-revwalk/src/commit_graph/parse.rs`
- [X] T017 [US3] Implement CommitGraph lookup and CommitGraphEntry access
- [X] T018 [US3] Wire commit-graph into RevWalk (prefer graph over ODB)
- [X] T019 [US3] Wire generation numbers into merge-base for pruning
- [X] T020 [US3] Add commit-graph tests

**Checkpoint**: Commit-graph acceleration works, results identical to non-accelerated

---

## Phase 6: User Story 5 - Pretty-Print (Priority: P2)

- [X] T021 [US5] Implement format_commit with all % specifiers in `crates/git-revwalk/src/pretty.rs`
- [X] T022 [US5] Implement --oneline, --short, --medium, --full, --fuller, --email formats
- [X] T023 [US5] Implement ASCII graph drawing in `crates/git-revwalk/src/graph_draw.rs`
- [X] T024 [US5] Add pretty-print tests in `crates/git-revwalk/tests/pretty_tests.rs`

**Checkpoint**: Format output matches git log exactly

---

## Phase 7: User Story 6 - Object Listing (Priority: P2)

- [X] T025 [US6] Implement list_objects in `crates/git-revwalk/src/objects.rs` — walk commits, trees, blobs
- [X] T026 [US6] Implement ObjectFilter (blob:none, blob:limit, tree:depth) in `crates/git-revwalk/src/filter.rs`
- [X] T027 [US6] Add object listing tests

**Checkpoint**: Object listing matches git rev-list --objects

---

## Phase 8: Polish

- [X] T028 [P] Run `cargo clippy -p git-revwalk` and fix warnings
- [ ] T029 Create benchmarks for walk, merge-base, commit-graph
- [X] T030 Run `cargo test -p git-revwalk` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 (basic traversal first)
- Phase 3 depends on Phase 2 (ranges push to walker)
- Phase 4 depends on Phase 2 (merge-base uses traversal)
- Phase 5 depends on Phases 2+4 (accelerates both)
- Phase 6 can start after Phase 2 (independent formatting)
- Phase 7 depends on Phase 2 (object listing uses traversal)
- Phases 3, 4, 6 can run in parallel after Phase 2

### Cross-Spec Dependencies

- Spec 012 (merge) depends on: merge_base for merge operations
- Spec 014 (transport) depends on: list_objects for pack generation, have/want negotiation
- Spec 016 (porcelain) depends on: merge_base for rebase, diff
- Spec 017 (history) depends on: RevWalk + pretty-print for git log, graph for git log --graph

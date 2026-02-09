# Tasks: Large-Repo Performance Optimization

**Input**: Design documents from `/specs/022-perf-optimization/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Cargo workspace: `crates/<crate-name>/src/`
- Tests: `crates/<crate-name>/tests/` and `crates/git-cli/tests/`
- Benchmarks: `crates/<crate-name>/benches/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Capture baseline benchmarks and prepare shared infrastructure before any optimization work begins.

- [x] T001 Run baseline benchmarks and save results for regression comparison using `cargo bench -p git-cli --bench perf_compare`
- [x] T002 Add `oid_fanout_offset: usize` field to `CommitGraph` struct in `crates/git-revwalk/src/commit_graph/mod.rs`
- [x] T003 Store `oid_fanout_offset` during commit-graph parsing in `open_commit_graph()` in `crates/git-revwalk/src/commit_graph/parse.rs`
- [x] T004 [P] Add `pub fn contains(&self, oid: &ObjectId) -> bool` method to `CommitGraph` in `crates/git-revwalk/src/commit_graph/mod.rs`
- [x] T005 [P] Add `pub fn verify(&self) -> Result<(), RevWalkError>` checksum validation method to `CommitGraph` in `crates/git-revwalk/src/commit_graph/mod.rs`

**Checkpoint**: CommitGraph struct enhanced with fanout offset, contains(), and verify(). Baseline benchmarks captured.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement fanout-accelerated commit-graph lookup — this is the prerequisite for all graph-accelerated optimizations in US1, US3, and US4.

**CRITICAL**: No user story work can begin until this phase is complete.

- [x] T006 Rewrite `find_oid_position()` in `crates/git-revwalk/src/commit_graph/parse.rs` to use fanout table: read `fanout[first_byte-1]` and `fanout[first_byte]` from `oid_fanout_offset` to get (lo, hi) range, then binary search within that range only. Remove the TODO comment at line 164.
- [x] T007 Add `CommitMeta` struct (parents, tree_oid, commit_time, generation) in `crates/git-revwalk/src/walk.rs` as a lightweight alternative to full `Commit` parsing for traversal-only operations.
- [x] T008 Add `read_commit_meta()` method to `RevWalk` in `crates/git-revwalk/src/walk.rs` that checks commit-graph first via `self.commit_graph.lookup(oid)`, maps `CommitGraphEntry` to `CommitMeta`, and falls back to `read_commit()` → `CommitMeta` extraction when the OID is not in the graph.

**Checkpoint**: Foundation ready — fanout-accelerated lookups and graph-first metadata reading available for all user stories.

---

## Phase 3: User Story 1 — Fast History Browsing on Large Repos (Priority: P1)

**Goal**: Make `log` and `rev-list` on large repos within 1.2x of C git by reading commit metadata from the commit-graph instead of ODB.

**Independent Test**: Run `gitr log` and `gitr rev-list --all` on a large repo with a C git-generated commit-graph and compare wall-clock time and output byte-for-byte against C git.

### Implementation for User Story 1

- [x] T009 [US1] Modify `next_date_order()` in `crates/git-revwalk/src/walk.rs` to use `read_commit_meta()` instead of `read_commit()` for parent enumeration — avoid full commit parsing when only graph structure is needed. Construct `WalkEntry` from `CommitMeta` fields.
- [x] T010 [US1] Modify `enqueue()` in `crates/git-revwalk/src/walk.rs` to accept `CommitMeta` instead of `&Commit` — populate `generation`, `commit_date`, and `author_date` from the meta struct. Remove `#[allow(dead_code)]` from `generation` and `author_date` fields in `WalkEntry`.
- [x] T011 [US1] Implement generation-based pruning in `mark_hidden()` in `crates/git-revwalk/src/walk.rs`: track `min_generation` across all included tip commits, and in `mark_hidden()` stop walking ancestors when a commit's generation number is below the minimum generation of the hidden root (short-circuit the ancestor walk).
- [x] T012 [US1] Update `prepare_topo()` in `crates/git-revwalk/src/walk.rs` to use `read_commit_meta()` for BFS commit collection, eliminating full ODB reads during topological sort preparation.
- [x] T013 [US1] Update `next_topo()` in `crates/git-revwalk/src/walk.rs` to use `read_commit_meta()` for parent enumeration instead of `read_commit()`.
- [x] T014 [US1] Update the reverse-mode collection loop in `next_raw()` in `crates/git-revwalk/src/walk.rs` to use `read_commit_meta()` for parent enumeration.
- [x] T015 [US1] Ensure `Iterator::next()` for `RevWalk` in `crates/git-revwalk/src/walk.rs` still calls `read_commit()` (full parse) only when applying pattern filters (`passes_pattern_filter` needs author/committer strings). When no pattern filters are set, skip the full read.
- [x] T016 [US1] Add integration test in `crates/git-cli/tests/` that creates a large repo (500+ commits), generates a commit-graph with C git, runs `gitr log` and `gitr rev-list --all`, and asserts byte-identical output to `git log` and `git rev-list --all`.

**Checkpoint**: `log` and `rev-list` use commit-graph for zero-copy traversal. Expected: 42ms → ~24ms for log, 28.5ms → ~18ms for rev-list.

---

## Phase 4: User Story 5 — No Regression on Small/Medium Repos (Priority: P1)

**Goal**: Verify that Phase 3 changes do not regress performance on small or medium repos.

**Independent Test**: Run full benchmark suite on small and medium repos and compare against baseline from T001.

### Implementation for User Story 5

- [x] T017 [US5] Run `cargo bench -p git-cli --bench perf_compare` for small and medium repo sizes and verify no operation regresses more than 5% compared to T001 baseline.
- [x] T018 [US5] Add a unit test in `crates/git-revwalk/tests/` that verifies `RevWalk` produces identical commit ordering with and without a commit-graph file present (correctness invariant).
- [x] T019 [US5] Verify `read_commit_meta()` gracefully falls back to ODB when commit-graph is absent by testing on a repo with no commit-graph file in `crates/git-revwalk/tests/`.

**Checkpoint**: No regressions confirmed on small/medium repos. Commit-graph fallback verified.

---

## Phase 5: User Story 2 — Responsive Working Tree Status on Large Repos (Priority: P2)

**Goal**: Make `status` on large repos within 1.2x of C git by parallelizing filesystem stat calls.

**Independent Test**: Run `gitr status` on a large repo and compare wall-clock time and output byte-for-byte against C git.

### Implementation for User Story 2

- [x] T020 [US2] Add `rayon = { workspace = true }` dependency to `crates/git-diff/Cargo.toml`.
- [x] T021 [US2] Define a `StatResult` enum (Clean, StatMismatch with metadata, Deleted) in `crates/git-diff/src/worktree.rs` for the parallel stat phase output.
- [x] T022 [US2] Refactor `diff_index_to_worktree()` in `crates/git-diff/src/worktree.rs` into two phases: Phase A — parallel stat using `rayon::prelude::par_iter()` over `entries` to call `symlink_metadata()` and classify each entry as `StatResult::Clean`/`StatMismatch`/`Deleted`; Phase B — sequential content comparison for `StatMismatch` entries only (read content, compute diff hunks via ODB).
- [x] T023 [US2] Ensure deterministic output ordering in `diff_index_to_worktree()` in `crates/git-diff/src/worktree.rs`: collect parallel results into a `Vec` indexed by original position, then process sequentially to build `files: Vec<FileDiff>` in path order.
- [x] T024 [US2] Add integration test in `crates/git-cli/tests/` that creates a repo with 100+ files, modifies a subset, deletes some, and asserts `gitr status` output is byte-identical to `git status`.
- [x] T025 [US2] Run `cargo bench -p git-cli --bench perf_compare -- bench_status` and verify status on large repos improved. Verify small/medium repos do not regress > 5%.

**Checkpoint**: `status` parallelizes stat() calls via rayon. Expected: 52.3ms → ~33ms.

---

## Phase 6: User Story 3 — Efficient Blame on Large Files with Deep History (Priority: P3)

**Goal**: Make `blame` on large repos within 1.5x of C git by using proper diff-based attribution and cached ODB reads.

**Independent Test**: Run `gitr blame <file>` on a file with deep history and compare wall-clock time and output byte-for-byte against C git.

### Implementation for User Story 3

- [x] T026 [US3] Add `LineAttribution` enum (`Unchanged { parent_line: usize }`, `Changed`) and `diff_blame_lines()` function in `crates/git-cli/src/commands/blame.rs` that uses `git_diff::algorithm::diff_edits()` (Myers) to compute edit operations between parent and current file versions, then maps edits to `Vec<LineAttribution>`.
- [x] T027 [US3] Replace `find_changed_lines()` call in `blame_file()` in `crates/git-cli/src/commands/blame.rs` with `diff_blame_lines()` — update the blame attribution loop to use `LineAttribution::Unchanged` for inheriting blame from parent and `LineAttribution::Changed` for attributing to current commit. Maintain correct line number mapping across insertions and deletions.
- [x] T028 [US3] Switch all `repo.odb().read()` calls in `blame_file()` in `crates/git-cli/src/commands/blame.rs` to `repo.odb().read_cached()` for commit, tree, and blob reads during the history walk to leverage the existing LRU cache.
- [x] T029 [US3] Remove the old `find_changed_lines()` function from `crates/git-cli/src/commands/blame.rs` once `diff_blame_lines()` is fully integrated.
- [x] T030 [US3] Add integration test in `crates/git-cli/tests/` that creates a repo with 20+ commits modifying the same file (insertions, deletions, and moves), runs `gitr blame <file>`, and asserts byte-identical output to `git blame <file>`.
- [x] T031 [US3] Run `cargo bench -p git-cli --bench perf_compare -- bench_blame` and verify blame on large repos improved. Verify small/medium repos do not regress > 5%.

**Checkpoint**: Blame uses diff-based attribution and cached reads. Expected: 122.5ms → ~75ms (combines faster traversal from US1 + better diff + caching).

---

## Phase 7: User Story 4 — Commit-Graph Generation for Sustained Performance (Priority: P4)

**Goal**: Enable gitr to generate commit-graph files that are byte-compatible with C git's format.

**Independent Test**: Run `gitr commit-graph write`, then validate with `git commit-graph verify`. Run `git log` on the gitr-generated graph to confirm C git accepts it.

### Implementation for User Story 4

- [x] T032 [P] [US4] Create `crates/git-revwalk/src/commit_graph/write.rs` with `CommitGraphWriter` struct: `new(hash_algo)`, `add_commit(oid, tree_oid, parents, commit_time)`, and internal `CommitEntry` storage.
- [x] T033 [US4] Implement topological generation number computation in `CommitGraphWriter` in `crates/git-revwalk/src/commit_graph/write.rs`: sort commits by OID, build parent→child adjacency, compute generation numbers bottom-up (root commits = 1, others = max(parent generations) + 1).
- [x] T034 [US4] Implement `write()` method on `CommitGraphWriter` in `crates/git-revwalk/src/commit_graph/write.rs`: serialize header (CGPH, version 1, hash version, chunk count), chunk TOC (OIDF, OIDL, CDAT, EDGE offsets), OID Fanout (256 × 4-byte cumulative counts), OID Lookup (sorted OIDs), Commit Data (tree OID + parent indices + generation/date encoding), Extra Edges (octopus merge overflow), and trailing checksum.
- [x] T035 [US4] Export `CommitGraphWriter` from `crates/git-revwalk/src/commit_graph/mod.rs` and add `pub mod write;` declaration.
- [x] T036 [US4] Create `crates/git-cli/src/commands/commit_graph.rs` with `commit-graph write` subcommand: walk all refs via `repo.refs().iter(None)`, read each commit, call `writer.add_commit()`, then `writer.write()` to `.git/objects/info/commit-graph`.
- [x] T037 [US4] Add `commit-graph verify` subcommand in `crates/git-cli/src/commands/commit_graph.rs`: open commit-graph via `CommitGraph::open_from_repo()`, call `verify()`, report result.
- [x] T038 [US4] Wire `commit-graph` subcommand into CLI dispatcher in `crates/git-cli/src/main.rs` (or `crates/git-cli/src/commands/mod.rs`).
- [x] T039 [US4] Add integration test in `crates/git-cli/tests/` that creates a repo with 50+ commits (including an octopus merge), runs `gitr commit-graph write`, then asserts `git commit-graph verify` passes with zero errors.
- [x] T040 [US4] Add round-trip test in `crates/git-revwalk/tests/`: write commit-graph with `CommitGraphWriter`, read it back with `CommitGraph::open()`, verify all entries match original commit data.

**Checkpoint**: `gitr commit-graph write` and `gitr commit-graph verify` operational. C git can read gitr-generated graphs.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Index optimization, final regression validation, and documentation updates.

- [x] T041 [P] Add `memmap2 = { workspace = true }` dependency to `crates/git-index/Cargo.toml` and replace `std::fs::read(path)` with `unsafe { Mmap::map(&file) }` in `Index::read_from()` in `crates/git-index/src/lib.rs`
- [x] T042 Run full `cargo bench -p git-cli --bench perf_compare` across all repo sizes and compare against T001 baseline — verify all targets met (log ≤1.2x, status ≤1.2x, rev-list ≤1.2x, blame ≤1.5x, no regression >5% on small/medium)
- [x] T043 Run `cargo test --workspace` and `cargo clippy --workspace` to verify all tests pass and no lint warnings
- [x] T044 Update `docs/benchmark_summary.md` with post-optimization benchmark results

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (T002, T003 specifically) — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 completion (T006, T007, T008)
- **US5 (Phase 4)**: Depends on Phase 3 completion (validates US1 didn't regress)
- **US2 (Phase 5)**: Can start after Phase 2 — independent of US1
- **US3 (Phase 6)**: Benefits from US1 (faster traversal) but can start after Phase 2
- **US4 (Phase 7)**: Can start after Phase 1 (T032 is parallelizable) — independent of other stories
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Requires Foundational phase. Provides faster traversal that US3 and US5 build upon.
- **US5 (P1)**: Requires US1 complete to validate no regression. No code changes — validation only.
- **US2 (P2)**: Requires only Foundational phase. Fully independent of US1/US3/US4 (different crate: git-diff).
- **US3 (P3)**: Requires Foundational phase. Benefits from US1 (RevWalk already loads commit-graph). Should be done after US1 for maximum benefit.
- **US4 (P4)**: Requires only Phase 1 setup. Fully independent (new files: write.rs, commit_graph.rs).

### Within Each User Story

- Implementation tasks are ordered by dependency (data structures → algorithms → integration)
- Integration tests come after all implementation tasks for that story
- Benchmark validation is the final task in each story

### Parallel Opportunities

**Between stories (after Foundational):**
- US2 (git-diff/worktree.rs) and US4 (git-revwalk/write.rs + git-cli/commit_graph.rs) touch completely different files and can run in parallel with US1
- US4 T032 is explicitly marked [P] — the writer module can be started during Phase 2

**Within stories:**
- Phase 1: T004 and T005 can run in parallel (different methods, same file but independent)
- Phase 7: T032 can start in parallel with other phases (new file, no conflicts)

---

## Parallel Example: User Story 2

```
# After Foundational phase completes, US2 can start immediately:
Task T020: "Add rayon dependency to crates/git-diff/Cargo.toml"
Task T021: "Define StatResult enum in crates/git-diff/src/worktree.rs"

# Then sequentially:
Task T022: "Refactor diff_index_to_worktree() into parallel stat + sequential diff"
Task T023: "Ensure deterministic output ordering"
Task T024: "Integration test for status output parity"
Task T025: "Benchmark validation"
```

## Parallel Example: US1 + US2 + US4 Concurrent

```
# After Foundational (Phase 2) completes:
# Developer A: US1 (git-revwalk/walk.rs modifications)
# Developer B: US2 (git-diff/worktree.rs modifications)
# Developer C: US4 (new files: git-revwalk/write.rs, git-cli/commit_graph.rs)
# All three touch different files — zero merge conflicts.
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T005)
2. Complete Phase 2: Foundational (T006–T008)
3. Complete Phase 3: User Story 1 (T009–T016)
4. **STOP and VALIDATE**: Run benchmarks for log/rev-list — verify 1.2x target
5. Complete Phase 4: User Story 5 — regression validation (T017–T019)

### Incremental Delivery

1. Setup + Foundational → Graph infrastructure ready
2. Add US1 → log/rev-list optimized → Benchmark validates ~24ms log
3. Add US2 → status parallelized → Benchmark validates ~33ms status
4. Add US3 → blame optimized → Benchmark validates ~75ms blame
5. Add US4 → commit-graph write → Self-sufficient graph generation
6. Polish → Index mmap + final validation + docs update

### Parallel Team Strategy

With multiple developers after Foundational completes:

- **Developer A**: US1 (walk.rs graph traversal) → US3 (blame.rs diff attribution)
- **Developer B**: US2 (worktree.rs parallel stat) → Polish (index mmap)
- **Developer C**: US4 (commit-graph writer + CLI subcommand)

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- All output must remain byte-identical to C git — run diff comparisons after every story
- The existing `#[allow(dead_code)]` annotations on `generation` and `author_date` in walk.rs are removed as part of US1 (T010)
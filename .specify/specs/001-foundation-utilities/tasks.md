# Tasks: Foundation Utilities

**Input**: Design documents from `specs/001-foundation-utilities/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md

## Phase 1: Setup

**Purpose**: Create the git-utils crate and establish project structure

- [X] T001 Create `crates/git-utils/Cargo.toml` with dependencies: bstr, thiserror, clap, chrono, bitflags
- [X] T002 Create `crates/git-utils/src/lib.rs` with module declarations and public re-exports
- [X] T003 [P] Configure `rustfmt.toml` and `clippy.toml` at workspace root

**Checkpoint**: Crate compiles with `cargo check -p git-utils`

---

## Phase 2: Core Types (Blocking Prerequisites)

**Purpose**: Types that every other module depends on

- [X] T004 [US2] Implement error types in `crates/git-utils/src/error.rs` — define `UtilError`, `LockError` with thiserror
- [X] T005 [P] [US1] Implement byte string extensions in `crates/git-utils/src/bstring.rs` — `GitBStringExt` trait with shell_quote, c_quote, needs_quoting, rtrim_newlines
- [X] T006 [P] [US10] Implement `StringList<T>` in `crates/git-utils/src/collections/string_list.rs` — sorted/unsorted modes, insert, lookup, binary search
- [X] T007 [P] [US10] Implement `PriorityQueue<T>` in `crates/git-utils/src/collections/prio_queue.rs` — wrapper around BinaryHeap
- [X] T008 [P] [US10] Create `crates/git-utils/src/collections/mod.rs` and `hashmap.rs` — re-export std HashMap with git-specific convenience methods

**Checkpoint**: `cargo test -p git-utils` passes with unit tests for all core types

---

## Phase 3: User Story 1 - Byte String Manipulation (Priority: P1)

**Goal**: Complete byte string and path handling

**Independent Test**: Paths with non-UTF-8 bytes round-trip correctly through all operations

- [X] T009 [P] [US1] Add property-based tests for byte string operations in `crates/git-utils/tests/bstring_prop.rs`
- [X] T010 [US4] Implement `GitPath` in `crates/git-utils/src/path.rs` — new, join, dirname, basename, normalize, to_os_path, relative_to
- [X] T011 [US4] Add path compatibility tests verifying behavior matches C git for edge cases (trailing slashes, dots, empty components)

**Checkpoint**: All path operations match C git behavior

---

## Phase 4: User Story 9 - Wildmatch Pattern Matching (Priority: P2)

**Goal**: Git-compatible glob matching for gitignore and pathspecs

**Independent Test**: All C git wildmatch test vectors pass

- [X] T012 [US9] Port wildmatch algorithm from C to Rust in `crates/git-utils/src/wildmatch.rs`
- [X] T013 [US9] Import wildmatch test corpus from C git `t/` directory into `crates/git-utils/tests/wildmatch_corpus.rs`
- [X] T014 [P] [US9] Add property-based tests for wildmatch edge cases (empty patterns, deeply nested paths)

**Checkpoint**: 100% of C git wildmatch test vectors pass

---

## Phase 5: User Story 8 - Date Parsing (Priority: P2)

**Goal**: Parse and format all git date formats

**Independent Test**: Parse 100+ real date strings from git history and verify output

- [X] T015 [US8] Implement `GitDate::parse`, `parse_raw`, `parse_approxidate` in `crates/git-utils/src/date.rs`
- [X] T016 [US8] Implement `GitDate::format` for all `DateFormat` variants in `crates/git-utils/src/date.rs`
- [X] T017 [US8] Implement `Signature::parse` and `Signature::to_bytes` in `crates/git-utils/src/date.rs`
- [X] T018 [US8] Create date compatibility test suite in `crates/git-utils/tests/date_compat.rs` — compare against C git output

**Checkpoint**: Date parsing matches C git for all test vectors

---

## Phase 6: User Story 6 - Lock Files and Atomic Writes (Priority: P2)

**Goal**: RAII lock files for safe concurrent repository access

**Independent Test**: Multi-threaded stress test with concurrent lock attempts

- [X] T019 [US6] Implement `LockFile` in `crates/git-utils/src/lockfile.rs` — acquire, try_acquire, commit, rollback, Drop, Write impl
- [X] T020 [P] [US6] Implement tempfile with RAII cleanup in `crates/git-utils/src/tempfile.rs`
- [X] T021 [US6] Create concurrent lock stress test in `crates/git-utils/tests/lockfile_stress.rs`

**Checkpoint**: Lock files work correctly under concurrent access

---

## Phase 7: User Story 5 - Subprocess Execution (Priority: P2)

**Goal**: Spawn and manage external processes

**Independent Test**: Spawn echo command, capture output, verify exit code

- [X] T022 [US5] Implement `GitCommand` builder in `crates/git-utils/src/subprocess.rs` — new, arg, env, stdin/stdout/stderr modes, working_dir, timeout, run, spawn
- [X] T023 [US5] Add subprocess tests (pipe stdin/stdout, capture stderr, timeout, exit codes)

**Checkpoint**: Subprocess spawning works with all stdio modes

---

## Phase 8: User Story 3 - CLI Argument Parsing (Priority: P1)

**Goal**: Establish clap-based CLI framework matching C git conventions

**Independent Test**: Parse known C git command lines and verify identical interpretation

- [X] T024 [US3] Create CLI framework module with clap derive setup (this will be expanded by spec 015-018 as commands are added)
- [X] T025 [US3] Document C git argument conventions (--no-X, --, combined short flags) and verify clap handles them

**Checkpoint**: Framework parses basic git arguments correctly

---

## Phase 9: User Story 7 - Progress, Color, Pager (Priority: P3)

**Goal**: Terminal UX matching C git output experience

**Independent Test**: Verify color output respects --color flag and NO_COLOR environment variable

- [X] T026 [P] [US7] Implement color output in `crates/git-utils/src/color.rs` — ColorMode, Color enum, use_color, colorize
- [X] T027 [P] [US7] Implement progress display in `crates/git-utils/src/progress.rs` — Progress struct with update, tick, finish
- [X] T028 [US7] Implement pager setup in `crates/git-utils/src/pager.rs` — setup_pager respecting GIT_PAGER, core.pager, PAGER

**Checkpoint**: Terminal output matches C git visual behavior

---

## Phase 10: Polish & Cross-Cutting

- [X] T029 [P] Run `cargo clippy -p git-utils` and fix all warnings
- [X] T030 [P] Run `cargo doc -p git-utils` and verify all public items are documented
- [X] T031 Create benchmarks in `crates/git-utils/benches/path_bench.rs` for path operations and date parsing
- [X] T032 Run full test suite: `cargo test -p git-utils`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies
- **Phase 2 (Core Types)**: Depends on Phase 1
- **Phases 3-9 (User Stories)**: All depend on Phase 2; can proceed in parallel after Phase 2
- **Phase 10 (Polish)**: Depends on all prior phases

### Parallel Opportunities

- T005, T006, T007, T008 can all run in parallel (different files)
- T009, T012, T015 can run in parallel (different modules)
- T019, T020 can run in parallel (different files)
- T026, T027 can run in parallel (different files)

### Cross-Spec Dependencies

- Spec 002 (hash) depends on: error types (T004), byte string types (T005)
- Spec 003 (object model) depends on: byte strings (T005), collections (T006-T008), date/signature (T017)
- Spec 007 (index) depends on: path (T010), wildmatch (T012), lockfile (T019)
- Spec 008 (refs) depends on: lockfile (T019), path (T010)
- Spec 009 (config) depends on: path (T010), error types (T004)
- All command specs depend on: CLI framework (T024), color (T026), progress (T027), pager (T028)

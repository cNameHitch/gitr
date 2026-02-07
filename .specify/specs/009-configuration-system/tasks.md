# Tasks: Configuration System

**Input**: Design documents from `specs/009-configuration-system/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-config/Cargo.toml` with dependencies: git-utils, bstr, thiserror
- [X] T002 Create `crates/git-config/src/lib.rs` with module declarations, ConfigScope enum

**Checkpoint**: `cargo check -p git-config` compiles

---

## Phase 2: Core Parsing (Blocking)

**Purpose**: Parse config files — everything else depends on this

- [X] T003 [US3] Implement `ConfigKey` in `crates/git-config/src/lib.rs` — parse, to_canonical, matches, Display
- [X] T004 [US3] Implement config file parser in `crates/git-config/src/parse.rs` — section headers, key-value lines, comments, continuations, quoting
- [X] T005 [US3] Implement `ConfigFile` in `crates/git-config/src/file.rs` — parse, entries, get, get_all, to_bytes
- [X] T006 [US3] Implement `ConfigError` types in `crates/git-config/src/error.rs`
- [X] T007 [US3] Add parse compatibility tests in `crates/git-config/tests/parse_compat.rs` — parse real C git config files

**Checkpoint**: Config files parse correctly with all syntactic features

---

## Phase 3: User Story 1 - Read Config (Priority: P1)

**Goal**: Multi-scope config reading

- [X] T008 [US1] Implement `ConfigSet` core in `crates/git-config/src/set.rs` — new, add_file, get_string, get_all_strings
- [X] T009 [US1] Implement scope precedence logic in ConfigSet (command > worktree > local > global > system)
- [X] T010 [US1] Implement `ConfigSet::load` — discover and load config files from standard locations
- [X] T011 [US1] Add read tests verifying scope precedence with overlapping keys

**Checkpoint**: Config reading matches C git for all standard scenarios

---

## Phase 4: User Story 4 - Typed Access (Priority: P2)

**Goal**: Type-safe value interpretation

- [X] T012 [P] [US4] Implement `parse_bool` in `crates/git-config/src/types.rs`
- [X] T013 [P] [US4] Implement `parse_int` with k/m/g suffix handling
- [X] T014 [P] [US4] Implement `parse_path` with ~/ expansion
- [X] T015 [P] [US4] Implement `parse_color` and `ColorSpec`
- [X] T016 [US4] Wire typed accessors into ConfigSet: get_bool, get_int, get_usize, get_path, get_color
- [X] T017 [US4] Add type conversion tests in `crates/git-config/tests/type_conversion.rs`

**Checkpoint**: All typed accessors match C git interpretation

---

## Phase 5: User Story 2 - Write Config (Priority: P1)

**Goal**: Config modification with formatting preservation

- [X] T018 [US2] Implement `ConfigFile::set` — modify value in existing section or create new section
- [X] T019 [US2] Implement `ConfigFile::remove` and `remove_section`
- [X] T020 [US2] Implement `ConfigFile::write_to` using lock file for atomic writes
- [X] T021 [US2] Implement `ConfigSet::set` and `ConfigSet::remove` delegating to appropriate scope file
- [X] T022 [US2] Add write round-trip tests in `crates/git-config/tests/write_roundtrip.rs` — write, re-parse, verify values and formatting preserved

**Checkpoint**: Config writes are atomic, preserve formatting, and readable by C git

---

## Phase 6: User Story 5 - Includes (Priority: P2)

- [X] T023 [US5] Implement `include.path` processing in `crates/git-config/src/include.rs`
- [X] T024 [US5] Implement `includeIf` conditions: gitdir, gitdir/i, onbranch, hasconfig:remote.*.url
- [X] T025 [US5] Add circular include detection
- [X] T026 [US5] Add include tests in `crates/git-config/tests/include_tests.rs`

**Checkpoint**: Include directives work identically to C git

---

## Phase 7: User Story 6 - Environment Overrides (Priority: P2)

- [X] T027 [US6] Implement environment variable reading in `crates/git-config/src/env.rs` — GIT_CONFIG_COUNT, KEY, VALUE
- [X] T028 [US6] Implement GIT_CONFIG_NOSYSTEM, GIT_CONFIG_GLOBAL, GIT_CONFIG_SYSTEM
- [X] T029 [US6] Wire env overrides into ConfigSet::load
- [X] T030 [US6] Add environment override tests

**Checkpoint**: Environment overrides match C git behavior

---

## Phase 8: User Story 7 - Push and Remote Config Keys (Priority: P2)

- [X] T031a [US7] Implement `PushDefault` enum and `PushDefault::from_config` in `crates/git-config/src/types.rs`
- [X] T031b [US7] Implement `PushConfig` loader — read `push.default`, `push.followTags`, `push.autoSetupRemote` from ConfigSet
- [X] T031c [US7] Implement URL rewriting (`url.<base>.insteadOf`, `url.<base>.pushInsteadOf`) in `crates/git-config/src/url_rewrite.rs`
- [X] T031d [US7] Add tests for push config: verify all `push.default` values, URL rewriting, remote push refspecs
- [X] T031e [US7] Add tests verifying `branch.<name>.remote` and `branch.<name>.merge` upstream tracking config

**Checkpoint**: All push-related config keys are parsed and interpreted identically to C git

---

## Phase 9: Polish

- [X] T034 [P] Run `cargo clippy -p git-config` and fix warnings
- [X] T035 Create benchmarks in `crates/git-config/benches/config_bench.rs` — parse, lookup, typed conversion
- [X] T036 Run `cargo test -p git-config` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (sequential, core path)
- Phase 4 depends on Phase 2 (needs parsed values)
- Phase 5 depends on Phase 2 (needs ConfigFile)
- Phase 6 depends on Phase 3 (needs ConfigSet for include resolution)
- Phase 7 depends on Phase 3 (needs ConfigSet)
- Phases 4 and 5 can run in parallel after Phase 2
- T012, T013, T014, T015 can all run in parallel (different type parsers)

### Cross-Spec Dependencies

- Spec 008 (refs) depends on: config for reflog settings, ref formats
- Spec 010 (repository) depends on: ConfigSet::load, typed access for repo settings
- Spec 014 (transport) depends on: remote.*.url, http.* config values
- All command specs depend on: config for user preferences, command defaults

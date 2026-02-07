# Tasks: Repository & Setup

**Input**: Design documents from `specs/010-repository-and-setup/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-repository/Cargo.toml` with dependencies: git-utils, git-hash, git-odb, git-ref, git-config, git-index, thiserror
- [X] T002 Create `crates/git-repository/src/lib.rs` with Repository struct shell, RepoError, RepositoryKind

**Checkpoint**: `cargo check -p git-repository` compiles

---

## Phase 2: User Story 1 - Discovery (Priority: P1)

**Goal**: Find the .git directory from any location

- [X] T003 [US1] Implement `discover_git_dir` in `crates/git-repository/src/discover.rs` — walk up dirs, check for .git dir/file, bare repo detection
- [X] T004 [US1] Implement `.git` file parsing (gitdir: redirect for worktrees/submodules)
- [X] T005 [US1] Implement GIT_DIR, GIT_CEILING_DIRECTORIES environment handling in `crates/git-repository/src/env.rs`
- [X] T006 [US1] Implement `Repository::discover` using discover_git_dir
- [X] T007 [US1] Add discovery interop tests in `crates/git-repository/tests/discover_interop.rs`

**Checkpoint**: Discovery matches C git in all test cases

---

## Phase 3: User Story 2 - Initialization (Priority: P1)

- [X] T008 [US2] Implement `init_repository` in `crates/git-repository/src/init.rs` — create .git/ structure (HEAD, objects/, refs/, config)
- [X] T009 [US2] Implement bare repository initialization
- [X] T010 [US2] Implement template directory copying
- [X] T011 [US2] Implement `Repository::init` and `init_bare`
- [X] T012 [US2] Add init interop tests in `crates/git-repository/tests/init_interop.rs`

**Checkpoint**: Initialized repos are usable by C git

---

## Phase 4: User Story 3 - Repository Struct (Priority: P1)

**Goal**: Wire all subsystems together

- [X] T013 [US3] Implement `Repository::open` — open ODB, refs, config from git_dir
- [X] T014 [US3] Implement lazy index loading via OnceCell
- [X] T015 [US3] Implement all accessors: odb(), refs(), config(), index(), git_dir(), work_tree()
- [X] T016 [US3] Implement convenience methods: head_oid(), current_branch(), is_unborn()
- [X] T017 [US3] Add repository open tests

**Checkpoint**: Repository struct provides access to all subsystems

---

## Phase 5: User Story 5 - Environment Variables (Priority: P2)

- [X] T018 [US5] Implement environment variable handling in `crates/git-repository/src/env.rs` — GIT_WORK_TREE, GIT_OBJECT_DIRECTORY, GIT_ALTERNATE_OBJECT_DIRECTORIES, GIT_INDEX_FILE
- [X] T019 [US5] Wire env vars into Repository::open and discover
- [X] T020 [US5] Add environment variable tests

**Checkpoint**: All standard env vars are respected

---

## Phase 6: User Story 4 - Worktrees (Priority: P2)

- [X] T021 [US4] Implement worktree detection in `crates/git-repository/src/worktree.rs` — detect linked worktree, find commondir
- [X] T022 [US4] Implement commondir resolution (objects, refs from common; HEAD, index from worktree)
- [X] T023 [US4] Add worktree tests in `crates/git-repository/tests/worktree_tests.rs`

**Checkpoint**: Linked worktrees open correctly

---

## Phase 7: Polish

- [X] T024 [P] Run `cargo clippy -p git-repository` and fix warnings
- [X] T025 Run `cargo test -p git-repository` — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 → Phase 3 (sequential, discovery first)
- Phase 4 depends on Phase 2 (needs discovery for open)
- Phase 5 depends on Phase 2 (env vars affect discovery)
- Phase 6 depends on Phase 4 (needs Repository struct)
- Phases 3 and 5 can start in parallel after Phase 2

### Cross-Spec Dependencies

- Spec 011 (diff) depends on: Repository for accessing ODB and index
- Spec 012 (merge) depends on: Repository for all subsystems
- Spec 013 (rev-walk) depends on: Repository for ODB and refs
- All command specs (015-018) depend on: Repository as the entry point

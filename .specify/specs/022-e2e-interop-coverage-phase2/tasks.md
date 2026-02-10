# Tasks: Comprehensive E2E Interop Test Coverage

**Input**: Design documents from `/specs/021-e2e-interop-coverage/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, quickstart.md

**Tests**: This feature IS tests. All tasks produce test code.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- All test files: `crates/git-cli/tests/`
- Common harness: `crates/git-cli/tests/common/mod.rs`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Extend the common test harness with helpers needed by multiple user stories

- [ ] T001 Add `git_stdin(dir, args, stdin_bytes)` and `gitr_stdin(dir, args, stdin_bytes)` helper functions that pipe `&[u8]` to process stdin via `Stdio::piped()`, returning `CommandResult` with pinned env vars, in `crates/git-cli/tests/common/mod.rs`
- [ ] T002 Add `git_stdin_with_date(dir, args, stdin_bytes, epoch)` and `gitr_stdin_with_date(dir, args, stdin_bytes, epoch)` helpers with date override support in `crates/git-cli/tests/common/mod.rs`
- [ ] T003 Add `setup_untracked_files(dir)` helper that creates a committed repo with 2 tracked files, 2 untracked files, 1 untracked directory with a file, and a `.gitignore` with 1 ignored file present on disk, in `crates/git-cli/tests/common/mod.rs`
- [ ] T004 Add `setup_submodule_repo(dir, sub_dir)` helper that creates a bare repo at `sub_dir` with 2 commits, then clones it as a submodule inside `dir` using `file://` URL and commits the `.gitmodules`, in `crates/git-cli/tests/common/mod.rs`
- [ ] T005 Add `setup_large_repo(dir, commits, branches, files)` helper that creates a repo with the specified number of sequential commits (incrementing dates), branches forked from various points, and files spread across nested directories, in `crates/git-cli/tests/common/mod.rs`

**Checkpoint**: All new helpers compile. Run `cargo test --test e2e_workflow_tests` to verify common/mod.rs still works with existing tests.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Create the 4 new test file skeletons so user story tasks can add tests to them

**No blocking prerequisites beyond Phase 1** — each test file is independent.

- [ ] T006 [P] Create test file skeleton `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs` with `mod common; use common::*;` header and module doc comment describing scope (clean, submodule, worktree, am/format-patch)
- [ ] T007 [P] Create test file skeleton `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs` with `mod common; use common::*; use std::process::Command;` header and module doc comment describing scope (mktag, mktree, commit-tree, pack/index/verify, update-index/ref, check-attr/ignore)
- [ ] T008 [P] Create test file skeleton `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs` with `mod common; use common::*;` header and module doc comment describing scope (bundle, archive, notes, replace)
- [ ] T009 [P] Create test file skeleton `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs` with `mod common; use common::*; use std::process::Command;` header and module doc comment describing scope (prune, fast-import, hooks, large repos, config scoping)

**Checkpoint**: `cargo test --test e2e_porcelain_coverage_tests --test e2e_plumbing_coverage_tests --test e2e_bundle_archive_notes_tests --test e2e_maintenance_hooks_scale_tests` compiles with 0 tests.

---

## Phase 3: User Story 1 — Porcelain Command Interop Coverage (Priority: P1) MVP

**Goal**: E2e interop tests for `clean`, `submodule`, `worktree`, and `am`/`format-patch` — the highest-impact untested porcelain commands.

**Independent Test**: `cargo test --test e2e_porcelain_coverage_tests` — all tests pass.

### Clean Tests (5 tests)

- [ ] T010 [P] [US1] Implement `test_clean_dry_run`: setup untracked files in both dirs, run `clean -n` with git and gitr, `assert_output_eq` on listed files, verify no files actually removed, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T011 [P] [US1] Implement `test_clean_force`: setup untracked files, run `clean -f` with both tools, compare output and verify identical files remain on disk, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T012 [P] [US1] Implement `test_clean_force_dirs`: setup untracked files+dirs, run `clean -fd` with both tools, compare output and verify untracked dirs also removed, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T013 [P] [US1] Implement `test_clean_ignored`: setup with `.gitignore` and ignored files on disk, run `clean -fx` with both tools, compare output and verify ignored files removed, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T014 [P] [US1] Implement `test_clean_no_untracked`: setup repo with only tracked files, run `clean -f` with both tools, verify identical empty output and exit code 0, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`

### Submodule Tests (7 tests)

- [ ] T015 [US1] Implement `test_submodule_add_init_update`: create bare remote, run `submodule add <file-url>` then `submodule init` then `submodule update` with both tools in separate repos, compare `submodule status` output and `.gitmodules` content, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T016 [US1] Implement `test_submodule_status`: after setting up submodule repos, run `submodule status` with both tools and `assert_output_eq`, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T017 [US1] Implement `test_submodule_sync`: modify submodule URL in `.gitmodules`, run `submodule sync` with both tools, compare `config --get submodule.*.url` output, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T018 [US1] Implement `test_submodule_deinit`: setup submodule, run `submodule deinit <name>` with both tools, compare output and verify submodule working tree removed, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T019 [US1] Implement `test_submodule_foreach`: setup submodule, run `submodule foreach 'echo $name $path'` with both tools, `assert_output_eq`, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T020 [US1] Implement `test_submodule_cross_tool`: setup submodule with gitr, verify C git can `submodule status` and `fsck` the repo; then setup with C git, verify gitr can read it, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T021 [US1] Implement `test_submodule_nested`: create submodule that itself contains a submodule, run `submodule update --init --recursive` with both tools, compare resulting directory structure and `submodule status --recursive`, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`

### Worktree Tests (5 tests)

- [ ] T022 [P] [US1] Implement `test_worktree_add_list`: run `worktree add ../wt feature-branch` then `worktree list` with both tools, compare list output format, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T023 [P] [US1] Implement `test_worktree_remove`: add a worktree then `worktree remove` it with both tools, compare output and verify directory removed, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T024 [P] [US1] Implement `test_worktree_prune`: add a worktree, manually delete its directory, run `worktree prune` with both tools, compare output and verify stale entry removed, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T025 [P] [US1] Implement `test_worktree_detach`: run `worktree add --detach ../wt HEAD` with both tools, verify detached HEAD state matches, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T026 [US1] Implement `test_worktree_cross_tool`: create worktree with gitr, verify C git `worktree list` sees it and commits in the worktree are valid; reverse direction, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`

### Am / Format-Patch Tests (5 tests)

- [ ] T027 [P] [US1] Implement `test_format_patch_single`: setup 3-commit repo, run `format-patch -1 HEAD` with both tools, compare generated `.patch` file content (ignoring message-id), in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T028 [P] [US1] Implement `test_format_patch_range`: run `format-patch HEAD~2..HEAD` with both tools, compare number of files generated and their content, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T029 [US1] Implement `test_am_apply_patch`: generate patch with C git, apply with gitr `am` and verify resulting commit matches; then generate with gitr, apply with C git, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T030 [US1] Implement `test_format_patch_am_roundtrip`: gitr `format-patch` -> C git `am` -> compare repo state; then C git `format-patch` -> gitr `am` -> compare repo state, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`
- [ ] T031 [US1] Implement `test_am_three_way`: create patch that conflicts, apply with `am --three-way` with both tools, compare exit codes and conflict markers, in `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs`

**Checkpoint**: `cargo test --test e2e_porcelain_coverage_tests` — 22 tests pass, 0 failures, 0 ignored. All repos pass `fsck`.

---

## Phase 4: User Story 2 — Plumbing Command Interop Coverage (Priority: P2)

**Goal**: E2e interop tests for all untested plumbing commands (mktag, mktree, commit-tree, pack-objects, index-pack, update-index, update-ref, check-attr, check-ignore, verify-pack).

**Independent Test**: `cargo test --test e2e_plumbing_coverage_tests` — all tests pass.

### Object Creation (6 tests)

- [ ] T032 [P] [US2] Implement `test_mktag_from_stdin`: create a commit, construct valid tag content as bytes, pipe to `mktag` via `git_stdin`/`gitr_stdin`, compare returned OID, verify with `cat-file`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T033 [P] [US2] Implement `test_mktag_invalid_target`: pipe tag content referencing nonexistent OID to `mktag`, compare error exit codes, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T034 [P] [US2] Implement `test_mktree_from_stdin`: get `ls-tree` output from a commit, pipe to `mktree` with both tools, compare returned tree OID, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T035 [P] [US2] Implement `test_mktree_missing_flag`: pipe tree entry with nonexistent blob OID to `mktree --missing` with both tools, compare OIDs (should succeed), in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T036 [P] [US2] Implement `test_commit_tree_basic`: get tree OID via `write-tree`, run `commit-tree <tree> -m "msg"` with both tools, compare resulting commit content via `cat-file -p`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T037 [P] [US2] Implement `test_commit_tree_with_parents`: create 2 commits, get tree OID, run `commit-tree <tree> -p <parent1> -p <parent2> -m "merge"` with both tools, verify merge commit parents via `cat-file -p`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`

### Pack Operations (5 tests)

- [ ] T038 [P] [US2] Implement `test_pack_objects_stdout`: pipe OIDs to `pack-objects --stdout` with both tools, verify both produce valid packs by piping to `index-pack --stdin`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T039 [P] [US2] Implement `test_pack_objects_revs`: pipe `--all` revisions to `pack-objects --revs` with both tools, compare object count in output, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T040 [US2] Implement `test_pack_objects_roundtrip`: create pack with gitr `pack-objects`, build index with C git `index-pack`, verify all objects readable; then reverse, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T041 [P] [US2] Implement `test_index_pack_verify`: take a pack from `gc`, run `index-pack` with both tools on a copy, compare resulting `.idx` file sizes and `verify-pack` output, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T042 [P] [US2] Implement `test_verify_pack_verbose`: run `verify-pack -v` on same pack with both tools, compare object listing (OID, type, size columns), in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`

### Index & Ref Operations (6 tests)

- [ ] T043 [P] [US2] Implement `test_update_index_add`: create file, run `update-index --add <file>` with both tools, compare `ls-files --stage` output, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T044 [P] [US2] Implement `test_update_index_cacheinfo`: hash a blob, run `update-index --cacheinfo 100644,<oid>,<name>` with both tools, compare `ls-files --stage`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T045 [P] [US2] Implement `test_update_index_remove`: stage a file then `update-index --force-remove <file>` with both tools, compare `ls-files` (should be empty), in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T046 [P] [US2] Implement `test_update_ref_create`: run `update-ref refs/heads/new-branch <oid>` with both tools, compare `show-ref` output, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T047 [P] [US2] Implement `test_update_ref_delete`: create ref then `update-ref -d refs/heads/new-branch` with both tools, compare `show-ref` output (ref gone), in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T048 [US2] Implement `test_update_ref_stdin_transaction`: pipe `create refs/test/a <oid>\nupdate refs/test/b <oid> 0\n` to `update-ref --stdin` with both tools, compare `show-ref`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`

### Attribute & Ignore (3 tests)

- [ ] T049 [P] [US2] Implement `test_check_attr_output`: create `.gitattributes` with `*.txt text` and `*.bin binary`, run `check-attr -a file.txt file.bin` with both tools, `assert_output_eq`, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T050 [P] [US2] Implement `test_check_ignore_output`: create `.gitignore` with `*.log` and `build/`, run `check-ignore test.log build/out src/main.rs` with both tools, compare output and exit codes, in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`
- [ ] T051 [P] [US2] Implement `test_check_ignore_verbose`: run `check-ignore -v test.log` with both tools, compare verbose output (source file, line number, pattern, pathname), in `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs`

**Checkpoint**: `cargo test --test e2e_plumbing_coverage_tests` — 20 tests pass, 0 failures, 0 ignored.

---

## Phase 5: User Story 3 — Bundle, Archive & Notes Interop (Priority: P2)

**Goal**: E2e interop tests for bundle, archive, notes, and replace commands.

**Independent Test**: `cargo test --test e2e_bundle_archive_notes_tests` — all tests pass.

### Bundle Tests (4 tests)

- [ ] T052 [P] [US3] Implement `test_bundle_create_verify`: setup 3-commit repo, run `bundle create repo.bundle --all` then `bundle verify repo.bundle` with both tools, compare verify output (refs listed), in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T053 [P] [US3] Implement `test_bundle_gitr_create_git_unbundle`: create bundle with gitr, clone from bundle with C git, verify `log --oneline` matches original, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T054 [P] [US3] Implement `test_bundle_git_create_gitr_unbundle`: create bundle with C git, clone from bundle with gitr, verify `log --oneline` matches original, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T055 [P] [US3] Implement `test_bundle_list_heads`: create bundle, run `bundle list-heads` with both tools, `assert_output_eq` on listed refs, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`

### Archive Tests (4 tests)

- [ ] T056 [P] [US3] Implement `test_archive_tar`: setup repo with 3 files, run `archive --format=tar HEAD -o out.tar` with both tools, extract both tars with `tar -tf`, compare file listings, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T057 [P] [US3] Implement `test_archive_zip`: run `archive --format=zip HEAD -o out.zip` with both tools, list contents with `unzip -l`, compare file listings, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T058 [P] [US3] Implement `test_archive_prefix`: run `archive --format=tar --prefix=project/ HEAD` with both tools, extract and compare file paths (all should start with `project/`), in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T059 [US3] Implement `test_archive_cross_tool`: create tar with gitr, extract with `tar xf`; create tar with C git, extract with `tar xf`; compare extracted file contents byte-for-byte, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`

### Notes Tests (5 tests)

- [ ] T060 [P] [US3] Implement `test_notes_add_show`: add note with `notes add -m "note text" HEAD` with both tools, run `notes show HEAD`, `assert_output_eq`, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T061 [P] [US3] Implement `test_notes_list`: add notes to 3 commits, run `notes list` with both tools, `assert_output_eq` on OID listing, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T062 [P] [US3] Implement `test_notes_remove`: add note then `notes remove HEAD` with both tools, verify `notes show HEAD` returns error exit code, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T063 [P] [US3] Implement `test_notes_append`: add note, then `notes append -m "more text" HEAD` with both tools, compare `notes show HEAD` output (both lines present), in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T064 [US3] Implement `test_notes_cross_tool`: add note with gitr, read with C git `notes show`; add note with C git, read with gitr; verify both directions work, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`

### Replace Tests (3 tests)

- [ ] T065 [P] [US3] Implement `test_replace_object`: create 2 commits, run `replace <commit1> <commit2>` with both tools, verify `cat-file -p <commit1>` now shows commit2 content, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T066 [P] [US3] Implement `test_replace_delete`: create replacement, run `replace -d <commit1>` with both tools, verify `cat-file -p <commit1>` reverts to original, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`
- [ ] T067 [US3] Implement `test_replace_cross_tool`: create replacement with gitr, verify C git honors it (`log` shows replaced content); reverse direction, in `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs`

**Checkpoint**: `cargo test --test e2e_bundle_archive_notes_tests` — 16 tests pass, 0 failures, 0 ignored.

---

## Phase 6: User Story 4 — Maintenance & Integrity Interop (Priority: P2)

**Goal**: E2e interop tests for standalone `prune` and `fast-import`.

**Independent Test**: `cargo test --test e2e_maintenance_hooks_scale_tests -- prune` and `cargo test --test e2e_maintenance_hooks_scale_tests -- fast_import` — all pass.

### Prune Tests (3 tests)

- [ ] T068 [P] [US4] Implement `test_prune_unreachable`: create commits, reset HEAD to orphan earlier objects, run `prune` with both tools, compare remaining objects in `.git/objects`, `assert_fsck_clean`, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T069 [P] [US4] Implement `test_prune_dry_run`: create unreachable objects, run `prune -n` with both tools, compare listed objects, verify objects still exist on disk, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T070 [P] [US4] Implement `test_prune_preserves_reachable`: create repo where all objects are reachable, run `prune` with both tools, verify zero objects removed and `fsck` passes, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`

### Fast-Import Tests (3 tests)

- [ ] T071 [P] [US4] Implement `test_fast_import_basic`: construct a fast-import stream with 1 blob + 1 commit, pipe to `fast-import` with both tools via `git_stdin`/`gitr_stdin`, compare `log --oneline` and `cat-file -p HEAD`, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T072 [US4] Implement `test_fast_import_cross_tool`: import with gitr, verify C git `fsck` and `log` match; import with C git, verify gitr reads correctly, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T073 [P] [US4] Implement `test_fast_import_marks`: run `fast-import --export-marks=marks.txt` with both tools using identical stream with marks, compare marks files line-by-line, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`

**Checkpoint**: `cargo test --test e2e_maintenance_hooks_scale_tests -- "prune\|fast_import"` — 6 tests pass.

---

## Phase 7: User Story 5 — Hook Execution Interop (Priority: P3)

**Goal**: E2e tests verifying gitr executes hooks at the same trigger points as C git.

**Independent Test**: `cargo test --test e2e_maintenance_hooks_scale_tests -- hook` — all pass.

- [ ] T074 [P] [US5] Implement `test_hook_pre_commit_fires`: install executable `pre-commit` hook that creates marker file, run `commit` with both tools, verify marker file exists in both repos, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T075 [P] [US5] Implement `test_hook_pre_commit_blocks`: install `pre-commit` hook that exits 1, run `commit` with both tools, `assert_exit_code_eq` (both should fail), verify no commit created, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T076 [P] [US5] Implement `test_hook_post_commit_fires`: install `post-commit` hook that writes marker file, run `commit` with both tools, verify marker exists and commit succeeded, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T077 [US5] Implement `test_hook_commit_msg`: install `commit-msg` hook that appends "[hook]" to the message file argument, run `commit` with both tools, compare `log --format=%B -1` to verify hook modified the message, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`

**Checkpoint**: `cargo test --test e2e_maintenance_hooks_scale_tests -- hook` — 4 tests pass.

---

## Phase 8: User Story 6 — Large Repository Scalability (Priority: P3)

**Goal**: E2e tests that verify gitr handles large repos without diverging from C git.

**Independent Test**: `cargo test --test e2e_maintenance_hooks_scale_tests -- large_repo` — all pass.

- [ ] T078 [P] [US6] Implement `test_large_repo_log`: use `setup_large_repo(dir, 150, 0, 1)` for both repos, run `log --oneline`, `rev-list --count HEAD`, and `fsck` with both tools, `assert_output_eq` for each, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T079 [P] [US6] Implement `test_large_repo_many_branches`: use `setup_large_repo(dir, 10, 50, 1)`, run `branch --list` and `for-each-ref refs/heads/` with both tools, `assert_output_eq`, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T080 [P] [US6] Implement `test_large_repo_many_files`: use `setup_large_repo(dir, 1, 0, 500)`, run `ls-files`, `ls-tree -r HEAD`, and `status` with both tools, `assert_output_eq` for each, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`

**Checkpoint**: `cargo test --test e2e_maintenance_hooks_scale_tests -- large_repo` — 3 tests pass.

---

## Phase 9: User Story 7 — Config Scoping Interop (Priority: P3)

**Goal**: E2e tests that verify gitr config scoping matches C git precedence rules.

**Independent Test**: `cargo test --test e2e_maintenance_hooks_scale_tests -- config` — all pass.

- [ ] T081 [P] [US7] Implement `test_config_local_overrides_global`: set `user.name` in fake global config (via HOME) and local `.git/config` to different values, run `config --get user.name` with both tools, verify local wins, in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`
- [ ] T082 [US7] Implement `test_config_list_show_origin`: set multiple config values at different scopes, run `config --list --show-origin` with both tools, compare output format (file paths and values), in `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs`

**Checkpoint**: `cargo test --test e2e_maintenance_hooks_scale_tests -- config` — 2 tests pass.

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Final validation and cleanup

- [ ] T083 Run `cargo test` (full suite) and verify all 1280+ tests pass with 0 failures and 0 ignored
- [ ] T084 Run `cargo clippy --tests` on all new test files and fix any warnings
- [ ] T085 Verify total test suite runtime is under 60 seconds
- [ ] T086 Run quickstart.md validation: execute the verification commands and confirm expected output

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 (helpers must exist before test files import them)
- **Phases 3-9 (User Stories)**: All depend on Phase 2 (file skeletons must exist)
  - US1-US7 are independent of each other and can run in parallel
- **Phase 10 (Polish)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (Porcelain)**: Depends on T003 (setup_untracked_files), T004 (setup_submodule_repo)
- **US2 (Plumbing)**: Depends on T001, T002 (stdin helpers)
- **US3 (Bundle/Archive/Notes)**: No special helper dependencies beyond Phase 2
- **US4 (Maintenance)**: Depends on T001, T002 (stdin helpers for fast-import)
- **US5 (Hooks)**: No special helper dependencies
- **US6 (Scale)**: Depends on T005 (setup_large_repo)
- **US7 (Config)**: No special helper dependencies

### Within Each User Story

All tests marked [P] within a story can be written in parallel (different test functions, same file, no shared mutable state).

### Parallel Opportunities

- T006-T009 (file skeletons) can all be created in parallel
- Within US1: clean tests (T010-T014) are all parallel; worktree tests (T022-T025) are all parallel
- Within US2: all object creation tests (T032-T037) parallel; all index/ref tests (T043-T048) parallel
- Within US3: bundle tests (T052-T055) parallel; notes tests (T060-T063) parallel
- US1 through US7 can be developed in parallel once Phase 2 completes

---

## Parallel Example: User Story 1

```bash
# Phase 3 — All clean tests in parallel:
Task: "T010 test_clean_dry_run in e2e_porcelain_coverage_tests.rs"
Task: "T011 test_clean_force in e2e_porcelain_coverage_tests.rs"
Task: "T012 test_clean_force_dirs in e2e_porcelain_coverage_tests.rs"
Task: "T013 test_clean_ignored in e2e_porcelain_coverage_tests.rs"
Task: "T014 test_clean_no_untracked in e2e_porcelain_coverage_tests.rs"

# All worktree tests in parallel:
Task: "T022 test_worktree_add_list in e2e_porcelain_coverage_tests.rs"
Task: "T023 test_worktree_remove in e2e_porcelain_coverage_tests.rs"
Task: "T024 test_worktree_prune in e2e_porcelain_coverage_tests.rs"
Task: "T025 test_worktree_detach in e2e_porcelain_coverage_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T005)
2. Complete Phase 2: File skeletons (T006-T009)
3. Complete Phase 3: User Story 1 — Porcelain tests (T010-T031)
4. **STOP and VALIDATE**: `cargo test --test e2e_porcelain_coverage_tests` — 22 tests pass
5. This alone adds coverage for 4 major commands (clean, submodule, worktree, am/format-patch)

### Incremental Delivery

1. Setup + Foundational → Harness ready
2. Add US1 (Porcelain) → 22 tests → Validates 4 commands
3. Add US2 (Plumbing) → +20 tests → Validates 10 commands
4. Add US3 (Bundle/Archive/Notes) → +16 tests → Validates 4 commands
5. Add US4 (Maintenance) → +6 tests → Validates 2 commands
6. Add US5-US7 (Hooks/Scale/Config) → +9 tests → Validates cross-cutting concerns
7. Polish → Full validation → Ship

---

## Notes

- [P] tasks = different test functions, no shared state, safe to write in parallel
- [Story] label maps task to specific user story for traceability
- Every test function follows the dual-repo pattern: setup identical repos, run command with both tools, compare outputs
- All repos must pass `git fsck --full` after test operations
- Deterministic dates via `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE` env vars (handled by common helpers)
- Total: **86 tasks** (5 setup + 4 foundational + 73 test implementations + 4 polish)

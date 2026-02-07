# Tasks: End-to-End Git Interoperability Tests

**Input**: Design documents from `/specs/019-e2e-interop-tests/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Create the shared test harness module that all test files depend on.

- [x] T001 Create `CommandResult` struct and `gitr_bin()` path discovery function in `crates/git-cli/tests/common/mod.rs`. The struct must have `stdout: String`, `stderr: String`, `exit_code: i32` fields. `gitr_bin()` discovers the compiled gitr binary from cargo's target directory using `std::env::current_exe()`.
- [x] T002 Implement `git(dir, args) -> CommandResult` function in `crates/git-cli/tests/common/mod.rs`. Must pin all 11 environment variables: GIT_AUTHOR_NAME, GIT_AUTHOR_EMAIL, GIT_AUTHOR_DATE, GIT_COMMITTER_NAME, GIT_COMMITTER_EMAIL, GIT_COMMITTER_DATE, TZ=UTC, LC_ALL=C, LANG=C, GIT_CONFIG_NOSYSTEM=1, HOME=(set to dir's parent). Execute via `std::process::Command` and return `CommandResult`.
- [x] T003 Implement `gitr(dir, args) -> CommandResult` function in `crates/git-cli/tests/common/mod.rs`. Same environment pinning as `git()`, but executes the gitr binary from `gitr_bin()`. Also implement `git_with_date(dir, args, epoch)` and `gitr_with_date(dir, args, epoch)` variants that override GIT_AUTHOR_DATE and GIT_COMMITTER_DATE for multi-commit scenarios. Implement `next_date(counter: &mut u64) -> String` returning `"(1234567890 + counter) +0000"`.
- [x] T004 Implement assertion helpers in `crates/git-cli/tests/common/mod.rs`: `assert_output_eq(git: &CommandResult, gitr: &CommandResult)` — asserts stdout and exit_code match with diff-style error message; `assert_stdout_eq(git: &CommandResult, gitr: &CommandResult)` — asserts only stdout matches; `assert_exit_code_eq(git: &CommandResult, gitr: &CommandResult)` — asserts only exit codes match; `fsck(dir: &Path) -> CommandResult` — runs `git fsck --full`; `assert_fsck_clean(dir: &Path)` — runs fsck and asserts exit_code==0.
- [x] T005 Implement `assert_repo_state_eq(dir_a, dir_b)` in `crates/git-cli/tests/common/mod.rs`. Must compare: (1) HEAD ref value by reading `.git/HEAD`, (2) all refs under `.git/refs/` recursively, (3) the set of loose object IDs in `.git/objects/`. Panic with details of the first divergence found.
- [x] T006 Implement repo setup helpers in `crates/git-cli/tests/common/mod.rs`: `setup_empty_repo(dir)` — `git init -b main` + config user.name/email; `setup_linear_history(dir, n)` — empty repo + N commits each adding/modifying a file with deterministic content and incrementing dates; `setup_branched_history(dir)` — linear 3 commits on main, then `git checkout -b feature` from commit 2 with 2 divergent commits modifying different files; `setup_merge_conflict(dir)` — like branched but both branches modify the same lines of `conflict.txt`.
- [x] T007 Implement additional repo setup helpers in `crates/git-cli/tests/common/mod.rs`: `setup_bare_remote(dir)` — `git init --bare -b main` then populate with 2 commits via a temp working clone that pushes; `setup_binary_files(dir)` — repo with a file containing bytes `[0x89, 0x50, 0x4E, 0x47, ...]` (PNG-like header + 256 random-seeded bytes); `setup_unicode_paths(dir)` — repo with files `café.txt`, `naïve.txt`, `日本語.txt`; `setup_nested_dirs(dir)` — repo with `a/b/c/d/e/f/g/h/i/j/file.txt` (10 levels deep).

**Checkpoint**: Shared harness complete. Run `cargo test -p git-cli --test plumbing_tests -- --list` to verify it compiles (no tests yet depend on common, but the module should compile).

---

## Phase 2: Foundational (Refactor Existing Tests)

**Purpose**: Migrate existing test files to use the shared harness, eliminating duplication and ensuring consistency.

**CRITICAL**: No new e2e test files can be written until this phase confirms zero regressions.

- [x] T008 Refactor `crates/git-cli/tests/plumbing_tests.rs` to use shared harness. Add `mod common;` and `use common::*;` at top. Remove the duplicated `gitr_bin()`, `git()`, `gitr()`, and `setup_test_repo()` functions (~55 lines). Update all test functions to use the `CommandResult` struct — replace `let (actual, code) = gitr(...)` with `let result = gitr(...)` and access `result.stdout`/`result.exit_code`. Ensure all 27 existing tests still pass.
- [x] T009 [P] Refactor `crates/git-cli/tests/porcelain_tests.rs` to use shared harness. Add `mod common;` and `use common::*;` at top. Remove duplicated `gitr_bin()`, `git()`, `gitr()`, `gitr_full()`, and `setup_test_repo()` functions (~74 lines). Update all test functions to use `CommandResult`. Ensure all 29 existing tests still pass.
- [x] T010 [P] Refactor `crates/git-cli/tests/history_tests.rs` to use shared harness. Add `mod common;` and `use common::*;` at top. Remove duplicated `gitr_bin()`, `git()`, `gitr()`, `gitr_full()`, and `setup_history_repo()` functions (~62 lines). Update all test functions to use `CommandResult`. Ensure all 19+ existing tests still pass.
- [x] T011 Run `cargo test -p git-cli` and verify the total test count is unchanged (75 tests). All tests must pass with zero regressions. If any test fails, fix the harness compatibility issue before proceeding.

**Checkpoint**: All existing tests pass using shared harness. `cargo test -p git-cli` shows same pass count as before.

---

## Phase 3: User Story 1 — Basic Workflow Interop (Priority: P1) MVP

**Goal**: Verify that gitr handles the init→add→commit→status→diff cycle identically to C git.

**Independent Test**: Run `cargo test -p git-cli --test e2e_workflow_tests -- basic` — all basic workflow tests pass.

### Implementation for User Story 1

- [x] T012 [US1] Create test file `crates/git-cli/tests/e2e_workflow_tests.rs` with module imports (`mod common; use common::*; use tempfile;`). Implement `test_init_add_commit_status_cycle`: create two tempdirs; in dir_git run C git init→add hello.txt→commit; in dir_gitr run gitr init→add hello.txt→commit; compare status output, log output, and repo state (HEAD, refs, objects) using `assert_output_eq` and `assert_repo_state_eq`.
- [x] T013 [P] [US1] Implement `test_gitr_init_cgit_operates` in `crates/git-cli/tests/e2e_workflow_tests.rs`: gitr inits repo and commits 2 files; then C git runs add (new file), commit, and log on that same repo; assert C git exits 0 and log shows all 2 commits.
- [x] T014 [P] [US1] Implement `test_cgit_init_gitr_operates` in `crates/git-cli/tests/e2e_workflow_tests.rs`: C git inits repo and commits 2 files; then gitr runs add (new file), commit, and log on that same repo; assert gitr exits 0 and log shows all 2 commits.
- [x] T015 [P] [US1] Implement `test_status_identical_output` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup linear history (C git); run status with both tools on clean repo (assert_output_eq); add untracked file; run status again (assert_output_eq); modify tracked file; run status again (assert_output_eq).
- [x] T016 [P] [US1] Implement `test_diff_staged_and_unstaged` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup linear history; modify a file (unstaged); compare `diff` output; stage the file; compare `diff --cached` output; compare `diff HEAD` output. All three must match between gitr and C git.
- [x] T017 [P] [US1] Implement `test_multiple_commits_sequential` in `crates/git-cli/tests/e2e_workflow_tests.rs`: create two parallel repos; in each, perform 5 sequential commits (using incrementing dates via `next_date`), each modifying a different file. After all 5, compare `log --oneline` and `rev-list HEAD` output. Compare HEAD OID and all ref values.
- [x] T018 [P] [US1] Implement `test_add_multiple_files_and_directories` in `crates/git-cli/tests/e2e_workflow_tests.rs`: create nested directory structure (src/main.rs, src/lib/utils.rs, docs/readme.txt); add all with `add .`; compare `ls-files --stage` output between gitr and C git.
- [x] T019 [P] [US1] Implement `test_commit_message_formats` in `crates/git-cli/tests/e2e_workflow_tests.rs`: test 3 commit message styles — (1) single line, (2) multi-line with blank line separator, (3) message with special characters (quotes, newlines, unicode). Compare `log --format=%B` output for each.

**Checkpoint**: `cargo test -p git-cli --test e2e_workflow_tests -- basic` passes. Basic workflow interop verified.

---

## Phase 4: User Story 2 — Branching and Merging Interop (Priority: P1)

**Goal**: Verify that branch lifecycle and merge operations (ff, 3-way, conflict) produce identical results.

**Independent Test**: Run `cargo test -p git-cli --test e2e_workflow_tests -- branch` and `-- merge` — all branching/merging tests pass.

### Implementation for User Story 2

- [x] T020 [US2] Implement `test_branch_create_list_delete_cycle` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup linear history in two parallel repos; create branch "feature" (compare `branch` list output); delete it (compare `branch -d` output and `show-ref`); verify branch is gone in both.
- [x] T021 [P] [US2] Implement `test_fast_forward_merge` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup linear history; create branch, add commit on branch, switch to main, merge branch (ff). Compare merge output, `log --oneline`, and ref values between gitr and C git repos.
- [x] T022 [P] [US2] Implement `test_three_way_merge_no_conflict` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup branched history (divergent changes to different files); merge feature into main. Compare merge output, resulting tree (`ls-tree -r HEAD`), and `log --oneline --graph` between repos.
- [x] T023 [P] [US2] Implement `test_merge_conflict_markers` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup merge conflict scenario; attempt merge in both repos. Assert exit code matches (non-zero). Compare conflict markers in the working tree file byte-for-byte. Compare `ls-files --stage` output (should show stage 1/2/3 entries).
- [x] T024 [P] [US2] Implement `test_checkout_and_switch_equivalence` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup branched history; run gitr `checkout feature` and compare HEAD/working tree with C git `checkout feature`. Also test gitr `switch feature` produces same result.
- [x] T025 [P] [US2] Implement `test_merge_commit_message` in `crates/git-cli/tests/e2e_workflow_tests.rs`: perform a three-way merge; compare the auto-generated merge commit message via `log -1 --format=%B HEAD` between gitr and C git.
- [x] T026 [P] [US2] Implement `test_branch_rename` in `crates/git-cli/tests/e2e_workflow_tests.rs`: create branch "old-name", rename to "new-name" via `branch -m`; compare `show-ref` output and verify old ref is gone and new ref exists in both repos.
- [x] T027 [P] [US2] Implement `test_detached_head_operations` in `crates/git-cli/tests/e2e_workflow_tests.rs`: checkout a specific commit OID (detached HEAD); compare `rev-parse HEAD`, `symbolic-ref HEAD` (should fail), and `status` output between gitr and C git.

**Checkpoint**: `cargo test -p git-cli --test e2e_workflow_tests` passes (all US1 + US2 tests). Branching/merging interop verified.

---

## Phase 5: User Story 8 — Cross-Tool Repository Compatibility (Priority: P1)

**Goal**: Verify repos are freely interchangeable between gitr and C git with no corruption.

**Independent Test**: Run `cargo test -p git-cli --test e2e_workflow_tests -- cross_tool` — all cross-tool tests pass.

### Implementation for User Story 8

- [x] T028 [US8] Implement `test_gitr_repo_passes_cgit_fsck` in `crates/git-cli/tests/e2e_workflow_tests.rs`: gitr runs init, adds 3 files, makes 3 commits (incrementing dates). Then run C git `fsck --full` via `assert_fsck_clean`. Also run C git `log --oneline` and verify it shows 3 commits.
- [x] T029 [P] [US8] Implement `test_cgit_repo_passes_gitr_fsck` in `crates/git-cli/tests/e2e_workflow_tests.rs`: C git creates repo with 3 commits. Then run gitr `fsck` and assert exit code 0. Also run gitr `log --oneline` and verify 3 commits.
- [x] T030 [P] [US8] Implement `test_alternating_commits_ping_pong` in `crates/git-cli/tests/e2e_workflow_tests.rs`: gitr inits repo and makes commit 1; C git makes commit 2 (adding different file); gitr makes commit 3; C git makes commit 4. After each commit, run `fsck` with the other tool. Finally compare `log --oneline` from both tools — must be identical.
- [x] T031 [P] [US8] Implement `test_mixed_history_log_identical` in `crates/git-cli/tests/e2e_workflow_tests.rs`: build a repo with 5 commits alternating between gitr and C git. Run `log --format="%H %s"` with both tools. Output must be byte-identical.
- [x] T032 [P] [US8] Implement `test_gitr_gc_cgit_fsck` in `crates/git-cli/tests/e2e_workflow_tests.rs`: C git creates repo with 10+ commits (enough loose objects for gc to pack). Gitr runs `gc`. C git runs `fsck --full` — must pass. C git runs `log --oneline` — must show all commits.
- [x] T033 [P] [US8] Implement `test_cgit_gc_gitr_operates` in `crates/git-cli/tests/e2e_workflow_tests.rs`: gitr creates repo with 10+ commits. C git runs `gc`. Gitr runs `log --oneline` — must show all commits. Gitr runs `cat-file -p HEAD` — must succeed.

**Checkpoint**: `cargo test -p git-cli --test e2e_workflow_tests` passes (all US1 + US2 + US8 tests). Cross-tool compatibility verified. MVP complete.

---

## Phase 6: User Story 3 — History Inspection Interop (Priority: P2)

**Goal**: Verify log/show/blame produce byte-identical output against the same repo state.

**Independent Test**: Run `cargo test -p git-cli --test e2e_workflow_tests -- history` — tests pass. (Note: these are added to the workflow test file since they're closely related.)

### Implementation for User Story 3

- [x] T034 [US3] Implement `test_log_format_flags_interop` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup repo with 5+ commits (including a merge) using C git. Run the following with both tools and assert_output_eq for each: `log`, `log --oneline`, `log --format="%H %ae %s"`, `log --stat`, `log --graph --all --oneline`.
- [x] T035 [P] [US3] Implement `test_blame_interop` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup repo where file.txt has 3 lines added across 3 different commits (distinct author dates). Run `blame file.txt` with both tools. Assert_output_eq — OIDs, author, date, and line content must match.
- [x] T036 [P] [US3] Implement `test_show_commit_interop` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup linear history; get HEAD OID. Run `show HEAD`, `show --no-patch HEAD`, `show HEAD:hello.txt` with both tools. Assert_output_eq for each.
- [x] T037 [P] [US3] Implement `test_rev_list_interop` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup repo with merge history. Run `rev-list HEAD`, `rev-list --count HEAD`, `rev-list --reverse HEAD` with both tools. Assert_output_eq for each.

**Checkpoint**: History inspection interop verified.

---

## Phase 7: User Story 4 — Plumbing Command Interop (Priority: P2)

**Goal**: Verify plumbing commands produce identical scripting-compatible output.

**Independent Test**: Existing `plumbing_tests.rs` already covers much of this. These new tests add deeper interop coverage (format strings, all object types, complex rev expressions).

### Implementation for User Story 4

- [x] T038 [US4] Implement `test_cat_file_all_object_types` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup repo with a commit, tree, blob, and annotated tag. Run `cat-file -t`, `cat-file -s`, `cat-file -p` on each object OID with both tools. Assert_output_eq for all 12 comparisons (4 objects x 3 flags).
- [x] T039 [P] [US4] Implement `test_for_each_ref_format_strings` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup repo with 2 branches and 2 tags. Run `for-each-ref --format="%(refname) %(objectname) %(objecttype)"` with both tools. Assert_output_eq. Also test `for-each-ref --sort=-creatordate`.
- [x] T040 [P] [US4] Implement `test_rev_parse_complex_expressions` in `crates/git-cli/tests/e2e_workflow_tests.rs`: setup repo with 5 commits. Run `rev-parse` on: `HEAD`, `HEAD~2`, `HEAD^{tree}`, `main`, `HEAD~3^{commit}`. Assert_output_eq for each expression.

**Checkpoint**: Plumbing interop verified.

---

## Phase 8: User Story 5 — Remote Operations Interop (Priority: P2)

**Goal**: Verify clone/fetch/push/pull work correctly between gitr and C git over local file:// transport.

**Independent Test**: Run `cargo test -p git-cli --test e2e_remote_tests` — all tests pass.

### Implementation for User Story 5

- [x] T041 [US5] Create test file `crates/git-cli/tests/e2e_remote_tests.rs` with module imports. Implement `test_gitr_clone_matches_cgit_clone`: setup bare remote (C git, 2 commits); clone with C git into dir_git; clone with gitr into dir_gitr. Compare `log --oneline` output, `show-ref` output, and working tree file contents between the two clones.
- [x] T042 [P] [US5] Implement `test_gitr_clone_bare` in `crates/git-cli/tests/e2e_remote_tests.rs`: setup bare remote; clone --bare with both tools. Compare `show-ref` output and verify bare repo structure (HEAD, objects/, refs/ at root, no working tree).
- [x] T043 [P] [US5] Implement `test_clone_preserves_all_refs` in `crates/git-cli/tests/e2e_remote_tests.rs`: setup bare remote with 2 branches and 1 tag. Clone with gitr. Verify all remote-tracking refs and tags exist via `show-ref`. Compare with C git clone.
- [x] T044 [P] [US5] Implement `test_clone_remote_config` in `crates/git-cli/tests/e2e_remote_tests.rs`: clone with both tools. Compare `config --get remote.origin.url` and `config --get remote.origin.fetch` output.
- [x] T045 [US5] Implement `test_gitr_push_cgit_fetch` in `crates/git-cli/tests/e2e_remote_tests.rs`: setup bare remote; gitr clones; gitr creates a new commit and pushes. C git clones the same bare remote fresh. Verify the new commit is present in C git's clone via `log --oneline`.
- [x] T046 [P] [US5] Implement `test_cgit_push_gitr_fetch` in `crates/git-cli/tests/e2e_remote_tests.rs`: setup bare remote; both tools clone. C git creates new commit and pushes. Gitr runs `fetch origin` then `merge origin/main`. Verify gitr's log matches C git's updated log.
- [x] T047 [P] [US5] Implement `test_push_new_branch` in `crates/git-cli/tests/e2e_remote_tests.rs`: gitr clones bare remote; creates branch "feature" with 1 commit; pushes with `push origin feature`. C git clones fresh and verifies `show-ref` includes `refs/heads/feature`.
- [x] T048 [P] [US5] Implement `test_fetch_new_commits` in `crates/git-cli/tests/e2e_remote_tests.rs`: setup bare remote; gitr clones. C git pushes 2 new commits to bare. Gitr runs `fetch origin`. Compare `rev-list origin/main` output from gitr with C git — must show same OIDs.
- [x] T049 [P] [US5] Implement `test_pull_fast_forward` in `crates/git-cli/tests/e2e_remote_tests.rs`: setup bare remote; gitr clones. C git pushes 1 new commit. Gitr runs `pull origin main`. Compare `log --oneline` and HEAD with C git's state.

**Checkpoint**: `cargo test -p git-cli --test e2e_remote_tests` passes. Remote operations interop verified.

---

## Phase 9: User Story 6 — Advanced Operations Interop (Priority: P3)

**Goal**: Verify rebase, stash, cherry-pick, annotated tags, and gc/repack work compatibly.

**Independent Test**: Run `cargo test -p git-cli --test e2e_advanced_tests` — all tests pass.

### Implementation for User Story 6

- [x] T050 [US6] Create test file `crates/git-cli/tests/e2e_advanced_tests.rs` with module imports. Implement `test_rebase_linear`: setup branched history in two parallel repos; in each, rebase feature onto main. Compare `log --oneline` and `ls-tree -r HEAD` between repos.
- [x] T051 [P] [US6] Implement `test_rebase_conflict_abort` in `crates/git-cli/tests/e2e_advanced_tests.rs`: setup merge conflict scenario in two repos; attempt rebase in each; assert exit code matches (non-zero); run `rebase --abort` in each; verify clean state via `status` comparison.
- [x] T052 [P] [US6] Implement `test_rebase_onto` in `crates/git-cli/tests/e2e_advanced_tests.rs`: setup 3-branch scenario (main, feature-a, feature-b where b branches from a); rebase --onto main feature-a feature-b in both repos. Compare `log --oneline` output.
- [x] T053 [P] [US6] Implement `test_stash_push_pop_roundtrip` in `crates/git-cli/tests/e2e_advanced_tests.rs`: setup linear history in two repos; modify a file (unstaged); run `stash push` in both; verify working tree is clean; run `stash pop` in both; verify modification is back. Compare all outputs and `stash list`.
- [x] T054 [P] [US6] Implement `test_stash_list_output` in `crates/git-cli/tests/e2e_advanced_tests.rs`: create 3 stashes in two parallel repos (each modifying different files). Compare `stash list` output between repos.
- [x] T055 [P] [US6] Implement `test_stash_with_untracked` in `crates/git-cli/tests/e2e_advanced_tests.rs`: add an untracked file; run `stash push --include-untracked` in both repos. Verify untracked file is gone. Run `stash pop`. Verify file is back. Compare behavior.
- [x] T056 [P] [US6] Implement `test_cherry_pick_output_matches` in `crates/git-cli/tests/e2e_advanced_tests.rs`: setup branched history; cherry-pick the last commit from feature onto main in both repos. Compare `log --oneline` and working tree content.
- [x] T057 [P] [US6] Implement `test_revert_output_matches` in `crates/git-cli/tests/e2e_advanced_tests.rs`: setup linear history (3 commits); revert HEAD in both repos. Compare `log --oneline`, `diff HEAD~1`, and working tree.
- [x] T058 [P] [US6] Implement `test_cherry_pick_conflict` in `crates/git-cli/tests/e2e_advanced_tests.rs`: setup scenario where cherry-pick causes a conflict; attempt in both repos. Assert exit codes match (non-zero). Compare conflict markers in working tree.
- [x] T059 [P] [US6] Implement `test_annotated_tag_interop` in `crates/git-cli/tests/e2e_advanced_tests.rs`: create annotated tag `v1.0` with message "Release 1.0" in both repos (on same commit OID). Compare `cat-file -p` on the tag object. Compare `tag -l` output. Compare `describe` output.
- [x] T060 [P] [US6] Implement `test_tag_list_output_matches` in `crates/git-cli/tests/e2e_advanced_tests.rs`: create 3 tags (mix of lightweight and annotated) in both repos. Compare `tag` (list) and `tag -n` output.
- [x] T061 [P] [US6] Implement `test_tag_verify_cross_tool` in `crates/git-cli/tests/e2e_advanced_tests.rs`: gitr creates annotated tag; C git runs `describe` against that tag's commit. Verify C git's describe output references the tag name.
- [x] T062 [P] [US6] Implement `test_gc_repack_packfile_compat` in `crates/git-cli/tests/e2e_advanced_tests.rs`: C git creates repo with 10+ commits. Gitr runs `gc`. C git runs `fsck --full` — assert clean. C git runs `log --oneline` — verify all commits present.
- [x] T063 [P] [US6] Implement `test_fsck_after_repack` in `crates/git-cli/tests/e2e_advanced_tests.rs`: C git creates repo with 10+ commits. Gitr runs `repack -a -d`. C git runs `fsck --full` — assert clean.

**Checkpoint**: `cargo test -p git-cli --test e2e_advanced_tests` passes. Advanced operations interop verified.

---

## Phase 10: User Story 7 — Edge Case Interop (Priority: P3)

**Goal**: Verify gitr handles binary files, unicode paths, empty repos, and deep nesting identically to C git.

**Independent Test**: Run `cargo test -p git-cli --test e2e_edge_case_tests` — all tests pass.

### Implementation for User Story 7

- [x] T064 [US7] Create test file `crates/git-cli/tests/e2e_edge_case_tests.rs` with module imports. Implement `test_binary_file_add_commit_show`: use `setup_binary_files` helper; run `show HEAD` with both tools; compare output (should include "Binary files" marker). Run `cat-file -p <blob_oid>` with both tools — blob content must match byte-for-byte.
- [x] T065 [P] [US7] Implement `test_binary_diff_markers` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: setup repo with binary file committed; modify the binary file; run `diff` with both tools. Assert both show "Binary files ... differ" message. Compare exit codes.
- [x] T066 [P] [US7] Implement `test_binary_cat_file_identical` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: setup repo with binary file; get blob OID; run `cat-file -p <oid>` with both tools. Compare raw stdout bytes — must be identical.
- [x] T067 [P] [US7] Implement `test_empty_repo_status` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: `setup_empty_repo` (no commits). Run `status` with both tools. Compare output and exit code. Should show "No commits yet" or similar.
- [x] T068 [P] [US7] Implement `test_empty_repo_log` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: `setup_empty_repo`. Run `log` with both tools. Compare stderr (should show error about no commits) and exit code.
- [x] T069 [P] [US7] Implement `test_empty_commit` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: setup linear history in two repos; run `commit --allow-empty -m "empty"` with both tools. Compare `log --oneline` — should show the empty commit.
- [x] T070 [P] [US7] Implement `test_unicode_filename_roundtrip` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: use `setup_unicode_paths` helper; compare `ls-files` output between repos. Run `status` and compare. Verify each unicode filename appears correctly.
- [x] T071 [P] [US7] Implement `test_space_in_filename` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: create repo with file "hello world.txt" and "path with spaces/file.txt". Add and commit with both tools. Compare `ls-files` and `ls-tree -r HEAD` output.
- [x] T072 [P] [US7] Implement `test_special_chars_in_path` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: create repo with files containing `'`, `"`, `(`, `)`, `[`, `]` in names. Add and commit with both tools. Compare `ls-files` output.
- [x] T073 [P] [US7] Implement `test_deeply_nested_dirs` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: use `setup_nested_dirs` helper; compare `ls-tree -r HEAD` output between repos. Verify all 10 levels appear correctly.
- [x] T074 [P] [US7] Implement `test_many_files_in_directory` in `crates/git-cli/tests/e2e_edge_case_tests.rs`: create repo with 100 files (`file_000.txt` through `file_099.txt`) in a single directory. Add and commit with both tools. Compare `ls-files` output — file ordering must match.

**Checkpoint**: `cargo test -p git-cli --test e2e_edge_case_tests` passes. Edge case interop verified.

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: Final validation, CI integration, and cleanup.

- [x] T075 Run full test suite `cargo test --workspace` and verify all tests pass (existing + new). Record total test count — should be 75 (existing) + 56+ (new) = 131+ tests.
- [x] T076 [P] Verify CI compatibility by checking that `.github/workflows/test.yml` runs `cargo test --workspace` which will automatically include the new test files. No CI config changes should be needed.
- [x] T077 [P] Run `cargo clippy -p git-cli --tests` and fix any warnings in the new test files and shared harness.
- [x] T078 Mark any tests that fail due to known incomplete gitr command implementations with `#[ignore]` and add a comment explaining which functionality is missing (e.g., `// TODO: gitr gc does not yet produce valid packfiles`).

**Checkpoint**: Full suite green. CI ready. All known gaps documented with `#[ignore]`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1)**: Depends on Phase 2
- **Phase 4 (US2)**: Depends on Phase 2 (can parallel with Phase 3)
- **Phase 5 (US8)**: Depends on Phase 2 (can parallel with Phase 3/4)
- **Phase 6 (US3)**: Depends on Phase 2 (can parallel with Phase 3/4/5)
- **Phase 7 (US4)**: Depends on Phase 2 (can parallel with Phase 3-6)
- **Phase 8 (US5)**: Depends on Phase 2 (can parallel with Phase 3-7)
- **Phase 9 (US6)**: Depends on Phase 2 (can parallel with Phase 3-8)
- **Phase 10 (US7)**: Depends on Phase 2 (can parallel with Phase 3-9)
- **Phase 11 (Polish)**: Depends on all user story phases

### User Story Dependencies

- **US1 (Basic Workflow)**: No dependencies on other stories
- **US2 (Branching/Merging)**: No dependencies on other stories
- **US8 (Cross-Tool Compat)**: No dependencies on other stories
- **US3 (History Inspection)**: No dependencies on other stories
- **US4 (Plumbing Commands)**: No dependencies on other stories
- **US5 (Remote Operations)**: No dependencies on other stories
- **US6 (Advanced Operations)**: No dependencies on other stories
- **US7 (Edge Cases)**: No dependencies on other stories

All user stories share only the common harness (Phase 1+2) and can be implemented in any order or in parallel.

### Within Each User Story

1. Create test file with module imports (if new file)
2. Implement tests marked [P] in parallel
3. Run `cargo test -p git-cli --test <test_file>` to verify
4. Story complete

### Parallel Opportunities

**Phase 1** (sequential — each function builds on previous):
- T001 → T002 → T003 → T004 → T005 → T006 → T007

**Phase 2** (T009 and T010 parallel after T008 establishes pattern):
- T008, then T009 ‖ T010, then T011

**Phases 3-10** (all user stories parallel after Phase 2):
- US1 ‖ US2 ‖ US8 ‖ US3 ‖ US4 ‖ US5 ‖ US6 ‖ US7

**Within each user story** (most tasks parallel — different test functions, same file):
- e.g., Phase 3: T012 first (creates file), then T013 ‖ T014 ‖ T015 ‖ T016 ‖ T017 ‖ T018 ‖ T019

---

## Parallel Example: User Story 1

```text
# First, create the file (T012):
T012: Create e2e_workflow_tests.rs + test_init_add_commit_status_cycle

# Then launch remaining US1 tests in parallel (all different functions in same file):
T013: test_gitr_init_cgit_operates
T014: test_cgit_init_gitr_operates
T015: test_status_identical_output
T016: test_diff_staged_and_unstaged
T017: test_multiple_commits_sequential
T018: test_add_multiple_files_and_directories
T019: test_commit_message_formats
```

## Parallel Example: User Story 5

```text
# First, create the file (T041):
T041: Create e2e_remote_tests.rs + test_gitr_clone_matches_cgit_clone

# Then launch remaining US5 tests in parallel:
T042: test_gitr_clone_bare
T043: test_clone_preserves_all_refs
T044: test_clone_remote_config
T045: test_gitr_push_cgit_fetch
T046: test_cgit_push_gitr_fetch
T047: test_push_new_branch
T048: test_fetch_new_commits
T049: test_pull_fast_forward
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 + 8)

1. Complete Phase 1: Setup (shared harness)
2. Complete Phase 2: Foundational (refactor existing tests)
3. Complete Phase 3: US1 — Basic Workflow
4. Complete Phase 4: US2 — Branching/Merging
5. Complete Phase 5: US8 — Cross-Tool Compatibility
6. **STOP and VALIDATE**: Run `cargo test -p git-cli --test e2e_workflow_tests` — all P1 tests pass
7. This alone delivers significant compatibility confidence

### Incremental Delivery

1. Setup + Foundational → Harness ready, existing tests migrated
2. Add US1 + US2 + US8 → Core workflow interop verified (MVP!)
3. Add US3 + US4 → History and plumbing interop verified
4. Add US5 → Remote operations interop verified
5. Add US6 + US7 → Advanced ops and edge cases verified
6. Each increment adds coverage without breaking previous tests

---

## Notes

- [P] tasks = different functions in same file, no dependencies between them
- [Story] label maps each task to its user story for traceability
- All user stories are independently testable after Phase 2 (shared harness)
- Tests that fail due to incomplete gitr implementations should be `#[ignore]`d, not deleted
- Commit after each phase or logical group of tasks
- Stop at any checkpoint to validate independently
- Total: **78 tasks**, **56+ new test functions**, **4 new test files**, **1 shared harness module**

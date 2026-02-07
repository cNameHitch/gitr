# Tasks: Git Command Parity

**Input**: Design documents from `/specs/020-git-parity/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Interop tests are a primary deliverable of this spec (SC-001). Tests are written alongside implementation as each story depends on verifying byte-identical output against C git.

**Organization**: Tasks grouped by user story. P1 stories share foundational infrastructure (date format, path quoting) but are otherwise independent.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Path Conventions

- **Workspace**: `crates/<crate>/src/` for library code, `crates/git-cli/src/commands/` for CLI commands
- **Tests**: `crates/git-cli/tests/` for interop tests
- **Shared**: `crates/git-utils/src/` for cross-crate utilities

---

## Phase 1: Setup

**Purpose**: Shared infrastructure changes that multiple user stories depend on

- [X] T001 Add `DateFormat::Default` variant to `crates/git-utils/src/date.rs` — format as `"%a %b %e %H:%M:%S %Y %z"` using commit's stored tz_offset (NOT local time), matching C git's default date output (R1 in research.md)
- [X] T002 Change `FormatOptions::default()` in `crates/git-revwalk/src/pretty.rs` to use `DateFormat::Default` instead of `DateFormat::Iso` (line 39)
- [X] T003 [P] Add `quote_path(path: &[u8]) -> String` function to `crates/git-utils/src/path.rs` — octal-escape non-ASCII bytes (>127) and non-printable bytes within double quotes, escape backslash and double-quote, pass through printable ASCII unchanged (data-model.md §6)

**Checkpoint**: `cargo test -p git-utils` and `cargo test -p git-revwalk` pass. Date format and path quoting utilities available for all commands.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Packfile reading fix that MUST be complete before remote operations (US5) and affects packfile interop (US4)

**⚠️ CRITICAL**: US4 (packfile) and US5 (remote ops) depend on this phase

- [X] T004 Add cross-pack REF_DELTA resolver to `crates/git-pack/src/pack.rs` — add `read_object_with_resolver(&self, oid, resolver: impl Fn(&ObjectId) -> Option<(ObjectType, Vec<u8>)>)` method that falls back to the resolver callback when a REF_DELTA base is not found within the current pack (R3 in research.md)
- [X] T005 Wire the cross-pack resolver in `crates/git-odb/src/lib.rs` — when reading from a PackFile, pass a resolver closure that searches other packs and loose objects for REF_DELTA bases

**Checkpoint**: `cargo test -p git-pack` and `cargo test -p git-odb` pass. Objects can be read from packfiles with cross-pack delta references.

---

## Phase 3: User Story 1 — Merge Parity (Priority: P1) 🎯 MVP

**Goal**: Fast-forward merges advance refs, three-way merges produce correct trees with two-parent merge commits, conflicts produce correct exit codes and markers.

**Independent Test**: Create branches with divergent histories, merge with both gitr and C git, compare refs/tree/exit codes/conflict markers.

### Implementation for User Story 1

- [X] T006 [US1] Fix merge output messages in `crates/git-cli/src/commands/merge.rs` — fast-forward: emit `"Updating <short-old>..<short-new>\nFast-forward\n"`, three-way: emit `"Merge made by the 'ort' strategy.\n"`, conflict: emit `"CONFLICT (content): Merge conflict in <file>\nAutomatic merge failed; fix conflicts and then commit the result.\n"` (contract §1)
- [X] T007 [US1] Fix merge exit code in `crates/git-cli/src/commands/merge.rs` — return exit code 1 on conflict (not 0 or other), exit code 0 on clean merge (contract §1c)
- [X] T008 [US1] Verify conflict marker format in `crates/git-merge/src/content.rs` — ensure markers match C git exactly: `<<<<<<< HEAD`, `=======`, `>>>>>>> <branch-name>` with 7 angle brackets (R5 in research.md)
- [X] T009 [US1] Verify three-way merge tree completeness in `crates/git-merge/src/strategy/ort.rs` — ensure result tree includes files from both branches, not just base+diffs; verify working tree checkout writes all files after merge
- [X] T010 [US1] Write merge parity interop tests in `crates/git-cli/tests/parity_tests.rs` — 4 tests matching acceptance scenarios: (1) FF merge ref advance, (2) three-way clean merge with two parents and full tree, (3) conflict exit code and markers, (4) merge commit parent lines and message. Use `setup_branched_history()` and `setup_merge_conflict()` from `common/mod.rs`.

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes merge tests. Merge behavior matches C git.

---

## Phase 4: User Story 2 — Diff Hunk Content (Priority: P1)

**Goal**: `gitr diff`, `diff --cached`, and `diff HEAD` produce full unified diff output with hunk headers and content lines, byte-identical to C git.

**Independent Test**: Modify tracked files (staged/unstaged), compare diff output between gitr and C git.

### Implementation for User Story 2

- [X] T011 [US2] Investigate and fix diff blob content reading in `crates/git-cli/src/commands/diff.rs` — the unified formatter is correct (R2), so verify the diff command correctly reads blob content from ODB (including packfiles) and passes full file content to the diff engine, not just headers
- [X] T012 [US2] Verify diff format passthrough in `crates/git-cli/src/commands/diff.rs` — ensure `format_diff()` result including hunk content (`@@` lines, `+`/`-` lines) is written to stdout without truncation (contract §2)
- [X] T013 [US2] Write diff parity interop tests in `crates/git-cli/tests/parity_tests.rs` — 3 tests matching acceptance scenarios: (1) unstaged diff with hunk content, (2) `diff --cached` with hunk content, (3) `diff HEAD` with hunk content. Compare byte-for-byte with C git output.

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes diff tests. Full unified diff output matches C git.

---

## Phase 5: User Story 3 — Output Format Parity (Priority: P1)

**Goal**: All output formatting commands produce byte-identical output to C git: dates, log format newlines, blame format, detached HEAD status, empty repo exit code, unicode path escaping.

**Independent Test**: Run each command with known repo states, compare output byte-for-byte with C git.

### Implementation for User Story 3

- [X] T014 [P] [US3] Fix log date format in `crates/git-cli/src/commands/log.rs` — ensure default format uses `DateFormat::Default` from FormatOptions (should be automatic after T002), verify `--date=` flag parsing maps to correct DateFormat variants
- [X] T015 [P] [US3] Fix `log --format=` newline handling in `crates/git-cli/src/commands/log.rs` — ensure each `--format=` entry is followed by a newline separator, fix `saw_separator` logic (line ~124) to emit `\n` between entries matching C git (R6, FR-006)
- [X] T016 [P] [US3] Fix empty repo `log` exit code in `crates/git-cli/src/commands/log.rs` — detect unborn branch when `walker.push_head()` fails, emit `"fatal: your current branch '<name>' does not have any commits yet"` to stderr, exit with code 128 (R9, FR-009)
- [X] T017 [P] [US3] Fix show date format in `crates/git-cli/src/commands/show.rs` — use `DateFormat::Default` for commit display and tag tagger dates, not hardcoded `DateFormat::Iso` (R1)
- [X] T018 [P] [US3] Fix blame date+time format in `crates/git-cli/src/commands/blame.rs` — change from ISO date truncated to 10 chars (YYYY-MM-DD) to full `"YYYY-MM-DD HH:MM:SS <tz>"` format matching C git's default blame output (R7, FR-007)
- [X] T019 [P] [US3] Fix detached HEAD status display in `crates/git-cli/src/commands/status.rs` — change `"HEAD detached"` (line ~113) to `"HEAD detached at <short-oid>"` where short-oid is the 7-char abbreviated commit hash (R8, FR-008)
- [X] T020 [P] [US3] Fix unicode path escaping in `crates/git-cli/src/commands/ls_files.rs` — use `quote_path()` from T003 for path output when not using `-z` flag; with `-z` flag output raw bytes (R10, FR-010)
- [X] T021 [US3] Write output format parity interop tests in `crates/git-cli/tests/parity_tests.rs` — 7 tests matching acceptance scenarios: (1) log default date format, (2) log --format=%s newlines, (3) show date format, (4) blame date+time+OID format, (5) detached HEAD status with OID, (6) empty repo log exit 128, (7) ls-files unicode escaping. Use `setup_linear_history()`, `setup_unicode_paths()` from `common/mod.rs`.

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes all output format tests. All formatting matches C git.

---

## Phase 6: User Story 4 — Packfile Reading (Priority: P1)

**Goal**: gitr reads objects from packfiles produced by C git `gc`, including OFS_DELTA and REF_DELTA resolution.

**Independent Test**: Create repo with gitr, run C git `gc`, verify gitr can still `log`, `cat-file`, `show` all objects.

### Implementation for User Story 4

- [X] T022 [US4] Verify ODB pack discovery in `crates/git-odb/src/lib.rs` — ensure ODB scans `.git/objects/pack/*.idx` on first read and discovers all packfiles, including packs created by C git `gc`
- [X] T023 [US4] Verify delta chain resolution in `crates/git-pack/src/pack.rs` — test that multi-level OFS_DELTA chains (common after `gc`) resolve correctly, verify MAX_DELTA_CHAIN_DEPTH=512 is sufficient
- [X] T024 [US4] Write packfile interop tests in `crates/git-cli/tests/parity_tests.rs` — 3 tests matching acceptance scenarios: (1) 12 commits, gc, log --oneline shows all, (2) cat-file -p HEAD after gc matches pre-gc output, (3) OFS_DELTA/REF_DELTA resolution after gc

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes packfile tests. gitr fully operates on gc'd repos.

---

## Phase 7: User Story 5 — Remote Operations (Priority: P2)

**Goal**: clone/push/fetch/pull work over file:// protocol with byte-identical results to C git.

**Independent Test**: Set up bare repos, clone with gitr, push commits, verify with C git that objects and refs transferred correctly.

**Depends on**: Phase 2 (packfile fix) and Phase 3 (merge for pull)

### Implementation for User Story 5

- [X] T025 [US5] Fix file:// URL handling in `crates/git-transport/src/local.rs` — verify `file:///path/to/repo` correctly strips scheme and resolves to local path, test with both `file://` and bare path formats
- [X] T026 [US5] Fix clone remote config in `crates/git-cli/src/commands/clone.rs` — verify `remote.origin.url` and `remote.origin.fetch` are written correctly after clone, matching C git values (contract §10, FR-012)
- [X] T027 [US5] Fix clone --bare in `crates/git-cli/src/commands/clone.rs` — verify bare clone produces correct structure (HEAD, refs/, objects/, no .git wrapper, no working tree), matching C git (FR-013)
- [X] T028 [US5] Fix push over file:// in `crates/git-cli/src/commands/push.rs` — verify objects are transferred and remote refs updated correctly when pushing to a file:// bare repo (FR-014)
- [X] T029 [US5] Fix fetch over file:// in `crates/git-cli/src/commands/fetch.rs` — verify remote-tracking refs updated and objects received when fetching from file:// remote (FR-015)
- [X] T030 [US5] Fix pull integration in `crates/git-cli/src/commands/pull.rs` — verify pull performs fetch followed by fast-forward merge, producing identical state to C git (FR-016)
- [X] T031 [US5] Write remote operations interop tests in `crates/git-cli/tests/parity_tests.rs` — 7 tests matching acceptance scenarios: (1) clone file:// with correct refs/tree/config, (2) clone --bare structure, (3) push new commits visible to C git, (4) fetch + merge matches C git, (5) pull fast-forward matches C git, (6) push feature branch, (7) remote config values match. Use `setup_bare_remote()` from `common/mod.rs`.

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes remote tests. Full clone-push-fetch-pull cycle works between gitr and C git.

---

## Phase 8: User Story 6 — Stash Operations (Priority: P2)

**Goal**: stash push/pop/list work with multiple entries, proper working tree capture, and --include-untracked support.

**Independent Test**: Create dirty working trees, stash changes, verify clean state, pop, compare results with C git.

### Implementation for User Story 6

- [X] T032 [US6] Fix stash push to use reflog in `crates/git-cli/src/commands/stash.rs` — change from overwriting `refs/stash` to appending a reflog entry, capture actual working tree state (not just index), create proper 2-parent stash commit structure (FR-017, R12)
- [X] T033 [US6] Fix stash pop to restore full working tree in `crates/git-cli/src/commands/stash.rs` — read stash commit tree and restore to working tree (not just index), drop the reflog entry after successful restore (FR-018)
- [X] T034 [US6] Fix stash list to show all entries in `crates/git-cli/src/commands/stash.rs` — read all reflog entries for `refs/stash`, format as `stash@{N}: On <branch>: <message>` (FR-019)
- [X] T035 [US6] Implement stash push --include-untracked in `crates/git-cli/src/commands/stash.rs` — collect untracked files into a tree, create a 3rd parent commit containing them, remove untracked files from working tree (FR-020, data-model.md §4)
- [X] T036 [US6] Write stash interop tests in `crates/git-cli/tests/parity_tests.rs` — 3 tests matching acceptance scenarios: (1) push/pop roundtrip with status match, (2) 3-stash list format match, (3) --include-untracked roundtrip

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes stash tests. Stash roundtrip matches C git.

---

## Phase 9: User Story 7 — Plumbing Command Parity (Priority: P2)

**Goal**: for-each-ref excludes HEAD, rev-parse supports ^{type} peeling syntax.

**Independent Test**: Run plumbing commands against known repos, compare output with C git.

### Implementation for User Story 7

- [X] T037 [P] [US7] Fix for-each-ref HEAD exclusion in `crates/git-cli/src/commands/for_each_ref.rs` — filter out HEAD from ref iteration results, only include refs under `refs/` namespace (FR-021, R13)
- [X] T038 [P] [US7] Add ^{type} peeling to revision parser in `crates/git-revwalk/src/range.rs` — extend `split_revision_suffix()` to parse `^{tree}`, `^{commit}`, `^{blob}`, `^{tag}`, `^{}` suffixes; implement peeling algorithm: resolve base OID, read object, follow tags/commits until target type reached (FR-022, R14, data-model.md §5)
- [X] T039 [US7] Wire rev-parse peeling in `crates/git-cli/src/commands/rev_parse.rs` — ensure `resolve_revision()` is called with the full expression including ^{type} suffix (FR-022)
- [X] T040 [US7] Write plumbing parity interop tests in `crates/git-cli/tests/parity_tests.rs` — 3 tests matching acceptance scenarios: (1) for-each-ref excludes HEAD, (2) rev-parse HEAD^{tree} returns tree OID, (3) multiple peeling expressions match C git

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes plumbing tests. Scripting-compatible output matches C git.

---

## Phase 10: User Story 8 — Rebase Correctness (Priority: P3)

**Goal**: Linear rebase produces identical OIDs to C git when timestamps are pinned.

**Independent Test**: Rebase feature branch onto main in parallel repos, compare log --oneline output.

### Implementation for User Story 8

- [X] T041 [US8] Fix committer date handling in rebase in `crates/git-cli/src/commands/rebase.rs` — ensure `build_committer()` respects `GIT_COMMITTER_DATE` environment variable before falling back to `GitDate::now()`, so that pinned timestamps in test environments produce identical OIDs (FR-023, R15)
- [X] T042 [US8] Verify author info preservation in `crates/git-cli/src/commands/rebase.rs` — confirm cherry-pick during rebase preserves original author name, email, and timestamp from source commits
- [X] T043 [US8] Write rebase parity interop test in `crates/git-cli/tests/parity_tests.rs` — 1 test: feature branch with 2 commits diverged from main, rebase onto main, compare log --oneline and OIDs between gitr and C git with pinned timestamps

**Checkpoint**: `cargo test -p git-cli --test parity_tests` passes rebase test. Rebased OIDs match C git.

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: Final validation across all stories, CI readiness

- [X] T044 Run `cargo clippy --workspace -- -D warnings` and fix any warnings introduced by changes
- [X] T045 Run `cargo test --workspace` full suite to verify no regressions in existing 108 tests
- [X] T046 Verify all 25+ parity interop tests pass together via `cargo test -p git-cli --test parity_tests`
- [X] T047 Run quickstart.md manual verification steps to validate end-to-end behavior

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Setup (T001-T003)
- **US1 Merge (Phase 3)**: Depends on Setup (T001-T002 for date format in merge output)
- **US2 Diff (Phase 4)**: Depends on Foundational (T004-T005 for packfile reading in diff)
- **US3 Output Format (Phase 5)**: Depends on Setup (T001-T003 for date format and path quoting)
- **US4 Packfile (Phase 6)**: Depends on Foundational (T004-T005)
- **US5 Remote (Phase 7)**: Depends on Foundational (T004-T005) and US1 (merge for pull)
- **US6 Stash (Phase 8)**: Depends on Setup only (no cross-story deps)
- **US7 Plumbing (Phase 9)**: Depends on Setup only (no cross-story deps)
- **US8 Rebase (Phase 10)**: Depends on Setup only (no cross-story deps)
- **Polish (Phase 11)**: Depends on all story phases

### User Story Dependencies

```
Phase 1: Setup ──────────────────┬───────────────────────────────────┐
                                 │                                   │
Phase 2: Foundational ───────────┤                                   │
                                 │                                   │
                  ┌──────────────┼──────────────┐                    │
                  │              │              │                    │
           Phase 3: US1   Phase 5: US3   Phase 8: US6    Phase 9: US7
           (Merge P1)     (Format P1)    (Stash P2)      (Plumbing P2)
                  │              │                                   │
                  │       Phase 4: US2                     Phase 10: US8
                  │       (Diff P1)                        (Rebase P3)
                  │              │
                  │       Phase 6: US4
                  │       (Packfile P1)
                  │              │
                  └──────┬───────┘
                         │
                  Phase 7: US5
                  (Remote P2)
                         │
                  Phase 11: Polish
```

### Within Each User Story

- Implementation tasks first (fix the behavior)
- Interop test task last (validates the fix)
- Tasks marked [P] within a phase can run in parallel

### Parallel Opportunities

- **T001 + T003**: DateFormat::Default and quote_path() are in different files
- **T014-T020**: All US3 output format fixes are in different command files
- **T037 + T038**: for-each-ref and rev-parse peeling are independent
- **US1 + US3 + US6 + US7**: After setup, these 4 stories can proceed in parallel
- **US2 + US4**: After foundational, these can proceed in parallel

---

## Parallel Example: User Story 3 (Output Format)

```bash
# All these can run in parallel (different files):
Task: "Fix log date format in crates/git-cli/src/commands/log.rs"
Task: "Fix log --format= newline handling in crates/git-cli/src/commands/log.rs"  # same file as above — run sequentially
Task: "Fix show date format in crates/git-cli/src/commands/show.rs"
Task: "Fix blame date+time format in crates/git-cli/src/commands/blame.rs"
Task: "Fix detached HEAD status in crates/git-cli/src/commands/status.rs"
Task: "Fix unicode path escaping in crates/git-cli/src/commands/ls_files.rs"

# Then after all fixes:
Task: "Write output format parity interop tests in crates/git-cli/tests/parity_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1: Merge Parity)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 3: US1 Merge (T006-T010)
3. **STOP and VALIDATE**: `cargo test -p git-cli --test parity_tests` — merge tests pass
4. Merge behavior now matches C git for FF, three-way, and conflicts

### Incremental Delivery

1. Setup + Foundational → Shared infrastructure ready
2. US1 (Merge) → Core branch workflow works → Validate
3. US2 (Diff) + US3 (Format) + US4 (Packfile) → All P1 complete → Validate
4. US5 (Remote) + US6 (Stash) + US7 (Plumbing) → All P2 complete → Validate
5. US8 (Rebase) → P3 complete → Full parity achieved
6. Polish → CI green, all 25+ tests pass

### Suggested MVP Scope

**Phases 1-3** (Setup + US1 Merge): 10 tasks. Delivers the highest-impact fix (merge parity) and establishes the parity test infrastructure that all subsequent stories build on.

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- All interop tests use `tempfile::tempdir()` for isolation and `common/mod.rs` helpers
- Interop tests compare gitr vs C git output using `assert_output_eq()` or `assert_stdout_eq()`
- Commit after each completed phase
- Stop at any checkpoint to validate independently

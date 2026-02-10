# Tasks: Git Behavioral Parity Polish

**Input**: Design documents from `.specify/specs/024-git-parity-polish/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included — spec SC-007 requires each fix to have a dedicated E2E interop test.

**Organization**: Tasks grouped by user story (5 stories: P0 Core, P1 Flags, P2 Formatting, P2 Exit Codes, P3 Config/Init).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the E2E test file and shared helpers needed by all stories

- [x] T001 Create parity polish E2E test file in `crates/git-cli/tests/parity_polish_tests.rs` with `mod common; use common::*;` imports and section comments for each user story
- [x] T002 Add `-N` → `--max-count=N` transformation in `preprocess_args()` in `crates/git-cli/src/main.rs` — detect args matching `-\d+` pattern (e.g., `-1`, `-3`, `-10`) and rewrite to `--max-count=N` before clap parsing (R8)
- [x] T003 Override clap error handler in `crates/git-cli/src/main.rs` — replace `Cli::parse_from()` with `Cli::try_parse_from()`, match on error kind: `DisplayHelp`/`DisplayVersion` → exit 0, all other errors → `e.print()` + exit 128 (FR-038, R11, exit-code-mapping contract)

**Checkpoint**: Shared infrastructure ready — user story phases can proceed

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Library-level fixes in git-utils, git-revwalk, git-config, and git-repository that multiple user stories depend on

- [x] T004 [P] Fix date padding in `crates/git-utils/src/date.rs` — change `DateFormat::Default` strftime from `%e` to `%-e` (no-pad) and `DateFormat::Rfc2822` from `%d` to `%-d` (no-pad) so day-of-month has no space/zero padding (FR-023/024, data-model entity 6)
- [x] T005 [P] Add `%d` and `%D` decoration placeholders in `crates/git-revwalk/src/pretty.rs` — extend `format_commit()` signature to accept optional `decorations: Option<&HashMap<ObjectId, Vec<String>>>` parameter; implement `%d` (` (HEAD -> main, tag: v1.0)` with leading space) and `%D` (same without parens); pass-through unknown placeholders as literal text (FR-012, R7, format-string-parity contract)
- [x] T006 [P] Fix `format_builtin()` for `BuiltinFormat::Oneline` in `crates/git-revwalk/src/pretty.rs` — use full 40-char `oid.to_hex()` instead of abbreviated hash (FR-010)
- [x] T007 [P] Fix `format_builtin()` for `BuiltinFormat::Raw` in `crates/git-revwalk/src/pretty.rs` — indent each line of the commit message body with 4 leading spaces (FR-011)
- [x] T008 [P] Add system config file discovery in `crates/git-config/src/lib.rs` — add `fn system_config_path() -> Option<PathBuf>` that checks `GIT_CONFIG_SYSTEM` env, then platform paths (macOS: `/Library/Developer/CommandLineTools/usr/share/git-core/gitconfig`, Linux: `/etc/gitconfig`); respect `GIT_CONFIG_NOSYSTEM`; load system config first in cascade (system → global → local) (FR-039, R12, config-cascade contract)
- [x] T009 [P] Add macOS platform config in `crates/git-repository/src/lib.rs` — during `init_opts()`, on `#[cfg(target_os = "macos")]`, write `core.ignorecase = true` and `core.precomposeunicode = true` to `.git/config` (FR-041, R13, config-cascade contract)
- [x] T010 [P] Add sample hook file creation in `crates/git-repository/src/lib.rs` — during `init_opts()`, create standard sample hook files (`pre-commit.sample`, `commit-msg.sample`, `pre-push.sample`, `pre-rebase.sample`, `post-update.sample`, `prepare-commit-msg.sample`, `applypatch-msg.sample`, `fsmonitor-watchman.sample`, `push-to-checkout.sample`, `sendemail-validate.sample`) in `.git/hooks/` with content matching C git's templates (FR-042, R13)

**Checkpoint**: Foundation ready — all library-level changes complete, user stories can now proceed in parallel

---

## Phase 3: User Story 1 — Core Command Correctness (Priority: P0) 🎯 MVP

**Goal**: Fix the 12 most critical command bugs — pathspec disambiguation in diff/reset, check-ignore directory patterns, log date filtering, init directory creation, show stat/merge/format fixes, commit-tree stdin

**Independent Test**: Run each fixed command with identical inputs in both git and gitr repos and assert byte-identical stdout, stderr, and exit codes

### E2E Tests for User Story 1

- [x] T011 [P] [US1] Write E2E tests for diff pathspec disambiguation in `crates/git-cli/tests/parity_polish_tests.rs` — test bare `diff file.txt`, `diff -- file.txt`, `diff HEAD -- file.txt` against git output (FR-001 scenarios 1-3)
- [x] T012 [P] [US1] Write E2E test for diff --cached hunk headers in `crates/git-cli/tests/parity_polish_tests.rs` — stage a new file, run `diff --cached`, assert `@@ -0,0 +1,N @@` header format (FR-002 scenario 4)
- [x] T013 [P] [US1] Write E2E tests for reset bare pathspec in `crates/git-cli/tests/parity_polish_tests.rs` — test `reset file.txt` and `reset HEAD file.txt` against git (FR-003 scenarios 5-6)
- [x] T014 [P] [US1] Write E2E tests for check-ignore directory patterns in `crates/git-cli/tests/parity_polish_tests.rs` — test `check-ignore build/output.o` and `check-ignore build` with `build/` in .gitignore (FR-004 scenarios 7-8)
- [x] T015 [P] [US1] Write E2E test for log --since/--until in `crates/git-cli/tests/parity_polish_tests.rs` — create commits with different dates, filter with --since/--until, compare output (FR-005 scenario 9)
- [x] T016 [P] [US1] Write E2E test for init nested directory creation in `crates/git-cli/tests/parity_polish_tests.rs` — run `init /tmp/new/nested/repo`, verify directory hierarchy created (FR-006 scenario 10)
- [x] T017 [P] [US1] Write E2E tests for show --stat, merge header, and format modes in `crates/git-cli/tests/parity_polish_tests.rs` — test `show --stat`, `show --no-patch` on merge commit, `show --format=oneline`, `show --format=raw`, `show --format='%H %s'` (FR-007/008/010/011/012 scenarios 11-16)
- [x] T018 [P] [US1] Write E2E test for commit-tree stdin message in `crates/git-cli/tests/parity_polish_tests.rs` — pipe message via stdin to `commit-tree`, verify commit created (FR-009 scenario 13)

### Implementation for User Story 1

- [x] T019 [US1] Implement pathspec disambiguation in `crates/git-cli/src/commands/diff.rs` — modify `parse_diff_args()` to try `resolve_revision()` for each arg before `--`; if resolution fails and `Path::exists(arg)`, treat as pathspec; if neither, emit git-matching `fatal: ambiguous argument` error (FR-001, R1, pathspec-disambiguation contract)
- [x] T020 [US1] Fix hunk header off-by-one in `crates/git-cli/src/commands/diff.rs` (or `crates/git-diff/src/format/unified.rs`) — when source is `/dev/null` (new file), set start line to 0 producing `@@ -0,0 +1,N @@` (FR-002, R2)
- [x] T021 [US1] Implement pathspec disambiguation in `crates/git-cli/src/commands/reset.rs` — remove `#[arg(last = true)]` from paths field, add custom parsing: try first positional as revision, if fails and exists as path treat all positionals as pathspecs with implicit HEAD (FR-003, R1, pathspec-disambiguation contract)
- [x] T022 [US1] Fix directory pattern matching in `crates/git-cli/src/commands/check_ignore.rs` — when a gitignore pattern ends with `/` (e.g., `build/`), match both the directory name itself AND any path starting with the directory prefix (FR-004, R3)
- [x] T023 [US1] Implement --since/--until date filtering in `crates/git-cli/src/commands/log.rs` — parse since/until strings to timestamps via date parsing, filter commits post-walk by comparing `author.date.timestamp` against the range bounds (FR-005, R4)
- [x] T024 [US1] Add recursive directory creation in `crates/git-cli/src/commands/init.rs` — call `std::fs::create_dir_all()` on the target path before `Repository::init_opts()` (FR-006, R5)
- [x] T025 [US1] Fix show --stat to suppress diff in `crates/git-cli/src/commands/show.rs` — when `--stat` is set, skip the unified diff output after the stat summary (FR-007, R6)
- [x] T026 [US1] Add merge commit header in `crates/git-cli/src/commands/show.rs` — when `commit.parents.len() > 1`, emit `Merge: <short1> <short2>` line between the `commit` line and `Author:` line (FR-008, R6)
- [x] T027 [US1] Verify commit-tree stdin reading in `crates/git-cli/src/commands/commit_tree.rs` — confirm existing stdin fallback works when no `-m` flag is provided; fix if needed (FR-009)
- [x] T028 [US1] Fix show --format=oneline and --format=raw in `crates/git-cli/src/commands/show.rs` — for oneline, use full 40-char hash; for raw, indent message body with 4 spaces; wire decoration map for `%d`/`%D` when --decorate is used or format contains `%d`/`%D` (FR-010/011/012, R6)

**Checkpoint**: All 12 P0 core command fixes complete and independently testable via E2E tests

---

## Phase 4: User Story 2 — Missing Flag Support (Priority: P1)

**Goal**: Add 10 commonly used flags that gitr currently rejects

**Independent Test**: Run each flag in both git and gitr and assert identical output

### E2E Tests for User Story 2

- [x] T029 [P] [US2] Write E2E test for `log -3` in `crates/git-cli/tests/parity_polish_tests.rs` — create 10+ commits, run `log -3`, assert exactly 3 shown (FR-013)
- [x] T030 [P] [US2] Write E2E test for `log --oneline --decorate` in `crates/git-cli/tests/parity_polish_tests.rs` — create repo with branches/tags, compare decoration format (FR-014)
- [x] T031 [P] [US2] Write E2E test for `branch -v` in `crates/git-cli/tests/parity_polish_tests.rs` — create branches, compare verbose output format (FR-015)
- [x] T032 [P] [US2] Write E2E test for `tag -n` in `crates/git-cli/tests/parity_polish_tests.rs` — create annotated tags, verify annotation lines displayed (FR-016)
- [x] T033 [P] [US2] Write E2E test for `ls-tree -l HEAD` in `crates/git-cli/tests/parity_polish_tests.rs` — create files of varying sizes, compare long format output (FR-017)
- [x] T034 [P] [US2] Write E2E tests for `rev-parse --abbrev-ref HEAD` and `rev-parse --short HEAD` in `crates/git-cli/tests/parity_polish_tests.rs` (FR-018/019)
- [x] T035 [P] [US2] Write E2E test for `config --local user.name` in `crates/git-cli/tests/parity_polish_tests.rs` (FR-020)
- [x] T036 [P] [US2] Write E2E test for `format-patch --stdout HEAD~1` in `crates/git-cli/tests/parity_polish_tests.rs` (FR-021)
- [x] T037 [P] [US2] Write E2E test for `remote show origin` in `crates/git-cli/tests/parity_polish_tests.rs` (FR-022)

### Implementation for User Story 2

- [x] T038 [US2] Add `--decorate` flag and decoration map to `crates/git-cli/src/commands/log.rs` — add `#[arg(long)] decorate: bool` to LogArgs; at start of `run()`, build `HashMap<ObjectId, Vec<String>>` by iterating all refs; pass map to `format_commit()` and `format_builtin()`; order: HEAD first, then local branches, then remote-tracking, then tags (FR-014, R9)
- [x] T039 [P] [US2] Add `-v`/`--verbose` flag to `crates/git-cli/src/commands/branch.rs` — add `#[arg(short = 'v', long)] verbose: bool`; in `list_branches()`, when verbose, resolve tip commit for each branch and display `<name> <short-hash> <subject>` with `*` prefix for current branch (FR-015, R10)
- [x] T040 [P] [US2] Add `-n[num]` flag to `crates/git-cli/src/commands/tag.rs` — add annotation display option; when listing tags, if `-n` is present, read the tag object and display up to N annotation lines after each tag name (FR-016)
- [x] T041 [P] [US2] Add `-l`/`--long` flag to `crates/git-cli/src/commands/ls_tree.rs` — add `#[arg(short = 'l', long)] long: bool`; in `print_entry()`, when long mode, look up object size from ODB and include in output between mode/type and name (FR-017)
- [x] T042 [P] [US2] Add `--abbrev-ref` flag to `crates/git-cli/src/commands/rev_parse.rs` — add `#[arg(long)] abbrev_ref: bool`; when set, resolve symbolic ref and output short name (e.g., `main` instead of `refs/heads/main`) (FR-018)
- [x] T043 [P] [US2] Fix `--short` as optional-value flag in `crates/git-cli/src/commands/rev_parse.rs` — change `short` field to `Option<Option<usize>>` using clap's `num_args = 0..=1, default_missing_value = "7"` so `--short` alone defaults to 7 and `--short=N` uses N (FR-019)
- [x] T044 [P] [US2] Add `--local` flag to `crates/git-cli/src/commands/config.rs` — add `#[arg(long)] local: bool`; when set, read/write only from `.git/config`, ignoring global and system config (FR-020, config-cascade contract)
- [x] T045 [P] [US2] Add `--stdout` flag to `crates/git-cli/src/commands/format_patch.rs` — add `#[arg(long)] stdout: bool`; when set, write patch content to stdout instead of creating files (FR-021)
- [x] T046 [US2] Add `show` subcommand to `crates/git-cli/src/commands/remote.rs` — add `Show { name: String }` variant to `RemoteSubcommand`; implement handler that reads remote config (URL, fetch refspec) and displays URL, fetch/push refspecs, HEAD branch, and tracking info (FR-022)

**Checkpoint**: All 10 P1 missing flags work — previously rejected flags now accepted

---

## Phase 5: User Story 3 — Output Formatting Fidelity (Priority: P2)

**Goal**: Fix 12 formatting differences for character-identical output

**Independent Test**: Run each command in both git and gitr, capture stdout, diff character-by-character

### E2E Tests for User Story 3

- [x] T047 [P] [US3] Write E2E test for default date format padding in `crates/git-cli/tests/parity_polish_tests.rs` — create commit on single-digit day, compare `log` date output (FR-023)
- [x] T048 [P] [US3] Write E2E test for email date format in `crates/git-cli/tests/parity_polish_tests.rs` — compare `log --format=email` date output (FR-024)
- [x] T049 [P] [US3] Write E2E test for `diff --stat` alignment in `crates/git-cli/tests/parity_polish_tests.rs` (FR-025)
- [x] T050 [P] [US3] Write E2E test for `log --stat` blank line in `crates/git-cli/tests/parity_polish_tests.rs` (FR-026)
- [x] T051 [P] [US3] Write E2E test for `log --graph --oneline --all` in `crates/git-cli/tests/parity_polish_tests.rs` — create branching history, compare graph output (FR-027)
- [x] T052 [P] [US3] Write E2E test for `clean -n` sorted output and directory exclusion in `crates/git-cli/tests/parity_polish_tests.rs` (FR-028/029)
- [x] T053 [P] [US3] Write E2E test for commit summary line in `crates/git-cli/tests/parity_polish_tests.rs` — commit a file, verify `N files changed, N insertions(+)` in output (FR-030)
- [x] T054 [P] [US3] Write E2E test for `stash pop` status output in `crates/git-cli/tests/parity_polish_tests.rs` (FR-031)
- [x] T055 [P] [US3] Write E2E test for init symlink path resolution in `crates/git-cli/tests/parity_polish_tests.rs` (FR-032)
- [x] T056 [P] [US3] Write E2E test for format-patch filename prefix in `crates/git-cli/tests/parity_polish_tests.rs` (FR-033)
- [x] T057 [P] [US3] Write E2E test for shortlog oldest-first ordering in `crates/git-cli/tests/parity_polish_tests.rs` (FR-034)

### Implementation for User Story 3

- [x] T058 [P] [US3] Fix `diff --stat` alignment in `crates/git-diff/src/format/stat.rs` — use dynamic count_width based on max changes, matching git's column alignment algorithm (FR-025)
- [x] T059 [P] [US3] Add blank line between message and stat in `crates/git-cli/src/commands/log.rs` — when `--stat` is used, insert `\n` between the commit message output and the stat summary (FR-026)
- [x] T060 [P] [US3] Fix `--graph` compact notation in `crates/git-cli/src/commands/log.rs` (or `crates/git-revwalk/src/graph_draw.rs`) — ensure graph rendering uses git's compact `|/` and `|\` notation without extra blank lines between non-branching commits (FR-027)
- [x] T061 [P] [US3] Sort `clean -n` output alphabetically in `crates/git-cli/src/commands/clean.rs` — collect all paths into a Vec, sort, then print; also ensure files inside untracked directories are NOT listed when `-d` is not specified (FR-028/029)
- [x] T062 [P] [US3] Add commit summary line in `crates/git-cli/src/commands/commit.rs` — after creating the commit, compute diff stats (files changed, insertions, deletions) and output the `N files changed, N insertions(+), N deletions(-)` summary line with mode information (FR-030)
- [x] T063 [P] [US3] Add full status output after `stash pop` in `crates/git-cli/src/commands/stash.rs` — after applying the stash, run the status display logic (same as `git status` short output) showing working tree changes (FR-031)
- [x] T064 [P] [US3] Resolve symlinks in init success message in `crates/git-cli/src/commands/init.rs` — use `std::fs::canonicalize()` on the git dir path and ensure trailing `/` in the success message (FR-032)
- [x] T065 [P] [US3] Remove `./` prefix from format-patch filenames in `crates/git-cli/src/commands/format_patch.rs` — strip leading `./` from the output filename when printing the generated patch file path (FR-033)
- [x] T066 [P] [US3] Fix shortlog commit ordering in `crates/git-cli/src/commands/shortlog.rs` — reverse the per-author commit vector before display so commits appear oldest-first within each author group (FR-034, R14)

**Checkpoint**: All 12 P2 formatting fixes produce character-identical output to git

---

## Phase 6: User Story 4 — Exit Code Compatibility (Priority: P2)

**Goal**: Fix 4 exit code mismatches (3 command-specific + 1 global clap override)

**Independent Test**: Run each error scenario in both git and gitr and assert identical exit codes

### E2E Tests for User Story 4

- [x] T067 [P] [US4] Write E2E test for `show-ref --verify` exit code in `crates/git-cli/tests/parity_polish_tests.rs` — verify nonexistent ref returns exit code 1 (FR-035)
- [x] T068 [P] [US4] Write E2E test for `branch -d nonexistent` exit code in `crates/git-cli/tests/parity_polish_tests.rs` (FR-036)
- [x] T069 [P] [US4] Write E2E test for `checkout nonexistent` exit code in `crates/git-cli/tests/parity_polish_tests.rs` (FR-037)
- [x] T070 [P] [US4] Write E2E test for invalid CLI argument exit code in `crates/git-cli/tests/parity_polish_tests.rs` — run `gitr log --bogus-flag`, assert exit code is 128 (FR-038)

### Implementation for User Story 4

- [x] T071 [US4] Verify/fix `show-ref --verify` exit code in `crates/git-cli/src/commands/show_ref.rs` — return `Ok(128)` when ref not found matching git behavior (FR-035, exit-code-mapping contract)
- [x] T072 [P] [US4] Fix `branch -d` exit code in `crates/git-cli/src/commands/branch.rs` — when branch doesn't exist, return `Ok(1)` with `error: branch '<name>' not found.` to stderr (FR-036, exit-code-mapping contract)
- [x] T073 [P] [US4] Fix `checkout` exit code in `crates/git-cli/src/commands/checkout.rs` — when ref doesn't exist, return `Ok(1)` with `error: pathspec '<name>' did not match any file(s) known to git` to stderr (FR-037, exit-code-mapping contract)

**Checkpoint**: All 4 exit code scenarios return the same exit code as git (T003 already handles FR-038 globally)

---

## Phase 7: User Story 5 — Config and Init Platform Parity (Priority: P3)

**Goal**: Fix 4 config/init platform behaviors for macOS parity

**Independent Test**: Run init and config commands in both git and gitr and compare config files and output

### E2E Tests for User Story 5

- [x] T074 [P] [US5] Write E2E test for system config loading in `crates/git-cli/tests/parity_polish_tests.rs` — on macOS, verify `config --list` includes system-level entries when system config exists (FR-039)
- [x] T075 [P] [US5] Write E2E test for `config --show-origin` in `crates/git-cli/tests/parity_polish_tests.rs` — set a local config value, query with --show-origin, verify `file:.git/config\tkey=value` format (FR-040)
- [x] T076 [P] [US5] Write E2E test for macOS init config in `crates/git-cli/tests/parity_polish_tests.rs` — on macOS, run `init`, inspect .git/config for `ignorecase = true` and `precomposeunicode = true` (FR-041)
- [x] T077 [P] [US5] Write E2E test for sample hook files in `crates/git-cli/tests/parity_polish_tests.rs` — after `init`, verify `.git/hooks/pre-commit.sample` and other standard hooks exist (FR-042)

### Implementation for User Story 5

- [x] T078 [US5] Wire system config into `config --list` in `crates/git-cli/src/commands/config.rs` — update `all_entries()` call to include system config from the cascade (FR-039, depends on T008)
- [x] T079 [P] [US5] Add `--show-origin` prefix for single-key query in `crates/git-cli/src/commands/config.rs` — when querying a single key with `--show-origin`, output `file:<path>\t<key>=<value>` format showing which config file the value comes from (FR-040, config-cascade contract)

**Checkpoint**: All 4 P3 config/init behaviors match git on macOS (T008/T009/T010 from Phase 2 handle FR-039/041/042 library-level changes)

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Regression validation and final cleanup

- [x] T080 Run `cargo test --workspace` and verify zero regressions in existing test suite
- [x] T081 Run `cargo clippy --workspace` and fix any new warnings introduced by this feature
- [x] T082 Run quickstart.md verification scenarios end-to-end to confirm all fixes work

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 for test file (T001)
  - T004-T010 can all run in parallel (different files/crates)
- **User Stories (Phases 3-7)**: All depend on Phase 2 completion
  - US1 (Phase 3): Depends on T005 (format_commit decorations), T006/T007 (builtin format fixes)
  - US2 (Phase 4): Depends on T005 (decorations for --decorate flag)
  - US3 (Phase 5): Depends on T004 (date padding fixes)
  - US4 (Phase 6): Depends on T003 (clap exit code override)
  - US5 (Phase 7): Depends on T008/T009/T010 (config/init library changes)
- **Polish (Phase 8)**: Depends on all user stories complete

### User Story Dependencies

- **US1 (P0)**: Can start after Phase 2 — no dependencies on other stories
- **US2 (P1)**: Can start after Phase 2 — no dependencies on other stories
- **US3 (P2)**: Can start after Phase 2 — no dependencies on other stories
- **US4 (P2)**: Can start after Phase 1 (T003 handles FR-038) — no dependencies on other stories
- **US5 (P3)**: Can start after Phase 2 — no dependencies on other stories

### Within Each User Story

- E2E tests written FIRST (should fail initially)
- Implementation tasks fix the behaviors
- After implementation, re-run E2E tests to confirm they pass

### Parallel Opportunities

**Phase 2** (all different files):
- T004 (git-utils/date.rs) ‖ T005-T007 (git-revwalk/pretty.rs) ‖ T008 (git-config/lib.rs) ‖ T009-T010 (git-repository/lib.rs)

**Within each user story**, all tasks marked [P] can run in parallel:
- US1: T011-T018 (tests) in parallel, then T019-T028 many in parallel (different files)
- US2: T029-T037 (tests) in parallel, then T039-T045 (different command files) in parallel
- US3: T047-T057 (tests) in parallel, then T058-T066 (different command files) in parallel
- US4: T067-T070 (tests) in parallel, then T071-T073 (different command files) in parallel
- US5: T074-T077 (tests) in parallel, then T078-T079 in parallel

**Across user stories** (once Phase 2 is complete):
- US1 ‖ US2 ‖ US3 ‖ US4 ‖ US5 — all independent, can proceed simultaneously

---

## Parallel Example: User Story 1

```bash
# Launch all E2E tests for US1 together (8 test tasks):
Task: T011 "E2E test for diff pathspec disambiguation"
Task: T012 "E2E test for diff --cached hunk headers"
Task: T013 "E2E test for reset bare pathspec"
Task: T014 "E2E test for check-ignore directory patterns"
Task: T015 "E2E test for log --since/--until"
Task: T016 "E2E test for init nested directory creation"
Task: T017 "E2E test for show --stat/merge/format"
Task: T018 "E2E test for commit-tree stdin"

# Then launch parallel implementation tasks (different files):
Task: T019 "Pathspec disambiguation in diff.rs"
Task: T021 "Pathspec disambiguation in reset.rs"     # different file from T019
Task: T022 "Directory pattern matching in check_ignore.rs"
Task: T023 "Date filtering in log.rs"
Task: T024 "Recursive mkdir in init.rs"
Task: T025 "Show --stat fix in show.rs"               # same file as T026/T028
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational (T004-T010)
3. Complete Phase 3: User Story 1 (T011-T028)
4. **STOP and VALIDATE**: Run US1 E2E tests — all 12 P0 fixes should pass
5. This alone fixes the most critical daily-use command bugs

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 (P0 Core) → Test → **MVP — critical bugs fixed**
3. Add US2 (P1 Flags) → Test → commonly-used flags now work
4. Add US3 (P2 Formatting) → Test → character-identical output
5. Add US4 (P2 Exit Codes) → Test → script/CI compatibility
6. Add US5 (P3 Config/Init) → Test → platform parity
7. Polish → Full regression validation

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (P0 — most critical)
   - Developer B: User Story 2 (P1 — flag support)
   - Developer C: User Story 3 + 4 (P2 — formatting + exit codes)
   - Developer D: User Story 5 (P3 — config/init)
3. Stories complete and integrate independently — no cross-story conflicts

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks in same phase
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- E2E tests use the existing common test harness (`crates/git-cli/tests/common/mod.rs`)
- All E2E tests follow the dual-repo pattern: setup identical repos, run git and gitr, compare output
- The existing test harness pins `GIT_CONFIG_NOSYSTEM=1`, so system config changes (T008) won't affect existing tests
- Total: 82 tasks across 8 phases

# Tasks: Full Git CLI Parity (Phase 3)

**Input**: Design documents from `specs/026-git-parity-phase3/`
**Prerequisites**: plan.md, spec.md, data-model.md, contracts/cli-interface.md, research.md, quickstart.md

**Tests**: Not explicitly requested — test tasks omitted. E2E parity tests are included in the Polish phase as integration validation.

**Organization**: Tasks grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US6)
- Include exact file paths in descriptions

## Path Conventions

- **Cargo workspace**: `crates/<crate>/src/` for library code, `crates/git-cli/src/commands/` for CLI commands, `tests/` for E2E tests

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify workspace builds and add any missing dependencies

- [ ] T001 Verify workspace builds cleanly with `cargo build --workspace` and `cargo clippy --workspace` on current branch
- [ ] T002 Add `regex` dependency to `crates/git-diff/Cargo.toml` for pickaxe `-S`/`-G` search support (FR-062)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Library-level modules and structs that MUST be complete before ANY user story can be implemented. All additions are modules in existing crates — no new crates.

**CRITICAL**: No user story work can begin until this phase is complete.

### Color Infrastructure

- [ ] T003 [P] Implement `ColorSlot` enum with all git color slots (diff, status, branch, log/decorate, grep) and default ANSI mappings in `crates/git-utils/src/color.rs`
- [ ] T004 [P] Implement `ColorConfig` struct with `from_config()`, `effective_mode()`, and `get_color()` methods in `crates/git-utils/src/color.rs` — reads `color.ui`, `color.<cmd>`, and `color.<cmd>.<slot>` from config; supports custom color values (`normal`, `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `bold`, `dim`, `ul`, `blink`, `reverse`, `strike`, `#RRGGBB`)

### Diff Extensions

- [ ] T005 [P] Implement colored diff output formatting module in `crates/git-diff/src/color.rs` — wraps diff lines in ANSI escapes using `ColorConfig` from `git-utils`; handles old/new/context/meta/frag/func info slots
- [ ] T006 [P] Implement pickaxe search (`-S` string / `-G` regex) module in `crates/git-diff/src/pickaxe.rs` — filters diff output to only show hunks/files matching search pattern; uses `regex` crate for `-G`

### Utility Modules

- [ ] T007 [P] Implement `Mailmap` struct with `.mailmap` parsing (all 4 formats), `from_file()`, `from_config()`, and `lookup()` methods in `crates/git-utils/src/mailmap.rs` — reads from repo root `.mailmap` and `mailmap.file` config (FR-406)

### Repository Infrastructure

- [ ] T008 [P] Extend `HookRunner` with complete `HookType` enum (pre-commit, prepare-commit-msg, commit-msg, post-commit, pre-rebase, post-rewrite, post-checkout, post-merge, pre-push, pre-auto-gc, reference-transaction), `hook_exists()`, `run()`, and `run_or_ok()` methods in `crates/git-repository/src/hooks.rs` — resolves hooks path from `core.hooksPath` config or `.git/hooks/` (FR-401)
- [ ] T009 [P] Implement `GpgSigner` struct with `from_config()`, `sign()`, and `verify()` methods in `crates/git-repository/src/gpg.rs` — delegates to external `gpg` binary via `std::process::Command`; reads `gpg.program`, `user.signingKey`, `gpg.format` config (FR-402)
- [ ] T010 [P] Implement `EditorConfig` struct with `from_config()` and `edit_file()` in `crates/git-repository/src/editor.rs` — resolution cascade: `$GIT_EDITOR` > `core.editor` > `$VISUAL` > `$EDITOR` > `vi` (FR-410)

### Index/Attributes Extensions

- [ ] T011 [P] Extend `AttributeStack` in `crates/git-index/src/attributes.rs` with `is_binary()`, `eol_for()`, `diff_driver()`, `merge_driver()`, `filter_for()` convenience methods; add loading from `core.attributesFile` config and `$GIT_DIR/info/attributes` (FR-400)
- [ ] T012 [P] Implement sparse checkout support module in `crates/git-index/src/sparse.rs` — cone mode patterns, `$GIT_DIR/info/sparse-checkout` file parsing, working tree filtering (FR-314)

### Merge Extensions

- [ ] T013 [P] Implement `RerereDatabase` struct with `new()`, `record()`, `resolve()`, `forget()`, `gc()` methods in `crates/git-merge/src/rerere.rs` — manages `.git/rr-cache/` directory, conflict ID hashing, resolution storage (FR-315)
- [ ] T014 [P] Add `Octopus` variant to `MergeStrategyType` enum and implement octopus merge strategy in `crates/git-merge/src/strategy/octopus.rs` — merges 3+ branches, fails on conflict, produces multi-parent commit (FR-403)

### Revwalk Extensions

- [ ] T015 [P] Implement `merge_base()` common ancestor computation in `crates/git-revwalk/src/merge_base.rs` — supports `--all`, `--octopus`, `--is-ancestor`, `--fork-point` modes (FR-305)
- [ ] T016 [P] Implement cherry/cherry-pick filtering in `crates/git-revwalk/src/cherry.rs` — identifies commits not yet applied upstream by comparing patch IDs (FR-301)

### Transport Extensions

- [ ] T017 [P] Extend `CredentialHelper` in `crates/git-transport/src/credential.rs` with full helper invocation: `fill()`, `approve()`, `reject()` methods using `std::process::Command`; wire format protocol (key=value pairs, blank-line terminated); helper chain from `credential.helper` config (FR-408)

**Checkpoint**: Foundation ready — all library-level modules in place. User story implementation can now begin.

---

## Phase 3: User Story 1 — Color and Pager Output (Priority: P1) MVP

**Goal**: All color-producing commands output ANSI-colored text matching git's default scheme; long output is auto-piped through a pager.

**Independent Test**: Run `gitr log --oneline --all` in a 100+ commit repo — verify colored output and pager invocation. Run `diff <(git diff --color=always) <(gitr diff --color=always)` — verify identical ANSI codes.

### Implementation for User Story 1

- [ ] T018 [US1] Create shared `--color[=<when>]` clap arg group (auto/always/never) reusable across all commands in `crates/git-cli/src/commands/mod.rs` or a shared args module
- [ ] T019 [US1] Integrate `ColorConfig` and `--color` flag into `diff` command output in `crates/git-cli/src/commands/diff.rs` — wrap removed/added/context/meta/frag lines with appropriate ANSI escapes (FR-010, FR-013)
- [ ] T020 [P] [US1] Integrate `ColorConfig` and `--color` flag into `log` command output in `crates/git-cli/src/commands/log.rs` — color commit hashes (yellow), decorations (branch=green, remote=red, tag=bold yellow, HEAD=bold cyan) (FR-010, FR-013)
- [ ] T021 [P] [US1] Integrate `ColorConfig` and `--color` flag into `status` command output in `crates/git-cli/src/commands/status.rs` — color staged (green), changed (red), untracked (red) sections (FR-010, FR-013)
- [ ] T022 [P] [US1] Integrate `ColorConfig` and `--color` flag into `branch` command output in `crates/git-cli/src/commands/branch.rs` — color current (green), local (none), remote (red), upstream (blue) (FR-010, FR-013)
- [ ] T023 [P] [US1] Integrate `ColorConfig` and `--color` flag into `grep` command output in `crates/git-cli/src/commands/grep.rs` — color filename (magenta), line number (green), separator (cyan), match (bold red) (FR-010, FR-013)
- [ ] T024 [P] [US1] Integrate `ColorConfig` and `--color` flag into `show` command output in `crates/git-cli/src/commands/show.rs` — same color scheme as diff + log decorations (FR-010, FR-013)
- [ ] T025 [P] [US1] Integrate `ColorConfig` and `--color` flag into `blame` command output in `crates/git-cli/src/commands/blame.rs` (FR-010)
- [ ] T026 [P] [US1] Integrate `ColorConfig` and `--color` flag into `shortlog` command output in `crates/git-cli/src/commands/shortlog.rs` (FR-010)
- [ ] T027 [US1] Implement `--color-words` for word-level colored diff output in `crates/git-cli/src/commands/diff.rs` (FR-014)
- [ ] T028 [US1] Integrate pager setup into CLI main dispatch in `crates/git-cli/src/main.rs` — call `setup_pager()` before command `run()` for auto-paged commands (log, diff, show, blame, shortlog, grep, branch, tag, help); check terminal detection; set `LESS=FRX` and `LV=-c` env (FR-020, FR-023)
- [ ] T029 [US1] Support per-command pager config overrides (`pager.<cmd>`) in pager resolution cascade in `crates/git-cli/src/main.rs` (FR-021)
- [ ] T030 [US1] Ensure `--color=auto` produces no ANSI codes when stdout is piped (not a terminal) and `$TERM=dumb` suppresses color (FR-011)
- [ ] T031 [US1] Ensure `color.<cmd>=false` config and `color.ui=false` config suppress color when `--color` flag is not explicitly set (FR-012)

**Checkpoint**: Color and pager work identically to git. All visual output matches.

---

## Phase 4: User Story 2 — Top-Level CLI Parity (Priority: P1)

**Goal**: `gitr --version`, `gitr --help`, and all global flags work identically to git; `-C`/`-c` conflicts are resolved.

**Independent Test**: Run `gitr --version` — verify `gitr version X.Y.Z` format. Run `gitr switch -c mybranch` — verify branch creation (not config override). Run `gitr --help` — verify categorized output.

### Implementation for User Story 2

- [ ] T032 [US2] Fix `--version` output to produce `gitr version X.Y.Z` format (not `gitr X.Y.Z`) in `crates/git-cli/src/main.rs` (FR-001)
- [ ] T033 [US2] Implement categorized `--help` output matching git's layout (start a working area, work on current change, examine history, grow/mark/tweak, collaborate) in `crates/git-cli/src/main.rs` (FR-002)
- [ ] T034 [US2] Add global flags `--work-tree`, `--bare`, `--no-replace-objects`, `--config-env`, `--exec-path`, `--namespace` to the `Cli` struct in `crates/git-cli/src/main.rs` and wire them into repository/config initialization (FR-003)
- [ ] T035 [US2] Implement `preprocess_args()` function for two-pass argument parsing: scan for `-C <path>` and `-c key=value` before the subcommand name, strip from args, store in side-channel; remove `-C`/`-c` global short flags from `Cli` struct so subcommand flags (`switch -c`, `diff -C`, `branch -c/-C`, `grep -C`) take precedence (FR-004)
- [ ] T036 [US2] Add `-p`/`--paginate` and `-P`/`--no-pager` global flags to `Cli` struct in `crates/git-cli/src/main.rs` and integrate with pager setup (FR-022)

**Checkpoint**: Top-level CLI matches git. All global flags and help output are correct.

---

## Phase 5: User Story 3 — Missing Command Flags (Priority: P1)

**Goal**: Every flag listed in `git <cmd> -h` is accepted by `gitr <cmd>` and behaves identically. This is the bulk of the parity work.

**Independent Test**: For each command, compare `git <cmd> -h` and `gitr <cmd> -h` side by side; run `gitr commit -s -m "msg"`, `gitr diff -S "func"`, `gitr log --follow`, `gitr merge --continue`, `gitr stash --keep-index` and verify identical behavior to git.

### Implementation for User Story 3

**commit flags** (FR-030 through FR-040):

- [ ] T037 [US3] Add missing commit flags in `crates/git-cli/src/commands/commit.rs`: `-F`/`--file` (read message from file), `-C`/`--reuse-message`, `-c`/`--reedit-message`, `--fixup`, `--squash`, `-s`/`--signoff` (append Signed-off-by trailer), `--trailer` (append arbitrary trailers), `-n`/`--no-verify` (skip hooks), `--dry-run`, `-v`/`--verbose` (show diff in editor), `--date` (override author date), `--reset-author`, and `create mode`/`delete mode` lines in commit summary output

**status flags** (FR-050 through FR-055):

- [ ] T038 [P] [US3] Add missing status flags in `crates/git-cli/src/commands/status.rs`: `-v`/`--verbose` (show staged diff), `-z` (NUL-terminated output), `-u`/`--untracked-files` with modes `no`/`normal`/`all`, `--ignored`, `--column`/`--no-column`, `--ahead-behind`/`--no-ahead-behind`

**diff flags** (FR-060 through FR-069):

- [ ] T039 [US3] Add missing diff flags in `crates/git-cli/src/commands/diff.rs`: `--full-index`, `-R` (reverse diff), `-S`/`-G` (pickaxe — wire to `git-diff/src/pickaxe.rs`), `--diff-filter=ACDMRTUXB*`, `--patience`/`--histogram`/`--minimal` algorithm selection, `--no-index`, `--check` (whitespace errors), `--src-prefix`/`--dst-prefix`/`--no-prefix`, `-z` (NUL-terminated); fix `diff --stat` with single commit argument to show working-tree vs commit diff (FR-068)

**log flags** (FR-070 through FR-080):

- [ ] T040 [US3] Add missing log flags in `crates/git-cli/src/commands/log.rs`: `-L` (line-range log), `--follow` (track renames), `--diff-filter`, `--abbrev-commit`, `--no-decorate`, `-g`/`--walk-reflogs`, `--left-right`, `--cherry-pick`/`--cherry-mark`, auto-decorate when stdout is terminal, `--ancestry-path`, `--simplify-by-decoration`, `--decorate-refs`/`--decorate-refs-exclude`, `--source`, `--use-mailmap` (wire to `git-utils/src/mailmap.rs`)

**show flags** (FR-085 through FR-087):

- [ ] T041 [P] [US3] Add missing show flags in `crates/git-cli/src/commands/show.rs`: `--decorate` (ref decoration), `-q`/`--quiet` (suppress diff), and full annotated tag display (tag header + tagged commit + diff)

**branch flags** (FR-090 through FR-095):

- [ ] T042 [P] [US3] Add missing branch flags in `crates/git-cli/src/commands/branch.rs`: `-t`/`--track`/`--no-track` (upstream tracking), `-u`/`--set-upstream-to`/`--unset-upstream`, `-c`/`-C` (branch copy), `--merged`/`--no-merged` (merge-status filter), `--sort` (custom sort keys), `-f`/`--force`

**switch flags** (FR-100 through FR-104):

- [ ] T043 [P] [US3] Add missing switch flags in `crates/git-cli/src/commands/switch.rs`: `-c` as short for `--create` (depends on T035 global flag fix), `--guess`/`--no-guess` (DWIM remote tracking), `-q`/`--quiet`, `-m`/`--merge`, `--conflict`, `--orphan`, `-t`/`--track`/`--no-track`

**checkout flags** (FR-110 through FR-112):

- [ ] T044 [P] [US3] Add missing checkout flags in `crates/git-cli/src/commands/checkout.rs`: `-q`, `-m`, `--conflict`, `--ours`/`--theirs`, `-t`/`--track`, `--orphan`; `-p`/`--patch` flag (wires to interactive hunk selector in Phase 8)

**merge flags** (FR-120 through FR-124):

- [ ] T045 [US3] Add missing merge flags in `crates/git-cli/src/commands/merge.rs`: rename `--cont` to `--continue` (FR-120), `--strategy`/`-s` and `-X`/`--strategy-option` (wire to `git-merge` strategies), `-v`/`--verbose`, `-q`/`--quiet`, `--stat`/`--no-stat`, `-e`/`--edit`, `--allow-unrelated-histories`, `-s`/`--signoff`, `--verify`/`--no-verify`

**rebase flags** (FR-130 through FR-135):

- [ ] T046 [US3] Add missing rebase flags in `crates/git-cli/src/commands/rebase.rs`: `-q`/`--quiet`, `-v`/`--verbose`, `--signoff`, `-f`/`--force-rebase`, `--autosquash`/`--no-autosquash`, `--autostash`/`--no-autostash`, `--update-refs`, `-x`/`--exec`, `--root`, `-s`/`--strategy`, `-X`/`--strategy-option`; `-i`/`--interactive` flag (wires to interactive rebase in Phase 8)

**cherry-pick flags** (FR-140 through FR-143):

- [ ] T047 [P] [US3] Add missing cherry-pick flags in `crates/git-cli/src/commands/cherry_pick.rs`: `-m`/`--mainline` (for merge commits), `-x` (append cherry-picked note), `-s`/`--signoff`, `--ff`, `--strategy`/`-X`, `--allow-empty`/`--allow-empty-message`

**revert flags** (FR-150 through FR-151):

- [ ] T048 [P] [US3] Add missing revert flags in `crates/git-cli/src/commands/revert.rs`: `-m`/`--mainline` (for reverting merge commits), `-s`/`--signoff`, `--strategy`/`-X`

**tag flags** (FR-160 through FR-162):

- [ ] T049 [P] [US3] Add missing tag flags in `crates/git-cli/src/commands/tag.rs`: `-s`/`--sign` and `-u`/`--local-user` (GPG signing — wire to `git-repository/src/gpg.rs`), `-F` (read message from file), `--sort`, `--contains`/`--no-contains`, `--merged`/`--no-merged`, `--format`, `--points-at`

**stash flags** (FR-170 through FR-175):

- [ ] T050 [US3] Add missing stash flags in `crates/git-cli/src/commands/stash.rs`: `--keep-index`/`-k` (keep staged), `--staged`/`-S` (stash only staged), `branch` subcommand (create branch from stash), `create`/`store` subcommands, fix `drop stash@{N}` for any valid index N (not just 0); `-p`/`--patch` flag (wires to interactive hunk selector in Phase 8)

**reset flags** (FR-180):

- [ ] T051 [P] [US3] Add missing reset flags in `crates/git-cli/src/commands/reset.rs`: `--keep`, `-q`/`--quiet`, `-N`/`--no-refresh`; `-p`/`--patch` flag (wires to interactive hunk selector in Phase 8)

**restore flags** (FR-185):

- [ ] T052 [P] [US3] Add missing restore flags in `crates/git-cli/src/commands/restore.rs`: `--overlay`/`--no-overlay`, `--conflict`, `--ours`/`--theirs`, `--pathspec-from-file`; `-p`/`--patch` flag (wires to interactive hunk selector in Phase 8)

**clean flags** (FR-190):

- [ ] T053 [P] [US3] Add missing clean flags in `crates/git-cli/src/commands/clean.rs`: `-i` (interactive clean), `-e`/`--exclude` (additional exclude patterns)

**blame flags** (FR-195 through FR-197):

- [ ] T054 [P] [US3] Add missing blame flags in `crates/git-cli/src/commands/blame.rs`: `-b`, `--root`, `-f`/`--show-name`, `-p`/`--line-porcelain`, `-c`, `-t`, `-l`, `-s`, `--incremental`, full `-C` (detect copies) and `-M` (detect moves) implementation beyond stubs, `--ignore-rev`/`--ignore-revs-file`

**shortlog flags** (FR-200):

- [ ] T055 [P] [US3] Add missing shortlog flags in `crates/git-cli/src/commands/shortlog.rs`: `-c`/`--committer`, `-w` (line wrapping), `--group`

**reflog subcommands** (FR-205 through FR-207):

- [ ] T056 [P] [US3] Implement missing reflog subcommands in `crates/git-cli/src/commands/reflog.rs`: `expire` (remove old entries), `delete` (delete specific entries), `--date` flag for `show`, `exists` subcommand

**clone flags** (FR-210):

- [ ] T057 [P] [US3] Add missing clone flags in `crates/git-cli/src/commands/clone.rs`: `-v`/`--verbose`, `--mirror`, `-l`/`--local`, `--single-branch`/`--no-single-branch`, `--recurse-submodules`, `--shallow-since`, `--shallow-exclude`, `--no-tags`, `--filter`, `--origin`, `--sparse`

**fetch flags** (FR-215):

- [ ] T058 [P] [US3] Add missing fetch flags in `crates/git-cli/src/commands/fetch.rs`: `-v`, `-f`/`--force`, `--dry-run`, `-j`/`--jobs`, `--shallow-since`/`--shallow-exclude`, `--unshallow`/`--deepen`, `--recurse-submodules`, `--set-upstream`

**pull flags** (FR-220):

- [ ] T059 [P] [US3] Add missing pull flags in `crates/git-cli/src/commands/pull.rs`: `-v`, `--stat`/`--no-stat`, `--log`/`--no-log`, `--squash`, `--commit`/`--no-commit`, `-e`/`--edit`, `--ff`/`--no-ff`, `--strategy`/`-X`, `--all`, `--depth`, `--tags`, `-p`/`--prune`, `--autostash`

**push flags** (FR-225):

- [ ] T060 [P] [US3] Add missing push flags in `crates/git-cli/src/commands/push.rs`: `--all`, `--mirror`, `--thin`/`--no-thin`, `--signed`, `--recurse-submodules`

**remote subcommands** (FR-230):

- [ ] T061 [P] [US3] Implement missing remote subcommands in `crates/git-cli/src/commands/remote.rs`: `set-head`, `prune`, `update`, `set-branches`, `get-url`

**config flags** (FR-235):

- [ ] T062 [US3] Add missing config flags in `crates/git-cli/src/commands/config.rs`: `--system`, `-f`/`--file`, `--get-all`, `--get-regexp`, `--replace-all`, `--add`, `--unset-all`, `--rename-section`, `--remove-section`, `-e`/`--edit`, `--type`/`--bool`/`--int`/`--path`, `-z`, `--name-only`, `--includes`

**describe flags** (FR-240):

- [ ] T063 [P] [US3] Add missing describe flags in `crates/git-cli/src/commands/describe.rs`: `--contains`, `--all`, `--first-parent`, `--exact-match`, `--candidates`, `--match`/`--exclude`

**format-patch flags** (FR-245):

- [ ] T064 [P] [US3] Add missing format-patch flags in `crates/git-cli/src/commands/format_patch.rs`: `-s`/`--signoff`, `-N`/`--no-numbered`, `-k`, `--to`, `--cc`, `--from`, `--in-reply-to`, `--base`, `-v`/`--reroll-count`, `--range-diff`

**gc flags** (FR-250):

- [ ] T065 [P] [US3] Add missing gc flags in `crates/git-cli/src/commands/gc.rs`: `--cruft` for cruft pack generation

**Checkpoint**: All existing commands have complete flag parity with git. `gitr <cmd> -h` matches `git <cmd> -h` for every command.

---

## Phase 6: User Story 4 — Missing Plumbing and Porcelain Commands (Priority: P2)

**Goal**: All 18+ missing commands are implemented and produce output matching git's format.

**Independent Test**: Run `gitr merge-base HEAD HEAD~1` and verify it matches `git merge-base HEAD HEAD~1`; run `echo "  hello  " | gitr stripspace` and verify identical whitespace handling; run `gitr ls-remote origin` and verify ref listing matches git.

### Implementation for User Story 4

**Simple/leaf commands** (no complex dependencies):

- [ ] T066 [P] [US4] Implement `stripspace` command in `crates/git-cli/src/commands/stripspace.rs` — read stdin, strip unnecessary whitespace; `-s`/`--strip-comments` and `-c`/`--comment-lines` flags; register in `commands/mod.rs` (FR-311)
- [ ] T067 [P] [US4] Implement `count-objects` command in `crates/git-cli/src/commands/count_objects.rs` — count unpacked objects in `.git/objects/`, report disk consumption; `-v`/`--verbose` and `-H`/`--human-readable` flags; uses `git-odb`; register in `commands/mod.rs` (FR-302)
- [ ] T068 [P] [US4] Implement `fmt-merge-msg` command in `crates/git-cli/src/commands/fmt_merge_msg.rs` — produce merge commit message from branch names; `-m <message>`, `--log[=<n>]`, `--no-log`, `-F <file>` flags; register in `commands/mod.rs` (FR-310)

**Diff plumbing commands** (use `git-diff`):

- [ ] T069 [P] [US4] Implement `diff-files` command in `crates/git-cli/src/commands/diff_files.rs` — compare working tree files against index; `-p`, `-q`, `--` path filters; register in `commands/mod.rs` (FR-303)
- [ ] T070 [P] [US4] Implement `diff-index` command in `crates/git-cli/src/commands/diff_index.rs` — compare tree to working tree or index; `--cached`, `-p`, `<tree-ish>`, path filters; register in `commands/mod.rs` (FR-303)
- [ ] T071 [P] [US4] Implement `diff-tree` command in `crates/git-cli/src/commands/diff_tree.rs` — compare two tree objects; `-r`, `-p`, `--name-only`, `--name-status`, path filters; register in `commands/mod.rs` (FR-303)

**Revwalk-based commands** (use `git-revwalk`):

- [ ] T072 [P] [US4] Implement `merge-base` command in `crates/git-cli/src/commands/merge_base.rs` — find common ancestors; `--all`, `--octopus`, `--is-ancestor`, `--fork-point` flags; wire to `git-revwalk/src/merge_base.rs`; register in `commands/mod.rs` (FR-305)
- [ ] T073 [P] [US4] Implement `cherry` command in `crates/git-cli/src/commands/cherry.rs` — find commits not applied upstream; `-v` flag; wire to `git-revwalk/src/cherry.rs`; register in `commands/mod.rs` (FR-301)
- [ ] T074 [P] [US4] Implement `name-rev` command in `crates/git-cli/src/commands/name_rev.rs` — find symbolic names for revisions; `--tags`, `--refs=<pattern>`, `--no-undefined`, `--always` flags; uses `git-revwalk` + `git-ref`; register in `commands/mod.rs` (FR-307)
- [ ] T075 [P] [US4] Implement `whatchanged` command in `crates/git-cli/src/commands/whatchanged.rs` — show logs with diff per commit; supports all log options plus diff output; uses `git-revwalk` + `git-diff`; register in `commands/mod.rs` (FR-312)
- [ ] T076 [P] [US4] Implement `range-diff` command in `crates/git-cli/src/commands/range_diff.rs` — compare two commit ranges; `<range1> <range2>` args; uses `git-revwalk` + `git-diff` for patch comparison; register in `commands/mod.rs` (FR-308)

**Merge-related commands** (use `git-merge`):

- [ ] T077 [P] [US4] Implement `merge-file` command in `crates/git-cli/src/commands/merge_file.rs` — three-way file merge; `-p`/`--stdout`, `--diff3`, `-L <label>` flags; uses `git-merge`; register in `commands/mod.rs` (FR-306)
- [ ] T078 [P] [US4] Implement `merge-tree` command in `crates/git-cli/src/commands/merge_tree.rs` — merge without touching index/working tree; `--write-tree` flag; uses `git-merge`; register in `commands/mod.rs` (FR-306)
- [ ] T079 [P] [US4] Implement `rerere` command in `crates/git-cli/src/commands/rerere.rs` — reuse recorded resolution; subcommands `clear`, `forget <pathspec>`, `diff`, `status`, `gc`; wire to `git-merge/src/rerere.rs`; register in `commands/mod.rs` (FR-315)

**Patch/apply commands**:

- [ ] T080 [P] [US4] Implement `apply` command in `crates/git-cli/src/commands/apply.rs` — apply patch to files/index; reads from stdin or file; standard apply options; uses `git-merge` apply module; register in `commands/mod.rs` (FR-300)

**Index commands** (use `git-index`/`git-odb`):

- [ ] T081 [P] [US4] Implement `read-tree` command in `crates/git-cli/src/commands/read_tree.rs` — read tree info into index; `-m`, `-u`/`--reset`, `--prefix=<prefix>` flags; uses `git-index` + `git-odb`; register in `commands/mod.rs` (FR-309)
- [ ] T082 [P] [US4] Implement `sparse-checkout` command in `crates/git-cli/src/commands/sparse_checkout.rs` — manage sparse checkout; subcommands `init`, `set`, `add`, `reapply`, `disable`, `list`; wire to `git-index/src/sparse.rs`; register in `commands/mod.rs` (FR-314)

**Transport/remote commands** (use `git-transport`):

- [ ] T083 [P] [US4] Implement `ls-remote` command in `crates/git-cli/src/commands/ls_remote.rs` — list remote refs; `--heads`, `--tags`, `--refs`, `-q` flags and pattern filters; uses `git-transport`; register in `commands/mod.rs` (FR-304)

**Maintenance command** (use `git-odb`/`git-pack`):

- [ ] T084 [P] [US4] Implement `maintenance` command in `crates/git-cli/src/commands/maintenance.rs` — run maintenance tasks; `run` subcommand with `--task=<task>`, `--auto`, `--schedule=<frequency>` flags; tasks: gc, commit-graph, prefetch, loose-objects, incremental-repack, pack-refs; register in `commands/mod.rs` (FR-313)

**External tool delegation**:

- [ ] T085 [P] [US4] Implement `difftool` command in `crates/git-cli/src/commands/difftool.rs` — launch external diff tool; `--tool=<tool>`, `--no-prompt` flags; reads `diff.tool` config; uses `std::process::Command`; register in `commands/mod.rs` (FR-316)
- [ ] T086 [P] [US4] Implement `request-pull` command in `crates/git-cli/src/commands/request_pull.rs` — generate summary of pending changes; `-p` flag, `<start> <url> [<end>]` args; uses `git-revwalk` + `git-diff`; register in `commands/mod.rs` (FR-317)

**Checkpoint**: All 21 new commands are implemented and produce git-compatible output.

---

## Phase 7: User Story 5 — Behavioral and Data Format Parity (Priority: P2)

**Goal**: Advanced git features (.gitattributes, .mailmap, config includes, hooks, merge strategies, GPG, credentials, diff3 conflict style) all work identically to git.

**Independent Test**: Create `.gitattributes` with `*.txt text eol=crlf` and verify line-ending normalization; add `[include]` to `.gitconfig` and verify `gitr config --list` includes values; run merge with `--strategy=ours` and verify tree matches.

### Implementation for User Story 5

- [ ] T087 [P] [US5] Integrate `.gitattributes` EOL normalization into `git add` and `git checkout` paths in `crates/git-index/src/` — LF→CRLF on checkout, CRLF→LF on add, based on `text`/`eol` attributes; implement `text=auto` binary detection (FR-400)
- [ ] T088 [P] [US5] Implement clean/smudge filter execution in `crates/git-index/src/attributes.rs` — resolve `filter.<name>.clean` and `filter.<name>.smudge` from config, execute via `std::process::Command`, pipe file contents through filter (FR-400)
- [ ] T089 [P] [US5] Integrate diff driver selection from gitattributes into `git-diff` — when `diff=<driver>` is set, use `diff.<driver>.command` from config or apply driver-specific behavior; wire `AttributeStack.diff_driver()` into diff pipeline (FR-400)
- [ ] T090 [US5] Verify config `[include]` and `[includeIf]` E2E parity — write tests comparing `gitr config --list` vs `git config --list` with include directives; verify circular include detection, missing files silently ignored, relative path resolution, `gitdir:`, `gitdir/i:`, `onbranch:`, `hasconfig:` conditions (FR-407)
- [ ] T091 [P] [US5] Integrate `Mailmap` into log, shortlog, and blame formatters in `crates/git-cli/src/commands/` — when `--use-mailmap` flag is passed or `log.mailmap=true` config is set, apply `Mailmap::lookup()` to author/committer identities before display (FR-406)
- [ ] T092 [P] [US5] Integrate `GpgSigner` into commit creation: add `-S`/`--gpg-sign` flag to commit command, sign commit object and embed signature in `gpgsig` header in `crates/git-cli/src/commands/commit.rs` (FR-402)
- [ ] T093 [P] [US5] Integrate `GpgSigner` into tag creation: add `-s`/`--sign` flag behavior to tag command, sign tag object and append signature in `crates/git-cli/src/commands/tag.rs` (FR-402)
- [ ] T094 [US5] Wire complete hook lifecycle into all commands: `prepare-commit-msg` and `post-commit` in commit, `pre-rebase` in rebase, `post-rewrite` in rebase/cherry-pick/revert, `post-checkout` in checkout/switch, `post-merge` in merge/pull, `pre-push` in push, `pre-auto-gc` in gc — using `HookRunner` from `git-repository/src/hooks.rs` (FR-401)
- [ ] T095 [P] [US5] Wire merge strategy selection into merge command: `--strategy=recursive|ort|octopus|ours|subtree` flag dispatches to correct `MergeStrategyType`; implement `-X`/`--strategy-option` pass-through to strategy implementations in `crates/git-cli/src/commands/merge.rs` + `crates/git-merge/` (FR-403)
- [ ] T096 [P] [US5] Integrate `CredentialHelper` into transport layer: before HTTP/SSH auth in fetch/push/clone, call `credential_helper.fill()` to obtain credentials; call `approve()`/`reject()` after auth success/failure in `crates/git-transport/` (FR-408)
- [ ] T097 [P] [US5] Implement alternate object database support: read `$GIT_DIR/objects/info/alternates`, resolve paths, include alternate object stores in object lookup chain in `crates/git-odb/` (FR-409)
- [ ] T098 [P] [US5] Implement `merge.conflictStyle=diff3` support: when configured, produce conflict markers with `|||||||` base section between `<<<<<<<` and `=======` markers in `crates/git-merge/` (FR-411)
- [ ] T099 [P] [US5] Implement configurable rename/copy detection thresholds: `-M<n>` (move threshold percentage), `-C<n>` (copy threshold percentage) flags in diff/log/blame commands — pass threshold to diff engine in `crates/git-diff/` (FR-404)
- [ ] T100 [P] [US5] Implement shallow clone operation support: `--deepen`, `--unshallow`, `--shallow-since`, `--shallow-exclude` flags in clone/fetch — manage `.git/shallow` file and negotiate shallow boundaries in `crates/git-transport/` (FR-405)

**Checkpoint**: All behavioral features pass E2E comparison with git.

---

## Phase 8: User Story 6 — Interactive Modes (Priority: P3)

**Goal**: Interactive patch staging (`add -p`) and interactive rebase (`rebase -i`) work identically to git's interactive modes.

**Independent Test**: Run `gitr add -p` on a file with 3 hunks and verify y/n/s/q selection works; run `gitr rebase -i HEAD~3` and verify editor opens with pick/squash/edit options.

### Implementation for User Story 6

- [ ] T101 [US6] Implement `InteractiveHunkSelector` struct in `crates/git-cli/src/interactive.rs` — open `/dev/tty` for user input; `select_hunks()` method presents each hunk with context and accepts commands: `y` (yes), `n` (no), `q` (quit), `a` (all), `d` (done), `s` (split), `e` (edit), `?` (help); `split_hunk()` method for subdividing hunks; prompt format matches git: `(N/M) Stage this hunk [y,n,q,a,d,s,e,?]?` (FR-412)
- [ ] T102 [US6] Wire `InteractiveHunkSelector` into `add -p` in `crates/git-cli/src/commands/add.rs` — generate diff hunks, present for selection, stage selected hunks to index (FR-412)
- [ ] T103 [P] [US6] Wire `InteractiveHunkSelector` into `reset -p` in `crates/git-cli/src/commands/reset.rs` — present staged hunks for individual unstaging (FR-412)
- [ ] T104 [P] [US6] Wire `InteractiveHunkSelector` into `stash -p` in `crates/git-cli/src/commands/stash.rs` — present hunks for selective stashing (FR-412)
- [ ] T105 [P] [US6] Wire `InteractiveHunkSelector` into `checkout -p` and `restore -p` in `crates/git-cli/src/commands/checkout.rs` and `crates/git-cli/src/commands/restore.rs` — present hunks for selective checkout/restore (FR-412)
- [ ] T106 [US6] Implement interactive rebase (`-i`/`--interactive`) in `crates/git-cli/src/commands/rebase.rs` — generate `.git/rebase-merge/git-rebase-todo` with `pick <hash> <subject>` lines; open editor via `EditorConfig`; parse edited todo (pick, reword, edit, squash, fixup, drop, exec, break, label, reset, merge); execute via sequencer; support `--autosquash` (reorder fixup!/squash!); support `--autostash`; state files for `--continue`/`--abort`/`--skip` (FR-130)

**Checkpoint**: All interactive modes work identically to git.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Validation, cleanup, and E2E testing across all user stories

- [ ] T107 Run `cargo clippy --workspace` and fix all warnings introduced by phase 3 changes
- [ ] T108 Run `cargo test --workspace` and fix any failing tests
- [ ] T109 [P] Create E2E color and pager parity test suite in `tests/e2e_color_pager.rs` — compare `gitr` vs `git` output for colored diff, log, status, branch, grep; verify pager invocation behavior
- [ ] T110 [P] Create E2E missing flags parity test suite in `tests/e2e_missing_flags.rs` — for each command with new flags, compare `gitr <cmd> <flag>` vs `git <cmd> <flag>` output
- [ ] T111 [P] Create E2E new commands parity test suite in `tests/e2e_new_commands.rs` — for each of the 21 new commands, compare `gitr <cmd>` vs `git <cmd>` output for standard inputs
- [ ] T112 [P] Create E2E behavioral parity test suite in `tests/e2e_behavioral_parity.rs` — test gitattributes EOL, config includes, mailmap, hooks, merge strategies, GPG signing, credential helpers, diff3 conflict style
- [ ] T113 Run quickstart.md validation scenarios: side-by-side comparison of gitr vs git for all documented quick test commands

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories
- **US1 Color/Pager (Phase 3)**: Depends on T003, T004, T005 (color infra)
- **US2 Top-Level CLI (Phase 4)**: Depends on Phase 2 completion
- **US3 Missing Flags (Phase 5)**: Depends on Phase 2 (many flags wire to foundational modules); can begin in parallel with US1/US2 for flags not needing color/pager
- **US4 New Commands (Phase 6)**: Depends on Phase 2 (commands use foundational library modules); can proceed in parallel with US1–US3
- **US5 Behavioral Parity (Phase 7)**: Depends on Phase 2 foundational modules; can proceed in parallel with US3/US4
- **US6 Interactive Modes (Phase 8)**: Depends on T101 (hunk selector), T010 (editor config); largely independent of other stories
- **Polish (Phase 9)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Foundational T003-T005 → US1 tasks. No dependency on other stories.
- **US2 (P1)**: Foundational complete → US2 tasks. No dependency on other stories. T035 (preprocess_args) unblocks `switch -c` in US3.
- **US3 (P1)**: Foundational complete → US3 tasks. Some flags depend on US5 library modules (GPG for `-S`, hooks for `--no-verify`) but flag *acceptance* (clap args) can proceed immediately.
- **US4 (P2)**: Foundational complete → US4 tasks. All 21 commands can be implemented in parallel.
- **US5 (P2)**: Foundational T007-T017 → US5 integration tasks. Can proceed in parallel with US3/US4.
- **US6 (P3)**: T101 (hunk selector) → T102-T105 (wire into commands). T106 (interactive rebase) depends on T010 (editor) and T046 (rebase flags).

### Within Each User Story

- Shared infrastructure (clap arg groups, config loading) before individual commands
- Library modules before CLI integration
- Core implementation before edge-case handling

### Parallel Opportunities

- All Phase 2 foundational tasks (T003–T017) can run in parallel (different files/crates)
- All Phase 6 new commands (T066–T086) can run in parallel (each is a separate `.rs` file)
- US3 flag tasks for commands in different files can run in parallel
- US5 integration tasks across different crates can run in parallel
- US6 wire-ups (T103–T105) can run in parallel after T101 completes

---

## Parallel Example: Phase 2 (Foundational)

```bash
# All foundational library modules in different crates — fully parallel:
Task: "Implement ColorSlot enum in crates/git-utils/src/color.rs"          # T003
Task: "Implement ColorConfig struct in crates/git-utils/src/color.rs"      # T004
Task: "Implement colored diff module in crates/git-diff/src/color.rs"      # T005
Task: "Implement pickaxe module in crates/git-diff/src/pickaxe.rs"         # T006
Task: "Implement Mailmap in crates/git-utils/src/mailmap.rs"               # T007
Task: "Extend HookRunner in crates/git-repository/src/hooks.rs"            # T008
Task: "Implement GpgSigner in crates/git-repository/src/gpg.rs"            # T009
Task: "Implement EditorConfig in crates/git-repository/src/editor.rs"      # T010
Task: "Extend AttributeStack in crates/git-index/src/attributes.rs"        # T011
Task: "Implement sparse checkout in crates/git-index/src/sparse.rs"        # T012
Task: "Implement RerereDatabase in crates/git-merge/src/rerere.rs"         # T013
Task: "Implement Octopus strategy in crates/git-merge/src/strategy/"       # T014
Task: "Implement merge_base in crates/git-revwalk/src/merge_base.rs"       # T015
Task: "Implement cherry filter in crates/git-revwalk/src/cherry.rs"        # T016
Task: "Extend CredentialHelper in crates/git-transport/src/credential.rs"  # T017
```

## Parallel Example: Phase 6 (New Commands)

```bash
# All new commands are separate files — fully parallel:
Task: "Implement stripspace in crates/git-cli/src/commands/stripspace.rs"       # T066
Task: "Implement count-objects in crates/git-cli/src/commands/count_objects.rs"  # T067
Task: "Implement merge-base in crates/git-cli/src/commands/merge_base.rs"       # T072
Task: "Implement diff-tree in crates/git-cli/src/commands/diff_tree.rs"         # T071
Task: "Implement ls-remote in crates/git-cli/src/commands/ls_remote.rs"         # T083
# ... (all 21 commands simultaneously)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (T003–T017) — CRITICAL, blocks all stories
3. Complete Phase 3: User Story 1 (Color & Pager)
4. **STOP and VALIDATE**: `diff <(git diff --color=always) <(gitr diff --color=always)` should be identical
5. This alone makes gitr visually indistinguishable from git for daily use

### Incremental Delivery

1. Setup + Foundational → Library infrastructure ready
2. US1 (Color/Pager) → Visual parity — gitr looks like git (MVP!)
3. US2 (Top-Level CLI) → Entry point parity — `--version`, `--help`, global flags
4. US3 (Missing Flags) → Functional parity — all flags work on all commands
5. US4 (New Commands) → Command parity — no missing commands
6. US5 (Behavioral) → Deep parity — gitattributes, mailmap, hooks, GPG, etc.
7. US6 (Interactive) → Full parity — `add -p`, `rebase -i`
8. Polish → Validated with E2E test suite

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: US1 (Color/Pager) + US2 (Top-Level CLI)
   - Developer B: US3 (Missing Flags — core commands: commit, diff, log, merge, rebase)
   - Developer C: US3 (Missing Flags — remaining commands: stash, blame, clone, fetch, push, etc.)
   - Developer D: US4 (New Commands — all 21 in parallel)
   - Developer E: US5 (Behavioral Parity — gitattributes, hooks, GPG, etc.)
3. After US1–US5: Developer F takes US6 (Interactive Modes)
4. Everyone: Polish phase with E2E tests

---

## Notes

- [P] tasks = different files, no dependencies on in-progress tasks
- [Story] label maps each task to its user story for traceability
- Each user story is independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently
- All 21 new commands follow the established pattern: thin CLI module in `git-cli/src/commands/` dispatching to library crates
- No new crates — all work extends existing 16-crate workspace
- Existing infrastructure (color.rs, pager.rs, attributes.rs, include.rs) is leveraged — many "new" tasks are integration/extension, not green-field

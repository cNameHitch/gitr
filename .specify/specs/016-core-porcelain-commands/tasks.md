# Tasks: Core Porcelain Commands

**Input**: Design documents from `specs/016-core-porcelain-commands/`
**Prerequisites**: All library crates, plumbing commands (spec 015)

## Phase 1: User Story 1 - Repository Creation (Priority: P1)

- [X] T001 [US1] Implement `init` in `src/commands/init.rs` — --bare, --initial-branch, --template
- [X] T002 [US1] Implement `clone` in `src/commands/clone.rs` — fetch + checkout, --depth, --bare, --branch
- [X] T003 [US1] Add init/clone integration tests

**Checkpoint**: Can create and clone repositories

---

## Phase 2: User Story 2 - Staging (Priority: P1)

- [X] T004 [P] [US2] Implement `add` in `src/commands/add.rs` — -A, -u, pathspec, .gitignore respect
- [X] T005 [P] [US2] Implement `rm` in `src/commands/rm.rs` — --cached, -f, -r
- [X] T006 [P] [US2] Implement `mv` in `src/commands/mv.rs`
- [X] T007 [US2] Implement `status` in `src/commands/status.rs` — --short, --branch, --porcelain, long format
- [X] T008 [US2] Implement `restore` in `src/commands/restore.rs` — --staged, --worktree, --source
- [X] T009 [US2] Add staging integration tests

**Checkpoint**: Full staging workflow works

---

## Phase 3: User Story 3 - Commit (Priority: P1)

- [X] T010 [US3] Implement `commit` in `src/commands/commit.rs` — -m, -a, --amend, --allow-empty, editor launch
- [ ] T011 [US3] Implement commit hook support (pre-commit, commit-msg, post-commit)
- [X] T012 [US3] Add commit integration tests

**Checkpoint**: Commits created correctly

---

## Phase 4: User Story 4 - Branching (Priority: P1)

- [X] T013 [P] [US4] Implement `branch` in `src/commands/branch.rs` — -d, -D, -m, -M, -a, -r, --list, --format
- [X] T014 [P] [US4] Implement `switch` in `src/commands/switch.rs` — --create, --force-create, --detach, --force
- [X] T015 [US4] Implement `checkout` in `src/commands/checkout.rs` — dispatch to switch/restore
- [X] T016 [US4] Implement `merge` in `src/commands/merge.rs` — --no-ff, --ff-only, --squash, --abort, --continue
- [X] T017 [US4] Add branching and merge integration tests

**Checkpoint**: Branch management and merge work

---

## Phase 5: User Story 5 - Remote Operations (Priority: P1)

- [X] T018 [P] [US5] Implement `remote` in `src/commands/remote.rs` — add, remove, rename, set-url, -v
- [X] T019 [P] [US5] Implement `fetch` in `src/commands/fetch.rs` — --all, --prune, --depth, --tags
- [X] T020 [US5] Implement `pull` in `src/commands/pull.rs` — fetch + merge/rebase
- [X] T021 [US5] Implement `push` in `src/commands/push.rs`:
  - Parse CLI args: remote, refspecs, -f/--force, --force-with-lease[=ref:oid], --delete, --tags, -u/--set-upstream, --atomic, --dry-run, --push-option, --no-verify, --verbose, --progress
  - Resolve remote name (default from `branch.<current>.remote` or "origin")
  - Resolve push URL (check `remote.<name>.pushUrl`, then `remote.<name>.url`, apply `url.<base>.pushInsteadOf` rewrites)
  - Resolve refspecs: explicit args → `remote.<name>.push` config → `push.default` logic (via `resolve_push_refspecs`)
  - Run pre-push hook (unless --no-verify): pipe ref update info to hook stdin
  - Open transport connection to remote's `receive-pack` service
  - Call protocol-level `push()` with computed ref updates and options
  - If `--set-upstream`, configure `branch.<name>.remote` and `branch.<name>.merge`
  - Display per-ref results and progress output
  - If `push.followTags`, include reachable annotated tags in ref updates
- [X] T021a [US5] Implement `resolve_push_refspecs` — handle all `push.default` modes (nothing, current, upstream, simple, matching)
- [ ] T021b [US5] Implement pre-push hook integration — format: `<local-ref> <local-oid> <remote-ref> <remote-oid>\n` per ref, abort push if hook exits non-zero
- [X] T022 [US5] Add remote operation integration tests — test push with: no args (push.default), explicit refspec, force, force-with-lease, delete, tags, set-upstream, atomic, pre-push hook abort, empty push (up-to-date)

**Checkpoint**: Full remote workflow works

---

## Phase 6: User Story 6 - History Management (Priority: P2)

- [X] T023 [P] [US6] Implement `rebase` in `src/commands/rebase.rs` — --onto, --abort, --continue, --skip
- [X] T024 [P] [US6] Implement `reset` in `src/commands/reset.rs` — --soft, --mixed, --hard
- [X] T025 [P] [US6] Implement `tag` in `src/commands/tag.rs` — -a, -d, -l, -m, -v
- [X] T026 [P] [US6] Implement `stash` in `src/commands/stash.rs` — push, pop, list, show, drop, apply
- [X] T027 [P] [US6] Implement `clean` in `src/commands/clean.rs` — -f, -d, -n, -x
- [ ] T028 [US6] Add interactive rebase support in rebase.rs
- [X] T029 [US6] Add history management integration tests

**Checkpoint**: All history management commands work

---

## Phase 7: Interactive Features

- [ ] T030 Implement `git add -p` (interactive patch staging)
- [ ] T031 Implement `git rebase -i` (interactive rebase with editor)

---

## Phase 8: Polish

- [X] T032 [P] Run `cargo clippy` and fix warnings
- [ ] T033 Ensure all exit codes match C git
- [X] T034 Run full integration test suite

---

## Dependencies & Execution Order

- Phases 1-5 are sequential by priority (can overlap once each is started)
- Phase 6 depends on Phase 4 (branching infrastructure)
- Within each phase, [P] tasks can run in parallel
- Phase 7 depends on Phases 2 and 6
- Phase 8 depends on all prior phases

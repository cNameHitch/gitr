# Feature Specification: Core Porcelain Commands

**Feature Branch**: `016-core-porcelain-commands`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: All library crates (001-014), 015-plumbing-commands

## User Scenarios & Testing

### User Story 1 - Repository Creation (Priority: P1)

As a git user, I need `git init` and `git clone` to create repositories.

**Why this priority**: Entry point for all git usage.

**Acceptance Scenarios**:

1. **Given** `git init myrepo`, **When** run, **Then** a new repository is created.
2. **Given** `git clone <url> dest`, **When** run, **Then** the repository is cloned with all refs and history.
3. **Given** `git clone --depth=1`, **When** run, **Then** a shallow clone is created.
4. **Given** `git clone --bare`, **When** run, **Then** a bare repository is created.

---

### User Story 2 - Staging Workflow (Priority: P1)

As a git user, I need `git add`, `git rm`, `git mv`, `git status`, and `git restore` to manage the staging area.

**Why this priority**: Daily workflow commands.

**Acceptance Scenarios**:

1. **Given** modified files, **When** `git add .`, **Then** all changes are staged.
2. **Given** `git add -p`, **When** run, **Then** interactive patch staging works.
3. **Given** `git rm file`, **When** run, **Then** the file is removed from index and working tree.
4. **Given** `git mv old new`, **When** run, **Then** the file is renamed in the working tree and index.
5. **Given** staged and unstaged changes, **When** `git status`, **Then** both are displayed with correct labels.
6. **Given** `git status --short`, **When** run, **Then** compact two-column format is shown.
7. **Given** `git restore file`, **When** run, **Then** the working tree file is restored from index.
8. **Given** `git restore --staged file`, **When** run, **Then** the staged changes are unstaged.

---

### User Story 3 - Commit Workflow (Priority: P1)

As a git user, I need `git commit` to create commits.

**Acceptance Scenarios**:

1. **Given** staged changes, **When** `git commit -m "msg"`, **Then** a commit is created with the staged tree.
2. **Given** `git commit -a`, **When** run, **Then** all tracked modified files are auto-staged and committed.
3. **Given** `git commit --amend`, **When** run, **Then** the last commit is replaced.
4. **Given** `git commit` with no `-m`, **When** run, **Then** the editor opens for the commit message.

---

### User Story 4 - Branch Management (Priority: P1)

As a git user, I need `git branch`, `git checkout`, `git switch`, and `git merge` for branch operations.

**Acceptance Scenarios**:

1. **Given** `git branch feature`, **When** run, **Then** a new branch is created at HEAD.
2. **Given** `git branch -d feature`, **When** run, **Then** the branch is deleted (if fully merged).
3. **Given** `git branch -D feature`, **When** run, **Then** the branch is force-deleted.
4. **Given** `git switch feature`, **When** run, **Then** HEAD is updated and working tree is checked out.
5. **Given** `git switch -c newbranch`, **When** run, **Then** a new branch is created and checked out.
6. **Given** `git checkout feature`, **When** run, **Then** works as switch (legacy compatibility).
7. **Given** `git merge feature`, **When** run, **Then** the feature branch is merged into current branch.
8. **Given** `git merge --no-ff`, **When** run, **Then** a merge commit is always created.

---

### User Story 5 - Remote Operations (Priority: P1)

As a git user, I need `git fetch`, `git pull`, `git push`, and `git remote` for remote collaboration.

**Acceptance Scenarios**:

1. **Given** `git fetch origin`, **When** run, **Then** new objects and refs are downloaded.
2. **Given** `git pull`, **When** run, **Then** fetch + merge (or rebase per config) is performed.
3. **Given** `git push origin main`, **When** run, **Then** local commits are pushed to remote.
4. **Given** `git push` with no arguments, **When** run, **Then** the `push.default` config determines what to push and `branch.<name>.remote` determines where.
5. **Given** `git push --force-with-lease origin main`, **When** the remote ref has been updated by someone else, **Then** the push is rejected.
6. **Given** `git push --atomic origin main feature`, **When** one ref fails, **Then** no refs are updated.
7. **Given** `git push -u origin newbranch`, **When** run, **Then** the branch is pushed and upstream tracking is configured.
8. **Given** `git remote add origin <url>`, **When** run, **Then** a new remote is configured.
9. **Given** `git remote -v`, **When** run, **Then** all remotes with URLs are listed.

---

### User Story 6 - Rebase and Reset (Priority: P2)

As a git user, I need `git rebase`, `git reset`, `git tag`, `git stash`, and `git clean` for history management.

**Acceptance Scenarios**:

1. **Given** `git rebase main`, **When** run, **Then** current branch is rebased onto main.
2. **Given** `git rebase -i HEAD~3`, **When** run, **Then** interactive rebase editor opens.
3. **Given** `git reset --soft HEAD~1`, **When** run, **Then** HEAD moves back, index/worktree unchanged.
4. **Given** `git reset --hard HEAD~1`, **When** run, **Then** HEAD, index, and worktree all reset.
5. **Given** `git tag v1.0`, **When** run, **Then** a lightweight tag is created.
6. **Given** `git tag -a v1.0 -m "Release"`, **When** run, **Then** an annotated tag is created.
7. **Given** `git stash`, **When** run, **Then** working tree changes are saved and tree is cleaned.
8. **Given** `git stash pop`, **When** run, **Then** stashed changes are restored.
9. **Given** `git clean -fd`, **When** run, **Then** untracked files and directories are removed.

### Edge Cases

- `git add` with gitignored files (should warn)
- `git commit` with empty message (should fail)
- `git merge` with conflicts (pause for resolution)
- `git checkout` with uncommitted changes that would be overwritten
- `git push` non-fast-forward (reject without --force)
- `git clone` interrupted mid-transfer
- `git rebase` with merge conflicts at multiple commits
- `git reset --hard` losing uncommitted work (should work but is dangerous)

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement `init` with --bare, --initial-branch, --template
- **FR-002**: System MUST implement `clone` with --depth, --branch, --bare, --recurse-submodules
- **FR-003**: System MUST implement `add` with -A, -u, -p (interactive), -n (dry-run), pathspec
- **FR-004**: System MUST implement `rm` with --cached, -f, -r
- **FR-005**: System MUST implement `mv` with -f, -n (dry-run)
- **FR-006**: System MUST implement `status` with --short, --branch, --porcelain, --long
- **FR-007**: System MUST implement `commit` with -m, -a, --amend, --allow-empty, -e (editor), --no-edit
- **FR-008**: System MUST implement `branch` with -d, -D, -m, -M, -a, -r, --list, --format
- **FR-009**: System MUST implement `switch` with -c, -C, --detach, --force
- **FR-010**: System MUST implement `checkout` with -b, -B, --detach, --force, pathspec
- **FR-011**: System MUST implement `merge` with --no-ff, --ff-only, --squash, --abort, --continue
- **FR-012**: System MUST implement `fetch` with --all, --prune, --depth, --tags
- **FR-013**: System MUST implement `pull` with --rebase, --no-rebase, --ff-only
- **FR-014**: System MUST implement `push` with -f, --force-with-lease, --force-with-lease=<ref>, --delete, --tags, --set-upstream/-u, --atomic, --push-option, --dry-run, --verbose, --progress, --no-verify (skip pre-push hook)
- **FR-015**: System MUST implement `remote` with add, remove, rename, set-url, -v
- **FR-016**: System MUST implement `rebase` with -i, --onto, --abort, --continue, --skip
- **FR-017**: System MUST implement `reset` with --soft, --mixed (default), --hard, --merge, --keep
- **FR-018**: System MUST implement `tag` with -a, -d, -l, -m, --format, -v (verify)
- **FR-019**: System MUST implement `stash` with push, pop, list, show, drop, apply, clear
- **FR-020**: System MUST implement `clean` with -f, -d, -n, -x, -X
- **FR-021**: System MUST implement `restore` with --staged, --worktree, --source

### Key Entities

Commands are thin wrappers around library APIs. Each command module handles argument parsing and delegates to library functions.

## Success Criteria

### Measurable Outcomes

- **SC-001**: All 21 commands produce identical output to C git for standard operations
- **SC-002**: All commands accept the same arguments as C git (verified against man pages)
- **SC-003**: Error messages and exit codes match C git behavior
- **SC-004**: Interactive commands (add -p, rebase -i) work in terminal
- **SC-005**: Commands work with repositories created by C git and vice versa

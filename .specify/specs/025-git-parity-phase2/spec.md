# Feature Specification: Git Behavioral Parity — Phase 2

**Feature Branch**: `025-git-parity-phase2`
**Created**: 2026-02-09
**Status**: Draft
**Input**: Systematic comparison testing of 110+ commands revealed 35 differences between gitr and git (v2.39.5, Apple Git-154). 16 are missing flags/features that cause errors, 19 are output format mismatches.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Missing CLI Flags & Subcommand Arguments (Priority: P1)

A developer using gitr as a drop-in replacement for git encounters errors when using common flags that git supports. Commands like `merge --no-edit`, `revert --no-edit`, `switch -c`, `show -s`, `config --unset`, `config --global`, `log --date`, `log --merges`, `log --no-merges`, `log -- <path>`, `diff --word-diff`, `branch --contains`, and `--version` fail with "unexpected argument" errors. This blocks adoption because scripts and muscle-memory workflows break immediately.

**Why this priority**: These are hard failures — the command produces an error instead of a result. Any developer trying gitr will hit these within minutes and abandon it.

**Independent Test**: Run each missing-flag command in a test repository and verify it completes successfully with correct behavior matching git.

**Acceptance Scenarios**:

1. **Given** a repository with divergent branches, **When** the user runs `gitr merge feature --no-edit`, **Then** the merge completes using the default merge message without opening an editor.
2. **Given** a repository with commits, **When** the user runs `gitr revert HEAD --no-edit`, **Then** the revert commit is created using the default revert message without opening an editor.
3. **Given** a repository on branch `main`, **When** the user runs `gitr switch -c new-branch`, **Then** a new branch `new-branch` is created and checked out at the current HEAD.
4. **Given** a repository with commits, **When** the user runs `gitr show -s HEAD`, **Then** the commit header is displayed without the diff output.
5. **Given** a repository with a config key set, **When** the user runs `gitr config --unset <key>`, **Then** the key is removed from the local config file.
6. **Given** a user home directory with `~/.gitconfig`, **When** the user runs `gitr config --global user.name`, **Then** the global config value is read and displayed.
7. **Given** a repository with commits, **When** the user runs `gitr log --date=iso`, **Then** dates are displayed in ISO 8601 format. Same for `--date=relative` and `--date=short`.
8. **Given** a repository with merge commits, **When** the user runs `gitr log --merges --oneline`, **Then** only merge commits are listed.
9. **Given** a repository with merge and non-merge commits, **When** the user runs `gitr log --no-merges --oneline`, **Then** only non-merge commits are listed.
10. **Given** a repository with file changes in a subdirectory, **When** the user runs `gitr log --oneline -- subdir/`, **Then** only commits touching files in `subdir/` are listed.
11. **Given** a repository with word-level changes, **When** the user runs `gitr diff --word-diff`, **Then** the diff output shows word-level additions and deletions using the `[-removed-]{+added+}` format.
12. **Given** a repository with branches, **When** the user runs `gitr branch --contains HEAD`, **Then** branches containing the commit are listed.
13. **Given** any directory, **When** the user runs `gitr --version`, **Then** a version string is printed (e.g., `gitr version 0.1.1-alpha.10`).

---

### User Story 2 — Output Format Parity (Priority: P2)

A developer compares gitr output with git output in scripts, CI pipelines, or visual inspection. Differences in output formatting (missing diffstat after commit, wrong graph rendering, missing status messages after reset, unsorted file lists) cause confusion and break text-processing pipelines that parse git output.

**Why this priority**: These are soft failures — the command works but produces different output. Scripts that grep/parse git output will silently misbehave, and developers will notice visual discrepancies that erode trust.

**Independent Test**: Run each command in identical git and gitr repositories and diff the output. All output must match character-for-character (excluding commit hashes and timestamps).

**Acceptance Scenarios**:

1. **Given** a repository with staged changes, **When** the user runs `gitr commit -m "msg"`, **Then** the output includes the diffstat summary line (e.g., `1 file changed, 1 insertion(+)`).
2. **Given** a commit that amends the previous one, **When** the user runs `gitr commit --amend -m "msg"`, **Then** the output includes the Date line and diffstat.
3. **Given** a linear commit history, **When** the user runs `gitr log --graph --oneline`, **Then** no extra `|` lines appear between commits on a single branch.
4. **Given** a successful merge, **When** the merge completes, **Then** the output includes the diffstat summary after "Merge made by the 'ort' strategy." line.
5. **Given** a cherry-pick operation, **When** the cherry-pick succeeds, **Then** the output format is `[branch hash] message` (includes branch name).
6. **Given** a stash pop, **When** the user runs `gitr stash pop`, **Then** the full working tree status is printed and the "Dropped" message uses the full 40-character hash.
7. **Given** a mixed reset, **When** the user runs `gitr reset HEAD~1`, **Then** the output shows "Unstaged changes after reset:" followed by the list of affected files.
8. **Given** a hard reset, **When** the user runs `gitr reset --hard HEAD`, **Then** the output shows `HEAD is now at <short-hash> <subject>`.
9. **Given** a `git mv` operation, **When** the user runs `gitr status --short`, **Then** renames are detected and shown as `R  old -> new`.
10. **Given** a rebase, **When** the user runs `gitr rebase main`, **Then** the output shows `Rebasing (1/1)` progress format and ends with "Successfully rebased and updated refs/heads/<branch>."
11. **Given** a repository, **When** the user runs `gitr gc`, **Then** no output is printed (silent like git).
12. **Given** a repository with untracked files and directories, **When** the user runs `gitr status`, **Then** untracked files are sorted alphabetically and directories are collapsed to `subdir/` instead of listing individual files.

---

### User Story 3 — Date Parsing & Reflog (Priority: P1)

A developer uses environment variables `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE` with ISO 8601 timestamps to create reproducible commits (common in CI and testing). Additionally, `gitr reflog` returns empty despite operations having been performed, which breaks debugging workflows.

**Why this priority**: ISO 8601 date env vars are used in CI/testing pipelines and reproducible builds. Reflog is essential for recovery workflows (`git reflog` to find lost commits).

**Independent Test**: Create commits with ISO 8601 dates in environment variables and verify the commit object contains the correct timestamp. Run operations and verify reflog shows entries.

**Acceptance Scenarios**:

1. **Given** `GIT_AUTHOR_DATE="2024-01-15T10:00:00+00:00"` is set, **When** the user runs `gitr commit -m "test"`, **Then** the commit is created with the specified author date.
2. **Given** `GIT_COMMITTER_DATE="2024-01-15T10:00:00+00:00"` is set, **When** the user runs `gitr commit -m "test"`, **Then** the commit is created with the specified committer date.
3. **Given** a repository where commits, checkouts, and resets have been performed, **When** the user runs `gitr reflog`, **Then** all operations are listed with correct `HEAD@{N}` indices, short hashes, and action descriptions.

---

### User Story 4 — Minor Output Corrections (Priority: P3)

Several commands have minor formatting differences: `describe` error messages have doubled "fatal:" prefix, `tag -n` doesn't show commit messages for lightweight tags, `show` indents annotated tag messages with 4 spaces when git uses none, `log --pretty=fuller` omits the `Merge:` line for merge commits, `shortlog` reads from HEAD instead of stdin in non-tty mode, `init` doesn't resolve symlinks or format the path correctly, and the `status` unstage hint uses a different message.

**Why this priority**: These are cosmetic or edge-case differences that are unlikely to break workflows but reduce confidence in gitr as a faithful reproduction.

**Independent Test**: Run each command in identical git and gitr repositories and compare output formatting.

**Acceptance Scenarios**:

1. **Given** a repository with no annotated tags, **When** the user runs `gitr describe`, **Then** the error message matches git exactly: `fatal: No annotated tags can describe '<hash>'. However, there were unannotated tags: try --tags.`
2. **Given** a lightweight tag on a commit, **When** the user runs `gitr tag -n`, **Then** the tag line includes the commit subject (e.g., `v1.0            Initial commit`).
3. **Given** an annotated tag with a message, **When** the user runs `gitr show v2.0`, **Then** the tag message body is not indented (no leading spaces).
4. **Given** a merge commit, **When** the user runs `gitr log -1 --pretty=fuller`, **Then** the output includes the `Merge:` line showing abbreviated parent hashes.
5. **Given** piped input from `git log`, **When** the user runs `gitr log --format="%aN" | gitr shortlog -s -n`, **Then** shortlog reads from stdin (not HEAD) and produces correct counts.
6. **Given** a directory path that is a symlink (e.g., `/var/folders/...` on macOS), **When** the user runs `gitr init`, **Then** the output path resolves symlinks and omits the `.git` suffix, matching git exactly.
7. **Given** staged files on an initial commit, **When** the user runs `gitr status`, **Then** the unstage hint reads `(use "git rm --cached <file>..." to unstage)` matching git v2.39 behavior.

---

### Edge Cases

- What happens when `--date=format:%Y-%m-%d` (custom strftime) is passed to `log`? Should be supported if `--date` is implemented.
- What happens when `config --global` is used but no `~/.gitconfig` exists? Should create the file, matching git behavior.
- What happens when `log -- <path>` is used with a path that matches both a file and a branch name? Should follow git's disambiguation rules (use `--` separator).
- What happens when `switch -c` is used with a branch name that already exists? Should error with the same message as git.
- What happens when `reset --hard` is run with uncommitted changes? Changes should be discarded, matching git.
- What happens when `reflog` is queried for a branch that has never been checked out? Should return empty or error matching git.
- What happens when `merge --no-edit` is combined with `--no-commit`? The `--no-commit` flag should take precedence.
- What happens when `diff --word-diff=color` or `--word-diff=porcelain` is passed? These are word-diff modes that git supports.
- What happens when `stash pop` encounters a conflict? Should show merge conflict status matching git.
- What happens when `add -p` is invoked in a non-interactive environment? Should handle gracefully.

## Requirements *(mandatory)*

### Functional Requirements

**Missing Flags & Features:**

- **FR-001**: System MUST support `--version` as a global flag, printing the application name and version.
- **FR-002**: System MUST support `--no-edit` flag on `merge`, using the auto-generated merge message without invoking an editor.
- **FR-003**: System MUST support `--no-edit` flag on `revert`, using the auto-generated revert message without invoking an editor.
- **FR-004**: System MUST support `switch -c <branch>` to create and switch to a new branch at the current HEAD.
- **FR-005**: System MUST support `config --unset <key>` to remove a configuration key from the config file.
- **FR-006**: System MUST support `config --global` to read and write from the user-level config file (`~/.gitconfig`).
- **FR-007**: System MUST support `log --date=<format>` for at least `iso`, `relative`, `short`, and `default` date formats.
- **FR-008**: System MUST support `log --merges` to filter log output to only merge commits.
- **FR-009**: System MUST support `log --no-merges` to filter log output to exclude merge commits.
- **FR-010**: System MUST support `log -- <path>` to limit log output to commits affecting the specified path(s).
- **FR-011**: System MUST support `diff --word-diff` to display word-level differences.
- **FR-012**: System MUST support `show -s` to suppress diff output and show only commit header information.
- **FR-013**: System MUST support `branch --contains <commit>` to list branches containing the specified commit.
- **FR-014**: System MUST parse ISO 8601 date strings (e.g., `2024-01-15T10:00:00+00:00`) from `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE` environment variables.
- **FR-015**: System MUST record reflog entries for commit, checkout, reset, merge, rebase, and other HEAD-modifying operations, and `reflog` must display them.

**Output Format Corrections:**

- **FR-016**: `commit` output MUST include a diffstat summary line (e.g., `1 file changed, 1 insertion(+)`).
- **FR-017**: `commit --amend` output MUST include the Date line and diffstat summary.
- **FR-018**: `log --graph` on linear history MUST NOT insert extra `|` lines between commits.
- **FR-019**: `merge` output MUST include a diffstat summary after the strategy message.
- **FR-020**: `cherry-pick` output MUST use format `[<branch> <short-hash>] <subject>` including the current branch name.
- **FR-021**: `stash pop` output MUST display the full working tree status and use the full 40-character hash in the "Dropped" message.
- **FR-022**: `reset` (mixed) output MUST show "Unstaged changes after reset:" followed by the list of modified/deleted files.
- **FR-023**: `reset --hard` output MUST show `HEAD is now at <short-hash> <subject>`.
- **FR-024**: `status --short` MUST detect file renames and display them as `R  old -> new`.
- **FR-025**: `rebase` output MUST use `Rebasing (N/M)` progress format and end with "Successfully rebased and updated refs/heads/<branch>."
- **FR-026**: `gc` MUST produce no output by default (matching git's silent behavior).
- **FR-027**: `status` MUST sort untracked files alphabetically and collapse directories to `subdir/` instead of listing individual files.
- **FR-028**: `describe` error messages MUST NOT have a doubled `fatal:` prefix and MUST match git's wording.
- **FR-029**: `tag -n` MUST show the commit subject for lightweight tags.
- **FR-030**: `show` for annotated tags MUST NOT indent the tag message body.
- **FR-031**: `log --pretty=fuller` for merge commits MUST include the `Merge:` line with abbreviated parent hashes.
- **FR-032**: `shortlog` in non-tty mode MUST read from stdin, matching git behavior.
- **FR-033**: `init` output MUST resolve symlinks in the path and omit the `.git` suffix.
- **FR-034**: `status` unstage hint MUST read `(use "git rm --cached <file>..." to unstage)` for initial commits, matching git v2.39 behavior.

**Deferred (not in scope):**

- **FR-035**: `add -p` (interactive patch mode) is deferred. It requires a full terminal UI for hunk selection and is a large standalone feature.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 34 in-scope commands/flags (FR-001 through FR-034) produce output identical to git v2.39.5 when run on the same repository state.
- **SC-002**: A scripted comparison test of 110+ commands (as performed in the initial audit) shows zero differences for all in-scope items.
- **SC-003**: ISO 8601 date strings in `GIT_AUTHOR_DATE` / `GIT_COMMITTER_DATE` are parsed correctly for all standard formats (`YYYY-MM-DDTHH:MM:SS+HH:MM`, `YYYY-MM-DD HH:MM:SS +HHMM`, Unix timestamp).
- **SC-004**: Reflog entries are recorded and displayed for all HEAD-modifying operations (commit, checkout, reset, merge, rebase, cherry-pick, stash pop).
- **SC-005**: No existing passing tests are broken by these changes (full regression suite passes).

### Assumptions

- The target git version for parity is 2.39.x (Apple Git, the version installed on the test system).
- `add -p` (interactive patch mode) is explicitly deferred as out of scope due to its complexity as a standalone UI feature.
- The `status` unstage hint should match git v2.39 behavior (`git rm --cached`) rather than newer git versions (`git restore --staged`), since that is the comparison baseline.
- Word-diff mode `plain` (default) is required; other modes (`color`, `porcelain`) are stretch goals.
- Global config support (`--global`) means reading/writing `~/.gitconfig`; system config (`--system`) is out of scope.

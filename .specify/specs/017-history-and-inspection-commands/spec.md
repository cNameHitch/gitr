# Feature Specification: History & Inspection Commands

**Feature Branch**: `017-history-and-inspection-commands`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: All library crates (001-014), 015-plumbing-commands

## User Scenarios & Testing

### User Story 1 - View Commit History (Priority: P1)

As a git user, I need `git log` to view commit history with various formats and filters.

**Why this priority**: `git log` is one of the most-used commands.

**Acceptance Scenarios**:

1. **Given** `git log`, **When** run, **Then** commits are shown newest-first with hash, author, date, message.
2. **Given** `git log --oneline`, **When** run, **Then** compact one-line format is shown.
3. **Given** `git log --graph`, **When** run, **Then** ASCII art branch graph is drawn.
4. **Given** `git log -p`, **When** run, **Then** diffs are shown with each commit.
5. **Given** `git log --stat`, **When** run, **Then** file change summary is shown per commit.
6. **Given** `git log --author="Alice"`, **When** run, **Then** only Alice's commits are shown.
7. **Given** `git log -- path/to/file`, **When** run, **Then** only commits touching that file are shown.
8. **Given** `git log A..B`, **When** run, **Then** commits in B but not in A are shown.

---

### User Story 2 - Show Objects (Priority: P1)

As a git user, I need `git show` to inspect individual commits, tags, trees, and blobs.

**Acceptance Scenarios**:

1. **Given** `git show HEAD`, **When** run, **Then** the commit message and diff are shown.
2. **Given** `git show v1.0`, **When** run, **Then** the tag message and tagged commit are shown.
3. **Given** `git show HEAD:file.txt`, **When** run, **Then** the file content at HEAD is printed.

---

### User Story 3 - Diff Commands (Priority: P1)

As a git user, I need `git diff` to see changes.

**Acceptance Scenarios**:

1. **Given** `git diff`, **When** run, **Then** unstaged changes are shown.
2. **Given** `git diff --cached`, **When** run, **Then** staged changes are shown.
3. **Given** `git diff HEAD`, **When** run, **Then** all changes since HEAD are shown.
4. **Given** `git diff A B`, **When** run, **Then** differences between two commits are shown.
5. **Given** `git diff --stat`, **When** run, **Then** summary statistics are shown.

---

### User Story 4 - Blame (Priority: P2)

As a git user, I need `git blame` to see who last changed each line of a file.

**Why this priority**: Essential for code review and debugging.

**Acceptance Scenarios**:

1. **Given** `git blame file.txt`, **When** run, **Then** each line is annotated with the commit, author, and date of its last change.
2. **Given** `git blame -L 10,20 file.txt`, **When** run, **Then** only lines 10-20 are shown.
3. **Given** `git blame -C file.txt`, **When** run, **Then** lines copied from other files are attributed to the source.

---

### User Story 5 - Bisect (Priority: P2)

As a git user, I need `git bisect` to binary-search for the commit that introduced a bug.

**Acceptance Scenarios**:

1. **Given** `git bisect start`, **When** run, **Then** bisect mode begins.
2. **Given** `git bisect good <oid>` and `git bisect bad <oid>`, **When** set, **Then** git checks out the midpoint commit.
3. **Given** repeated good/bad markings, **When** bisect completes, **Then** the first bad commit is identified.
4. **Given** `git bisect run <script>`, **When** run, **Then** automated bisection runs the script to determine good/bad.

---

### User Story 6 - Other History Commands (Priority: P2)

As a git user, I need `shortlog`, `describe`, `grep`, `cherry-pick`, `revert`, `am`, and `format-patch` for history operations.

**Acceptance Scenarios**:

1. **Given** `git shortlog`, **When** run, **Then** commits are grouped by author.
2. **Given** `git describe`, **When** run, **Then** a human-readable name based on nearest tag is produced (e.g., `v1.0-3-gabcdef`).
3. **Given** `git grep "pattern"`, **When** run, **Then** all files matching the pattern are shown.
4. **Given** `git cherry-pick <oid>`, **When** run, **Then** the commit is applied to current branch.
5. **Given** `git revert <oid>`, **When** run, **Then** the inverse of the commit is applied.
6. **Given** `git format-patch A..B`, **When** run, **Then** patch files are created for each commit.
7. **Given** `git am *.patch`, **When** run, **Then** patch files are applied as commits.
8. **Given** `git reflog`, **When** run, **Then** the reflog for HEAD is displayed.

### Edge Cases

- `git log` with 100K+ commits (performance, pagination)
- `git blame` on a file with many renames
- `git bisect` with merge commits
- `git grep` across all branches
- `git format-patch` with binary diffs
- `git am` with conflict

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement `log` with --oneline, --graph, --stat, -p, --format, --author, --since, --until, --all, pathspec
- **FR-002**: System MUST implement `show` for commits, tags, trees, blobs, and tree:path syntax
- **FR-003**: System MUST implement `diff` for worktree, staged, HEAD, and commit-to-commit comparisons
- **FR-004**: System MUST implement `blame` with -L (line range), -C (copy detection), -w (ignore whitespace), --porcelain
- **FR-005**: System MUST implement `bisect` with start, good, bad, skip, reset, run, log, visualize
- **FR-006**: System MUST implement `shortlog` with -s (summary), -n (sort by count), -e (show email)
- **FR-007**: System MUST implement `describe` with --tags, --long, --always, --dirty
- **FR-008**: System MUST implement `grep` with -i, -n, -l, -c, -e, --and, --or, --not, --all-match
- **FR-009**: System MUST implement `cherry-pick` with -n (no-commit), --edit, --abort, --continue
- **FR-010**: System MUST implement `revert` with -n, --edit, --abort, --continue
- **FR-011**: System MUST implement `format-patch` with -o (output dir), --cover-letter, -n, --thread
- **FR-012**: System MUST implement `am` with --abort, --continue, --skip, -3 (three-way merge)
- **FR-013**: System MUST implement `reflog` with show, expire, delete
- **FR-014**: System MUST implement `rev-list` (combines rev-walk with output formatting)

### Key Entities

Commands delegate to library APIs (git-revwalk, git-diff, git-merge, git-odb).

## Success Criteria

### Measurable Outcomes

- **SC-001**: `git log` output matches C git for all format options
- **SC-002**: `git blame` matches C git annotation for all test files
- **SC-003**: `git bisect` finds the correct commit for all test scenarios
- **SC-004**: `git format-patch` + `git am` round-trip preserves commits exactly
- **SC-005**: All commands produce identical output to C git

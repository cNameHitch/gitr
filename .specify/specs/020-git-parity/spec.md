# Feature Specification: Git Command Parity

**Feature Branch**: `020-git-parity`
**Created**: 2026-02-07
**Status**: Draft
**Input**: User description: "Make gitr a drop-in replacement for C git by implementing all missing functionality revealed by interop test gaps. Covers remote operations, merge parity, stash, output format parity, plumbing parity, packfile interop, and rebase correctness."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Merge Parity (Priority: P1)

A developer using gitr to merge branches expects identical behavior to C git. Currently, fast-forward merges do not advance the branch ref, three-way merges do not include feature branch files in the resulting tree, merge conflict handling produces different exit codes and output, and three-way merges do not produce proper merge commits with two parents.

**Why this priority**: Merging is fundamental to every git workflow. Broken merge means gitr cannot be used for any branch-based development, which is the majority of real-world git usage. This is the highest-impact gap because it affects local-only workflows that otherwise work.

**Independent Test**: Can be fully tested by creating branches with divergent histories, performing fast-forward and three-way merges with both gitr and C git in parallel repos, and comparing resulting refs, tree contents, exit codes, and conflict markers.

**Acceptance Scenarios**:

1. **Given** a repo with a branch ahead of main (no divergence), **When** gitr runs `merge feature` from main, **Then** the main branch ref advances to the feature tip (fast-forward), and `show-ref` output matches C git.
2. **Given** a repo with two branches containing non-conflicting changes to different files, **When** gitr runs `merge feature -m "merge"`, **Then** the resulting tree contains files from both branches, a merge commit with two parents is created, and `ls-tree -r HEAD` output matches C git.
3. **Given** a repo with two branches modifying the same file differently, **When** gitr runs `merge feature`, **Then** the exit code is 1, conflict markers are written to the working tree in the same format as C git, and `ls-files --stage` shows the conflict entries matching C git.
4. **Given** a three-way merge, **When** gitr creates the merge commit, **Then** `log --format=%s -n 1 HEAD` shows the merge message, and `cat-file -p HEAD` shows two parent lines.

---

### User Story 2 - Diff Hunk Content (Priority: P1)

A developer using gitr to view diffs expects to see the actual changed lines, not just the diff header. Currently, `gitr diff` shows the `diff --git` header but omits the hunk content (@@-lines and +/- lines).

**Why this priority**: Diff is essential for code review and understanding changes. Without hunk content, diff output is useless for any practical purpose.

**Independent Test**: Can be fully tested by modifying tracked files (staged and unstaged) and comparing `diff`, `diff --cached`, and `diff HEAD` output between gitr and C git.

**Acceptance Scenarios**:

1. **Given** a repo with a modified tracked file (unstaged), **When** gitr runs `diff`, **Then** the output includes the full unified diff with hunk headers and +/- content lines, matching C git byte-for-byte.
2. **Given** a repo with a staged modification, **When** gitr runs `diff --cached`, **Then** the output matches C git.
3. **Given** a repo with both staged and unstaged changes, **When** gitr runs `diff HEAD`, **Then** the output matches C git.

---

### User Story 3 - Output Format Parity (Priority: P1)

A developer or script consuming gitr output expects dates, formatting, and field widths to match C git exactly. Currently, gitr uses ISO date format instead of C git's default format, `log --format=%s` omits newlines between entries, `show` uses different date format, `blame` uses shorter OID prefix and omits time from date, `status` in detached HEAD omits the commit hash, `log` on empty repos returns exit 0 instead of 128, and `ls-files` shows unicode filenames literally instead of octal-escaping them.

**Why this priority**: Output format compatibility is critical for scripting and tooling. Any tool, CI pipeline, or IDE plugin that parses git output will break if the format differs.

**Independent Test**: Can be tested by running each command with known repo states and comparing output byte-for-byte between gitr and C git.

**Acceptance Scenarios**:

1. **Given** a repo with commits, **When** gitr runs `log` (default format), **Then** dates are formatted as `Thu Feb 13 23:31:30 2009 +0000` (C git format), not `2009-02-13 23:31:30 +0000`.
2. **Given** a repo with 3 commits, **When** gitr runs `log --format=%s`, **Then** each subject line is separated by a newline, matching C git output byte-for-byte.
3. **Given** a repo with commits, **When** gitr runs `show HEAD`, **Then** the date format, blank lines, and diff output match C git.
4. **Given** a file with multi-author history, **When** gitr runs `blame file.txt`, **Then** the OID prefix length, date format (including time), and column alignment match C git.
5. **Given** a repo in detached HEAD state, **When** gitr runs `status`, **Then** the output includes the short OID (e.g., "HEAD detached at 75f87f8"), matching C git.
6. **Given** an empty repo with no commits, **When** gitr runs `log`, **Then** the exit code is 128 (matching C git's "fatal: your current branch 'main' does not have any commits yet").
7. **Given** a repo with unicode-named files, **When** gitr runs `ls-files`, **Then** non-ASCII bytes are octal-escaped in double quotes (e.g., `"caf\303\251.txt"`), matching C git's default quoting behavior.

---

### User Story 4 - Packfile Reading (Priority: P1)

A developer using gitr on a repository where C git has run `gc` expects gitr to continue operating normally. Currently, gitr cannot read packed objects from packfiles produced by C git's `gc`.

**Why this priority**: Any repository that has existed for a while will have packed objects. Without packfile reading, gitr is broken on the vast majority of real-world repositories. This is a fundamental interoperability requirement.

**Independent Test**: Can be tested by creating a repo with gitr, running C git `gc` to pack objects, and then verifying gitr can still read objects via `log`, `cat-file`, and other commands.

**Acceptance Scenarios**:

1. **Given** a repo with 12 commits created by gitr, **When** C git runs `gc` (packing all loose objects), **Then** gitr can still run `log --oneline` and see all 12 commits.
2. **Given** a repo where C git has run `gc`, **When** gitr runs `cat-file -p HEAD`, **Then** the commit object is correctly read from the packfile and output matches the original loose object content.
3. **Given** a packfile with OFS_DELTA and REF_DELTA entries, **When** gitr reads objects from the pack, **Then** all objects are correctly resolved and accessible.

---

### User Story 5 - Remote Operations (Priority: P2)

A developer using gitr to interact with remote repositories expects clone, push, fetch, and pull to work over local file:// transport. Currently, none of these operations work (the gitr binary is not found for remote commands, suggesting they are not implemented).

**Why this priority**: Remote operations are essential for collaboration. However, they require local operations (merge, packfile reading) to work first, hence P2.

**Independent Test**: Can be tested by setting up bare repos, cloning with gitr, pushing commits, and verifying with C git that all objects and refs are correctly transferred.

**Acceptance Scenarios**:

1. **Given** a bare repo with commits, **When** gitr runs `clone file:///path/to/bare repo`, **Then** the cloned repo's log, show-ref, and working tree files match a C git clone.
2. **Given** a bare repo, **When** gitr runs `clone --bare file:///path/to/bare`, **Then** the resulting bare repo has the correct structure (HEAD, refs, objects, no .git directory) matching C git.
3. **Given** a cloned repo with new local commits, **When** gitr runs `push origin main`, **Then** C git can clone the bare repo and see the pushed commits.
4. **Given** new commits pushed by C git to a bare remote, **When** gitr runs `fetch origin` followed by `merge origin/main`, **Then** gitr's local state matches C git.
5. **Given** a cloned repo behind the remote by one commit, **When** gitr runs `pull origin main`, **Then** the local branch fast-forwards and `log --oneline` matches C git.
6. **Given** a cloned repo, **When** gitr creates a new branch with commits and runs `push origin feature`, **Then** C git can see the new remote branch.
7. **Given** a cloned repo, **When** gitr inspects `config --get remote.origin.url` and `config --get remote.origin.fetch`, **Then** the values match what C git sets during clone.

---

### User Story 6 - Stash Operations (Priority: P2)

A developer using gitr to temporarily shelve work expects stash push/pop/list to work correctly. Currently, stash pop does not fully restore working tree state, stash list only shows the latest entry, and `--include-untracked` does not stash untracked files.

**Why this priority**: Stash is a commonly used convenience feature but not as fundamental as merge or diff. It depends on correct internal commit/tree machinery.

**Independent Test**: Can be tested by creating dirty working trees, stashing changes, verifying clean state, popping, and comparing results between gitr and C git.

**Acceptance Scenarios**:

1. **Given** a repo with uncommitted modifications, **When** gitr runs `stash push`, **Then** `status --porcelain` shows a clean tree, and `stash pop` restores the modifications, all matching C git.
2. **Given** a repo where 3 stashes have been pushed with messages, **When** gitr runs `stash list`, **Then** all 3 entries are shown with correct `stash@{N}: On branch: message` format, matching C git.
3. **Given** a repo with an untracked file, **When** gitr runs `stash push --include-untracked`, **Then** the untracked file is removed from the working tree, and `stash pop` restores it, matching C git.

---

### User Story 7 - Plumbing Command Parity (Priority: P2)

A developer or script using plumbing commands expects exact output matching. Currently, `for-each-ref` includes HEAD in its output (C git does not), and `rev-parse` does not support `^{tree}` peeling syntax.

**Why this priority**: Plumbing commands are used by scripts, hooks, and tooling. Incorrect output breaks automation.

**Independent Test**: Can be tested by running plumbing commands against known repo states and comparing output between gitr and C git.

**Acceptance Scenarios**:

1. **Given** a repo with branches and tags, **When** gitr runs `for-each-ref --format=%(refname) %(objectname) %(objecttype)`, **Then** HEAD is not included in the output, and the ref list matches C git.
2. **Given** a repo with commits, **When** gitr runs `rev-parse HEAD^{tree}`, **Then** the tree OID is returned (exit 0), matching C git.
3. **Given** various peeling expressions (`HEAD^{commit}`, `v1.0^{commit}`, `HEAD^{tree}`), **When** gitr runs `rev-parse` on each, **Then** the resolved OIDs match C git.

---

### User Story 8 - Rebase Correctness (Priority: P3)

A developer using gitr to rebase a feature branch expects the resulting commit history to be identical to C git. Currently, linear rebase produces commits with different OIDs, likely due to timestamp or tree handling differences during commit replay.

**Why this priority**: Rebase is an advanced operation. The abort and onto variants already work, but linear rebase needs the commit replay to produce identical results.

**Independent Test**: Can be tested by rebasing a feature branch onto main in parallel repos and comparing resulting `log --oneline` output.

**Acceptance Scenarios**:

1. **Given** a feature branch with 2 commits diverged from main, **When** gitr runs `rebase main` from the feature branch, **Then** the rebased commit messages appear in the correct order on top of main, and the resulting OIDs match C git (same tree, same parent, same timestamps produce same OID).

---

### Edge Cases

- What happens when gitr attempts to read a packfile with multi-base deltas (delta chains deeper than 1)?
- How does gitr handle pack index v2 files created by C git?
- What happens when a clone encounters a repository with alternates configured?
- How does gitr handle fetching when the remote has tags pointing to non-commit objects?
- What happens when stash push is run with no changes to stash?
- How does gitr handle merge when the working tree has uncommitted changes?
- What happens when rev-parse receives an ambiguous ref name that could match both a branch and a tag?
- How does gitr handle `diff` on binary files (should show "Binary files differ" marker)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: gitr MUST advance the branch ref during fast-forward merges, producing identical ref state to C git.
- **FR-002**: gitr MUST perform three-way merges that include files from both branches in the resulting tree, creating a merge commit with two parent lines.
- **FR-003**: gitr MUST detect merge conflicts, exit with code 1, write C git-compatible conflict markers to the working tree, and populate the index with stage 1/2/3 entries.
- **FR-004**: gitr MUST produce unified diff output with hunk headers and content lines for `diff`, `diff --cached`, and `diff HEAD`.
- **FR-005**: gitr MUST format dates in default log/show output using C git's format (`Thu Feb 13 23:31:30 2009 +0000`), not ISO format.
- **FR-006**: gitr MUST append newlines between entries when using `log --format=` custom formats.
- **FR-007**: gitr MUST format `blame` output with the same OID prefix length, date+time format, and column alignment as C git.
- **FR-008**: gitr MUST include the short commit OID in detached HEAD `status` output (e.g., "HEAD detached at abc1234").
- **FR-009**: gitr MUST return exit code 128 when `log` is run on a repository with no commits.
- **FR-010**: gitr MUST octal-escape non-ASCII bytes in file paths for `ls-files` output, matching C git's default quoting.
- **FR-011**: gitr MUST read objects from packfiles (v2 pack index), including resolving OFS_DELTA and REF_DELTA entries.
- **FR-012**: gitr MUST implement `clone` over file:// protocol, creating a working copy with correct refs, objects, remote configuration, and working tree.
- **FR-013**: gitr MUST implement `clone --bare` creating a bare repository with correct structure.
- **FR-014**: gitr MUST implement `push` to send local objects and update remote refs over file:// protocol.
- **FR-015**: gitr MUST implement `fetch` to retrieve remote objects and update remote-tracking refs over file:// protocol.
- **FR-016**: gitr MUST implement `pull` as fetch followed by merge (fast-forward at minimum).
- **FR-017**: gitr MUST implement `stash push` to save working tree and index state to the stash reflog.
- **FR-018**: gitr MUST implement `stash pop` to restore the most recent stash and drop it from the reflog.
- **FR-019**: gitr MUST implement `stash list` to display all stash entries, not just the most recent.
- **FR-020**: gitr MUST implement `stash push --include-untracked` to also stash untracked files.
- **FR-021**: gitr MUST exclude HEAD from `for-each-ref` output, matching C git behavior.
- **FR-022**: gitr MUST support `^{tree}`, `^{commit}`, and other peeling syntax in `rev-parse`.
- **FR-023**: gitr MUST produce identical OIDs during `rebase` when the input trees, parents, messages, and timestamps are identical to C git.

### Key Entities

- **Packfile**: Binary file containing multiple git objects in a compressed, delta-encoded format. Consists of a header, object entries (undeltified, OFS_DELTA, REF_DELTA), and a trailing checksum. Accompanied by a pack index (.idx) file for random access.
- **Pack Index**: Binary file mapping object OIDs to offsets within a packfile. Version 2 format includes a fanout table, sorted OID list, CRC32 values, and offset tables.
- **Stash Entry**: A commit object stored in the `refs/stash` reflog. Each stash entry captures the working tree state, index state, and optionally untracked files as a special merge commit structure.
- **Merge Commit**: A commit with two or more parent lines, created during three-way merge. Contains the merged tree, both parent OIDs, author/committer info, and merge message.
- **Remote Configuration**: Git config entries (`remote.origin.url`, `remote.origin.fetch`) that define the relationship between a local clone and its upstream repository.
- **Conflict Entry**: Index entries at stages 1 (common ancestor), 2 (ours), and 3 (theirs) that represent an unresolved merge conflict.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 25 previously-deleted interop tests are re-added and pass — gitr output matches C git for every covered command and scenario.
- **SC-002**: Repositories with packed objects (after C git `gc`) are fully operable by gitr — `log`, `cat-file`, `show`, `blame`, and `diff` all produce correct output.
- **SC-003**: A complete clone-commit-push-fetch-pull cycle works between gitr and C git over file:// transport with identical resulting repository state.
- **SC-004**: All merge scenarios (fast-forward, three-way clean, three-way conflict) produce byte-identical results between gitr and C git (refs, tree, conflict markers, exit codes).
- **SC-005**: Stash push/pop/list roundtrip produces identical working tree and reflog state between gitr and C git.
- **SC-006**: All output format commands (log, show, blame, status, diff, ls-files, for-each-ref, rev-parse) produce byte-identical output between gitr and C git for the tested flag combinations.

## Assumptions

- C git (version 2.x) is available on the PATH in all test and development environments.
- Local file:// transport is the initial target for remote operations; SSH and HTTPS transport are out of scope.
- Packfile format targets are v2 pack index and standard pack format (not v2 pack format with capabilities).
- The existing shared test harness (`crates/git-cli/tests/common/mod.rs`) will be used for all re-added interop tests.
- SHA-1 repositories are the target; SHA-256 is out of scope.
- Stash implementation follows C git's internal structure (stash as reflog of special merge commits).
- The rebase OID mismatch is assumed to be caused by timestamp or environment variable handling during commit replay, not a fundamental algorithmic difference.

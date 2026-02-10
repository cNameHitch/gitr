# Feature Specification: Git Behavioral Parity Polish

**Feature Branch**: `023-git-parity-polish`
**Created**: 2026-02-09
**Status**: Draft
**Input**: User description: "Address 43 identified behavioral differences between gitr and git to achieve output-identical parity across functional bugs, missing flags/options, formatting/cosmetic differences, exit code mismatches, and config/init gaps."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Core Command Correctness (Priority: P0)

A developer using gitr for everyday work expects `diff`, `reset`, `log`, `show`, and `init` to behave identically to git. When these commands produce errors, wrong output, or missing information, the developer cannot trust gitr as a drop-in replacement and must fall back to git.

**Why this priority**: These are the most frequently used commands in any git workflow. Broken pathspec parsing in `diff` and `reset`, missing date filtering in `log`, and incorrect `show` output make gitr unusable for common daily tasks.

**Independent Test**: Run each fixed command with identical inputs in both git and gitr repos and assert byte-identical stdout, stderr, and exit codes.

**Acceptance Scenarios**:

1. **Given** a repo with modified tracked files, **When** running `gitr diff file.txt` (bare pathspec), **Then** output matches `git diff file.txt` exactly.
2. **Given** a repo with modified tracked files, **When** running `gitr diff -- file.txt`, **Then** output matches `git diff -- file.txt` exactly.
3. **Given** a repo with modified tracked files, **When** running `gitr diff HEAD -- file.txt`, **Then** output matches `git diff HEAD -- file.txt` exactly.
4. **Given** a newly staged file (not yet committed), **When** running `gitr diff --cached`, **Then** hunk headers show `@@ -0,0 +1,N @@` (not `@@ -1,0 +1,N @@`).
5. **Given** a repo with a staged file, **When** running `gitr reset file.txt` (no `--` separator), **Then** the file is unstaged, matching `git reset file.txt`.
6. **Given** a repo with a staged file, **When** running `gitr reset HEAD file.txt`, **Then** the file is unstaged, matching `git reset HEAD file.txt`.
7. **Given** a `.gitignore` with `build/` pattern, **When** running `gitr check-ignore build/output.o`, **Then** exit code is 0 and output matches `git check-ignore build/output.o`.
8. **Given** a `.gitignore` with `build/` pattern, **When** running `gitr check-ignore build`, **Then** exit code is 0 and output matches `git check-ignore build`.
9. **Given** a repo with commits spanning multiple dates, **When** running `gitr log --since='2025-01-15' --until='2025-01-16'`, **Then** only commits within that date range appear, matching git output.
10. **Given** a non-existent directory path, **When** running `gitr init /tmp/new/nested/repo`, **Then** the directory hierarchy is created and a repo is initialized, matching `git init` behavior.
11. **Given** a merge commit, **When** running `gitr show --stat HEAD`, **Then** only stat output appears (no full diff), matching `git show --stat HEAD`.
12. **Given** a merge commit, **When** running `gitr show --no-patch HEAD`, **Then** output includes the `Merge: <parent1> <parent2>` header line.
13. **Given** a tree hash and parent commit, **When** running `echo "message" | gitr commit-tree <tree> -p HEAD`, **Then** a commit is created using the piped message.
14. **Given** any commit, **When** running `gitr show --format=oneline --no-patch HEAD`, **Then** the full 40-character hash is shown (not abbreviated).
15. **Given** any commit, **When** running `gitr show --format=raw HEAD`, **Then** the commit message body is indented with 4 leading spaces.
16. **Given** any commit, **When** running `gitr show --format='%H %s' --no-patch HEAD`, **Then** output is the full hash followed by the subject line, matching git's custom format string support.

---

### User Story 2 - Missing Flag Support (Priority: P1)

A developer familiar with git expects commonly used flags and subcommands to work. When gitr rejects recognized git flags with "unexpected argument" errors, it breaks muscle memory and scripts that depend on these flags.

**Why this priority**: These flags are widely used in everyday workflows, shell aliases, and CI scripts. Their absence causes immediate friction and forces developers to rewrite commands.

**Independent Test**: Run each flag in both git and gitr and assert identical output or semantically equivalent results.

**Acceptance Scenarios**:

1. **Given** a repo with 10+ commits, **When** running `gitr log -3`, **Then** exactly 3 commits are shown, matching `git log -3`.
2. **Given** a repo with branches and tags, **When** running `gitr log --oneline --decorate`, **Then** ref names appear beside commit hashes in the same format as git.
3. **Given** a repo with multiple branches, **When** running `gitr branch -v`, **Then** each branch shows its commit hash and subject line, matching `git branch -v`.
4. **Given** a repo with annotated tags, **When** running `gitr tag -n`, **Then** tag annotation lines are displayed.
5. **Given** a tree with files of varying sizes, **When** running `gitr ls-tree -l HEAD`, **Then** object sizes are shown alongside entries, matching `git ls-tree -l HEAD`.
6. **Given** HEAD pointing to branch `main`, **When** running `gitr rev-parse --abbrev-ref HEAD`, **Then** output is `main`.
7. **Given** any commit, **When** running `gitr rev-parse --short HEAD`, **Then** output is an abbreviated hash (default 7 chars), matching git behavior.
8. **Given** a repo with local config, **When** running `gitr config --local user.name`, **Then** only the local config value is returned.
9. **Given** a repo with commits, **When** running `gitr format-patch --stdout HEAD~1`, **Then** patch content is written to stdout.
10. **Given** a repo with a configured remote, **When** running `gitr remote show origin`, **Then** remote URL, fetch/push refspecs, and tracking info are displayed.

---

### User Story 3 - Output Formatting Fidelity (Priority: P2)

A developer comparing gitr output with git output (visually or in scripts) expects identical formatting. Differences in date padding, stat alignment, graph rendering, and post-command summaries cause confusion and break automated tooling that parses git output.

**Why this priority**: While not functional blockers, formatting differences erode trust and break tools that parse git output (editors, CI, diff viewers). These should be fixed to complete the drop-in replacement promise.

**Independent Test**: Run each command in both git and gitr, capture stdout, and diff the outputs character-by-character. Assert zero differences.

**Acceptance Scenarios**:

1. **Given** a commit on February 9, **When** running `gitr log` (default format), **Then** the date shows `Feb 9` (no space-padding), matching git.
2. **Given** a commit on February 9, **When** running `gitr log --format=email`, **Then** the date shows `9 Feb` (no zero-padding), matching git.
3. **Given** a diff with file changes, **When** running `gitr diff --stat`, **Then** the change count alignment matches git exactly (minimal padding).
4. **Given** a commit with a stat, **When** running `gitr log --stat -1`, **Then** a blank line appears between the commit message and stat summary.
5. **Given** a branching history, **When** running `gitr log --graph --oneline --all`, **Then** the graph uses git's compact `|/` and `|\` notation with no extra blank lines between non-branching commits.
6. **Given** untracked files `a.txt`, `c.txt`, `b.txt`, **When** running `gitr clean -n`, **Then** files are listed alphabetically: `a.txt`, `b.txt`, `c.txt`.
7. **Given** an untracked directory `tempdir/` with files inside, **When** running `gitr clean -n` (without `-d`), **Then** files inside `tempdir/` are NOT listed.
8. **Given** a newly committed file, **When** running `gitr commit -m "test"`, **Then** output includes the `N files changed, N insertions(+)` summary and mode lines.
9. **Given** stashed changes, **When** running `gitr stash pop`, **Then** output includes full working tree status (not just the drop message).
10. **Given** a path with symlinks, **When** running `gitr init /tmp/test-repo`, **Then** the success message shows the resolved symlink path with trailing `/`.
11. **Given** a commit, **When** running `gitr format-patch HEAD~1`, **Then** the output filename has no `./` prefix.
12. **Given** a repo with multiple authors, **When** running `gitr shortlog`, **Then** commits within each author are ordered oldest-first.

---

### User Story 4 - Exit Code Compatibility (Priority: P2)

Scripts and CI pipelines rely on exit codes to determine success or failure. When gitr returns different exit codes than git for the same error conditions, automated workflows break silently or noisily.

**Why this priority**: Exit code mismatches cause subtle failures in shell scripts (`set -e`), CI pipelines, and any automation that branches on return codes.

**Independent Test**: Run each error scenario in both git and gitr and assert identical exit codes.

**Acceptance Scenarios**:

1. **Given** a nonexistent ref, **When** running `gitr show-ref --verify refs/heads/nonexistent`, **Then** exit code is 1, matching git.
2. **Given** a nonexistent branch, **When** running `gitr branch -d nonexistent`, **Then** exit code is 1, matching git.
3. **Given** a nonexistent ref, **When** running `gitr checkout nonexistent`, **Then** exit code is 1, matching git.
4. **Given** an invalid CLI argument, **When** running `gitr log --bogus-flag`, **Then** exit code is 128 (not 2), matching git.

---

### User Story 5 - Config and Init Platform Parity (Priority: P3)

A developer initializing a new repo or reading config expects the same defaults and config sources as git. Missing system-level config, platform-specific init settings, and incomplete `--show-origin` output cause subtle behavioral differences in downstream operations.

**Why this priority**: These are less frequently encountered but can cause confusing behavior when config values are unexpectedly missing or when a freshly initialized repo behaves differently than expected.

**Independent Test**: Run init and config commands in both git and gitr and compare config files and output.

**Acceptance Scenarios**:

1. **Given** a macOS system with Xcode CLT installed, **When** running `gitr config --list`, **Then** system-level config entries (from `/Library/Developer/CommandLineTools/usr/share/git-core/gitconfig`) are included.
2. **Given** a local config with `user.name` set, **When** running `gitr config --show-origin user.name`, **Then** output includes the origin file prefix (e.g., `file:.git/config	user.name=value`).
3. **Given** a macOS system, **When** running `gitr init test-repo`, **Then** `.git/config` includes `ignorecase = true` and `precomposeunicode = true`.
4. **Given** a freshly initialized repo, **When** inspecting `.git/hooks/`, **Then** standard sample hook files are present (e.g., `pre-commit.sample`, `commit-msg.sample`).

---

### Edge Cases

- What happens when `diff` is given both a revision and pathspec without `--` separator?
- How does `reset` handle ambiguous arguments that could be both a ref and a filename?
- What happens with `check-ignore` patterns using wildcards inside directories (e.g., `build/**/*.o`)?
- How does `log --since` handle timezone-aware date strings?
- What happens when `init` target path has no write permissions?
- How does `show --format` handle unknown format placeholders (e.g., `%Z`)?
- What happens with `log -0` (zero commits requested)?
- How does `clean -n` handle nested untracked directories?
- What happens when `commit-tree` receives both `-m` and stdin input?
- How does `rev-parse --short` handle hash collisions requiring longer abbreviations?

## Requirements *(mandatory)*

### Functional Requirements

**P0 - Core Command Fixes**

- **FR-001**: `diff` command MUST accept bare pathspecs (e.g., `diff file.txt`) without requiring `--` separator, resolving arguments as filenames when they are not valid revisions.
- **FR-002**: `diff` command MUST produce `@@ -0,0 +1,N @@` hunk headers when diffing new files against `/dev/null`.
- **FR-003**: `reset` command MUST accept bare pathspecs (e.g., `reset file.txt`) without requiring `--` separator, and MUST accept `reset HEAD file.txt` syntax.
- **FR-004**: `check-ignore` MUST correctly match directory patterns with trailing slash (e.g., `build/`) against both the directory itself and files within it.
- **FR-005**: `log --since` and `log --until` MUST filter commits by author date, returning only commits within the specified date range.
- **FR-006**: `init` MUST create the target directory hierarchy if it does not exist, matching git's behavior.
- **FR-007**: `show --stat` MUST suppress full diff output, showing only the stat summary.
- **FR-008**: `show` MUST include a `Merge: <abbreviated-parent1> <abbreviated-parent2>` header line for merge commits.
- **FR-009**: `commit-tree` MUST read the commit message from stdin when no `-m` flag is provided.
- **FR-010**: `show --format=oneline` MUST output the full 40-character hash (not abbreviated).
- **FR-011**: `show --format=raw` MUST indent the commit message body with 4 leading spaces.
- **FR-012**: `show --format` and `log --format` MUST support arbitrary printf-style format strings (e.g., `%H`, `%s`, `%an`, `%ae`, `%ad`, `%cn`, `%ce`, `%cd`, `%T`, `%t`, `%P`, `%p`, `%d`, `%D`, `%n`, `%%`, and all other standard git format placeholders).

**P1 - Missing Flags**

- **FR-013**: `log` MUST accept `-N` shorthand (e.g., `-1`, `-2`, `-10`) as equivalent to `-n N` / `--max-count=N`.
- **FR-014**: `log` MUST accept `--decorate` flag and display ref names (branches, tags, HEAD) beside commit hashes in `(ref1, ref2)` format.
- **FR-015**: `branch` MUST accept `-v` / `--verbose` flag and display abbreviated commit hash and subject line for each branch.
- **FR-016**: `tag` MUST accept `-n[num]` flag and display tag annotation lines.
- **FR-017**: `ls-tree` MUST accept `-l` / `--long` flag and display object sizes alongside entries.
- **FR-018**: `rev-parse` MUST accept `--abbrev-ref` flag and resolve symbolic refs to their short name.
- **FR-019**: `rev-parse --short` MUST work as an optional-value flag, accepting `--short` alone (defaulting to core.abbrev or 7) or `--short=N`.
- **FR-020**: `config` MUST accept `--local` flag to scope operations to the local `.git/config` file.
- **FR-021**: `format-patch` MUST accept `--stdout` flag to output patch content to stdout instead of files.
- **FR-022**: `remote` MUST support the `show` subcommand displaying URL, fetch/push refspecs, HEAD branch, and tracking info.

**P2 - Formatting Fixes**

- **FR-023**: Date formatting MUST use no-padding for day-of-month in default format (e.g., `Feb 9` not `Feb  9`).
- **FR-024**: Email date formatting MUST use no-padding for day-of-month (e.g., `9 Feb` not `09 Feb`).
- **FR-025**: `diff --stat` MUST use minimal padding before change counts, matching git's alignment.
- **FR-026**: `log --stat` MUST insert a blank line between the commit message and the stat summary.
- **FR-027**: `log --graph` MUST render branch visualization using git's compact notation (`|/`, `|\`, `|`) without extra blank lines between non-branching commits.
- **FR-028**: `clean -n` MUST sort output alphabetically.
- **FR-029**: `clean -n` (without `-d`) MUST NOT list files inside untracked directories.
- **FR-030**: `commit` output MUST include the `N files changed, N insertions(+), N deletions(-)` summary line and mode information.
- **FR-031**: `stash pop` MUST display full working tree status after applying the stash.
- **FR-032**: `init` success message MUST resolve symlinks in the path and include a trailing `/`.
- **FR-033**: `format-patch` output filenames MUST NOT include a `./` prefix.
- **FR-034**: `shortlog` MUST order commits oldest-first within each author group.

**P2 - Exit Codes**

- **FR-035**: `show-ref --verify` for nonexistent refs MUST exit with code 1.
- **FR-036**: `branch -d` for nonexistent branches MUST exit with code 1.
- **FR-037**: `checkout` for nonexistent refs MUST exit with code 1.
- **FR-038**: Invalid CLI arguments MUST cause exit code 128 (not clap's default of 2).

**P3 - Config / Init**

- **FR-039**: Config loading MUST include system-level gitconfig files (platform-specific paths).
- **FR-040**: `config --show-origin` MUST include the origin file prefix when querying a single key.
- **FR-041**: `init` on macOS MUST set `core.ignorecase = true` and `core.precomposeunicode = true` in the local config.
- **FR-042**: `init` MUST create standard sample hook files in `.git/hooks/`.

### Key Entities

- **Pathspec**: A file path argument that may be ambiguous with revision names; requires disambiguation logic matching git's "try as revision first, fall back to pathspec" algorithm.
- **Format String**: A printf-style template (e.g., `%H %s %an`) used by `log --format` and `show --format` to produce custom output.
- **Exit Code**: The process return code; git uses specific codes (0=success, 1=expected failure, 128=fatal error, 129=usage error) that scripts depend on.
- **System Config**: Platform-specific git configuration files loaded before user/local config in the config cascade.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All 12 P0 functional bug fixes produce byte-identical output to git in their respective E2E interop tests.
- **SC-002**: All 10 P1 missing flags are accepted without error and produce output matching git's behavior in E2E tests.
- **SC-003**: All 12 P2 formatting differences produce character-identical output to git in comparison tests.
- **SC-004**: All 4 exit code scenarios return the same exit code as git.
- **SC-005**: All 4 P3 config/init behaviors match git on macOS.
- **SC-006**: Zero regressions in existing E2E interop test suite (all pre-existing tests continue to pass).
- **SC-007**: Each fix includes at least one dedicated E2E interop test that runs both `git` and `gitr` with identical inputs and asserts matching outputs.
- **SC-008**: The complete test suite (existing + new) passes in CI within the established time budget.

## Assumptions

- The target platform for init/config platform-specific behavior is macOS (Darwin). Linux-specific system config paths are out of scope for this iteration but the approach should be extensible.
- "Byte-identical output" allows for differences in commit hashes, timestamps, and other inherently variable content. Comparison tests should normalize these values.
- The custom format string support (FR-012) covers the same set of placeholders that git's `pretty-formats` documents. Exotic or rarely-used placeholders may be added incrementally.
- Sample hook files created by `init` (FR-042) should match git's current set of sample hooks in content and naming.
- Exit code 128 for invalid CLI args (FR-038) may require a clap error handler override, which is an acceptable approach.
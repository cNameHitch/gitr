# Feature Specification: Diff Engine

**Feature Branch**: `011-diff-engine`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 003-object-model, 006-object-database, 007-index-staging-area, 010-repository-and-setup

## User Scenarios & Testing

### User Story 1 - Line-Level Diff (Priority: P1)

As a git user, I need to see line-by-line differences between two versions of a file so that I understand what changed.

**Why this priority**: `git diff` is one of the most-used commands. Line diff is the core algorithm.

**Independent Test**: Diff two known files, verify the output matches C git's `git diff` exactly.

**Acceptance Scenarios**:

1. **Given** two text files, **When** diffed with Myers algorithm, **Then** a minimal edit script is produced.
2. **Given** a unified diff output, **When** formatted, **Then** context lines, additions (+), and deletions (-) are shown correctly.
3. **Given** `--histogram` flag, **When** diffing, **Then** the histogram algorithm is used and output is identical to C git.
4. **Given** `--patience` flag, **When** diffing, **Then** the patience algorithm is used.
5. **Given** identical files, **When** diffed, **Then** no output is produced.

---

### User Story 2 - Tree Diff (Priority: P1)

As a gitr library, I need to diff two tree objects to determine which files were added, modified, deleted, or renamed between two commits.

**Why this priority**: Tree diffing underlies `git diff`, `git log --stat`, `git status`, and merge.

**Independent Test**: Compare two commits' trees, verify the list of changed files matches C git.

**Acceptance Scenarios**:

1. **Given** two tree objects, **When** diffed, **Then** added, deleted, modified, and type-changed entries are identified.
2. **Given** a tree and a null tree, **When** diffed, **Then** all entries are reported as added.
3. **Given** nested trees, **When** diffed recursively, **Then** changes at all depths are found.

---

### User Story 3 - Rename Detection (Priority: P2)

As a git user, I need rename and copy detection so that `git diff` shows file movements rather than delete+add.

**Why this priority**: Rename detection is a key git feature that makes history tracking useful.

**Independent Test**: Rename a file, commit, run diff, verify it's detected as a rename.

**Acceptance Scenarios**:

1. **Given** a file moved from A to B with identical content, **When** diffed with rename detection, **Then** it is shown as `rename A → B`.
2. **Given** a file moved and edited (>50% similar), **When** diffed, **Then** it is shown as rename with modifications.
3. **Given** `-M` threshold, **When** similarity is below threshold, **Then** it is shown as separate add/delete.
4. **Given** `-C` flag, **When** content was copied from another file, **Then** it is detected as a copy.

---

### User Story 4 - Diff Output Formats (Priority: P1)

As a git user, I need various diff output formats so that different use cases are served.

**Why this priority**: Different commands need different formats (unified, stat, raw, name-only).

**Independent Test**: Generate each format and compare against C git output.

**Acceptance Scenarios**:

1. **Given** a diff, **When** formatted as unified, **Then** the standard unified diff format is produced with `---`/`+++` headers.
2. **Given** a diff, **When** formatted as `--stat`, **Then** a summary showing files and +/- counts is produced.
3. **Given** a diff, **When** formatted as `--raw`, **Then** the raw format with modes and OIDs is produced.
4. **Given** a diff, **When** formatted as `--name-only`, **Then** only changed file paths are listed.
5. **Given** a diff, **When** formatted as `--name-status`, **Then** paths with M/A/D/R status are listed.

---

### User Story 5 - Working Tree Diff (Priority: P1)

As a git user, I need to diff the working tree against the index or HEAD so that `git diff` and `git diff --cached` work.

**Why this priority**: Everyday workflow depends on seeing uncommitted changes.

**Acceptance Scenarios**:

1. **Given** modified working tree files, **When** `git diff` runs, **Then** changes between index and working tree are shown.
2. **Given** staged changes, **When** `git diff --cached` runs, **Then** changes between HEAD and index are shown.
3. **Given** `git diff HEAD`, **When** run, **Then** changes between HEAD and working tree are shown.

---

### User Story 6 - Diffcore Pipeline (Priority: P2)

As a gitr library, I need the diffcore transformation pipeline for rename detection, path filtering, and diff manipulation.

**Why this priority**: The diffcore pipeline is how git processes raw tree diffs into meaningful output.

**Acceptance Scenarios**:

1. **Given** raw tree diff output, **When** passed through diffcore-rename, **Then** renames are detected.
2. **Given** a pathspec filter, **When** applied through diffcore-pathspec, **Then** only matching paths remain.

### Edge Cases

- Binary files (show "Binary files differ")
- Empty file vs non-empty file
- File mode change only (no content change)
- Submodule changes (gitlink mode 160000)
- Symlink targets in diff
- Very large files (>100MB)
- Files with no trailing newline
- Diff of identical trees (empty result)
- Merge commit diff (combined diff format)

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement Myers diff algorithm for line-level diff
- **FR-002**: System MUST implement histogram diff algorithm
- **FR-003**: System MUST implement patience diff algorithm
- **FR-004**: System MUST diff two tree objects recursively
- **FR-005**: System MUST detect file renames and copies (with configurable similarity threshold)
- **FR-006**: System MUST produce unified diff format output
- **FR-007**: System MUST produce --stat summary format
- **FR-008**: System MUST produce --raw format
- **FR-009**: System MUST produce --name-only and --name-status formats
- **FR-010**: System MUST diff working tree against index and HEAD
- **FR-011**: System MUST handle binary files (detect and report, no line diff)
- **FR-012**: System MUST support combined diff format for merge commits
- **FR-013**: System MUST implement the diffcore pipeline (rename, pathspec, break, order)
- **FR-014**: System MUST support color output in diffs
- **FR-015**: System MUST support custom diff drivers via gitattributes
- **FR-016**: System MUST support context lines configuration (-U<n>)

### Key Entities

- **DiffPair**: A pair of files to diff (old, new)
- **DiffResult**: Collection of file-level changes
- **FileDiff**: Changes to a single file (hunks, mode change, rename)
- **Hunk**: A contiguous region of changes
- **DiffAlgorithm**: Enum of diff algorithms (Myers, Histogram, Patience)

## Success Criteria

### Measurable Outcomes

- **SC-001**: Unified diff output matches C git byte-for-byte for a test corpus of 500+ diffs
- **SC-002**: Rename detection produces identical results to C git for all similarity thresholds
- **SC-003**: All diff output formats match C git
- **SC-004**: Diff of linux kernel tree (60K files) completes in < 5 seconds
- **SC-005**: Diff algorithms produce minimal edit scripts (verified against known-optimal solutions)

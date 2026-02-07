# Feature Specification: End-to-End Git Interoperability Tests

**Feature Branch**: `019-e2e-interop-tests`
**Created**: 2026-02-07
**Status**: Draft
**Input**: User description: "Add full end-to-end interoperability tests with C git. The test suite should verify that gitr is a byte-compatible drop-in replacement for git by running real git workflows end-to-end and comparing outputs/results between gitr and C git."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Basic Workflow Interop Verification (Priority: P1)

A contributor wants confidence that gitr handles the most common daily git workflow identically to C git: initializing a repo, staging files, committing, viewing status, and inspecting diffs. The test creates two parallel repositories — one driven by C git, one by gitr — executing the same sequence of operations, and asserts that every stdout output, exit code, and resulting on-disk repository state (object store, index, HEAD ref) is byte-identical.

**Why this priority**: The init-add-commit-status-diff cycle represents 80%+ of real-world git usage. If this workflow diverges, gitr cannot be a drop-in replacement.

**Independent Test**: Can be fully tested by running a sequence of init, add, commit, status, and diff commands in side-by-side repos and comparing all outputs. Delivers core compatibility confidence.

**Acceptance Scenarios**:

1. **Given** an empty temporary directory, **When** gitr runs `init`, `add`, `commit`, `status`, and `diff` in the same sequence as C git with identical inputs and environment variables, **Then** all stdout/stderr output and exit codes match byte-for-byte, and the resulting `.git` directory contents (objects, refs, index, HEAD) are identical.
2. **Given** a repo initialized by C git with committed files, **When** gitr runs `status` on that repo, **Then** the output is identical to C git's `status` output.
3. **Given** a repo initialized by gitr, **When** C git runs subsequent operations (add, commit, log) on that repo, **Then** C git operates correctly without errors, proving gitr-created repos are C git-compatible.

---

### User Story 2 - Branching and Merging Interop (Priority: P1)

A contributor wants to verify that branch creation, checkout/switch, and merge operations (including fast-forward, three-way, and conflict scenarios) produce identical results in gitr and C git. This covers the second most critical workflow after basic commits.

**Why this priority**: Branching and merging are fundamental collaboration workflows. Divergence here would make gitr unusable in team environments.

**Independent Test**: Can be fully tested by creating branches, making divergent commits, and performing merges in parallel repos, then comparing all outputs, exit codes, conflict markers, and resulting tree states.

**Acceptance Scenarios**:

1. **Given** a repo with a linear history, **When** gitr creates a branch and performs a fast-forward merge, **Then** the output and resulting refs match C git.
2. **Given** two branches with non-conflicting divergent changes, **When** gitr performs a three-way merge, **Then** the merge commit, tree content, and output match C git.
3. **Given** two branches with conflicting changes to the same file, **When** gitr performs a merge, **Then** the conflict markers in the working tree, the index state (stage entries), and the exit code match C git.

---

### User Story 3 - History Inspection Interop (Priority: P2)

A contributor wants to verify that log, show, blame, and other history inspection commands produce byte-identical output when run against the same repository state.

**Why this priority**: Developers rely on history inspection to understand code evolution. Divergent output would cause confusion but does not corrupt data.

**Independent Test**: Can be tested by building a repo with a known commit graph (linear, branching, merging) using C git, then running log/show/blame with various flags via both C git and gitr, comparing outputs.

**Acceptance Scenarios**:

1. **Given** a repo with 5+ commits (including merges), **When** gitr runs `log`, `log --oneline`, `log --graph --all`, and `log --format=...` with the same flags as C git, **Then** all outputs are byte-identical.
2. **Given** a file with multi-author history, **When** gitr runs `blame` on that file, **Then** the annotation output (OIDs, author, date, line content) is byte-identical to C git.
3. **Given** a commit OID, **When** gitr runs `show <oid>`, **Then** the commit header, message, and diff output match C git.

---

### User Story 4 - Plumbing Command Interop (Priority: P2)

A contributor wants to verify that low-level plumbing commands (cat-file, hash-object, rev-parse, ls-tree, ls-files, for-each-ref, update-index) produce identical outputs. Scripts and tools rely on these commands' exact output formats.

**Why this priority**: Plumbing commands are the backbone for tooling integrations (CI/CD, hooks, IDE plugins). Exact output compatibility is critical for scripting.

**Independent Test**: Can be tested by running each plumbing command against a known repo state and comparing outputs between gitr and C git.

**Acceptance Scenarios**:

1. **Given** a repo with known objects, **When** gitr runs `cat-file -t`, `cat-file -s`, and `cat-file -p` on each object type (blob, tree, commit, tag), **Then** all outputs match C git byte-for-byte.
2. **Given** a repo with branches, tags, and remotes, **When** gitr runs `for-each-ref` with various format strings, **Then** outputs match C git.
3. **Given** a complex revision expression (e.g., `HEAD~3^{tree}`, `main@{2}`), **When** gitr runs `rev-parse`, **Then** the resolved OID matches C git.

---

### User Story 5 - Remote Operations Interop (Priority: P2)

A contributor wants to verify that fetch, push, and clone work correctly between gitr and C git over local transport (file:// protocol). This ensures gitr can participate in workflows involving remote repositories.

**Why this priority**: Remote operations are essential for any collaborative workflow. Local transport testing avoids network complexity while still exercising the core protocol logic.

**Independent Test**: Can be tested by setting up bare repos, cloning with both gitr and C git, pushing/fetching between them, and comparing resulting refs and objects.

**Acceptance Scenarios**:

1. **Given** a bare repo created by C git with commits, **When** gitr clones it, **Then** the cloned repo's objects, refs, and working tree are identical to a C git clone.
2. **Given** a gitr clone with new local commits, **When** gitr pushes to the bare remote, **Then** C git can fetch those commits and the objects are valid.
3. **Given** new commits pushed by C git to a bare remote, **When** gitr fetches and fast-forward merges, **Then** the local state matches what C git would produce.

---

### User Story 6 - Advanced Operations Interop (Priority: P3)

A contributor wants to verify that advanced operations — rebase, cherry-pick, stash, annotated tags, and archive — produce results compatible with C git.

**Why this priority**: These are less frequent but important operations. Compatibility here rounds out the drop-in replacement story.

**Independent Test**: Can be tested by performing each operation in parallel repos and comparing all outputs, exit codes, and resulting repository state.

**Acceptance Scenarios**:

1. **Given** a branch with commits to rebase onto main, **When** gitr performs a rebase, **Then** the resulting linear history and tree content match what C git produces.
2. **Given** a working tree with uncommitted changes, **When** gitr runs `stash push` and then `stash pop`, **Then** the stash ref, working tree, and output match C git.
3. **Given** a commit, **When** gitr creates an annotated tag with a message, **Then** the tag object content and `cat-file -p` output match C git.

---

### User Story 7 - Edge Case and Stress Interop (Priority: P3)

A contributor wants to verify that gitr handles edge cases identically to C git: binary files, empty repositories, files with unicode paths, large files, symlinks, empty commits, and deeply nested directory structures.

**Why this priority**: Edge cases are where compatibility bugs typically hide. Coverage here prevents subtle breakage in real-world repositories.

**Independent Test**: Can be tested by constructing repos with each edge case using C git, then running gitr operations against them and comparing behavior.

**Acceptance Scenarios**:

1. **Given** a repo containing binary files (PNG, compiled object), **When** gitr runs `add`, `commit`, `diff`, `show`, and `cat-file -p` on them, **Then** behavior and outputs match C git (binary diff markers, correct blob storage).
2. **Given** a freshly initialized repo with no commits, **When** gitr runs `status`, `log`, `branch`, **Then** exit codes and error messages match C git.
3. **Given** a repo with files whose names contain unicode characters, spaces, and special characters, **When** gitr runs `ls-files`, `status`, and `add`, **Then** path encoding and outputs match C git.

---

### User Story 8 - Cross-Tool Repository Compatibility (Priority: P1)

A contributor wants to verify that repositories can be freely interchanged between gitr and C git — a repo created by gitr can be operated on by C git and vice versa, with no corruption or divergence. This is the ultimate "drop-in replacement" verification.

**Why this priority**: This is the core promise of gitr. If a user switches their git binary from C git to gitr, everything must continue working, and they must be able to switch back without issues.

**Independent Test**: Can be tested by alternating operations between gitr and C git on the same repo, running `fsck` after each tool switch, and verifying all operations succeed.

**Acceptance Scenarios**:

1. **Given** a repo where gitr performed init and 3 commits, **When** C git runs `fsck`, **Then** no errors or warnings are reported.
2. **Given** a repo with mixed gitr/C git history (alternating commits), **When** both tools run `log --all`, **Then** the output is identical.
3. **Given** a repo created by C git, **When** gitr runs `gc` and then C git runs `fsck`, **Then** no corruption is detected and all objects are accessible.

---

### Edge Cases

- What happens when gitr operates on a repo created by an older version of C git (v1.x object formats)?
- How does gitr handle packfiles created by C git (including thin packs, OFS_DELTA vs REF_DELTA)?
- What happens when running gitr on a repo with submodules initialized by C git?
- How does gitr handle repos with alternates configured by C git?
- What happens when C git encounters a packfile created by gitr's `gc`/`repack`?
- How does gitr handle a repo with a `.gitattributes` file specifying custom diff/merge drivers?
- What happens when file paths exceed 260 characters (Windows-style long path edge case)?
- How does gitr handle repos with grafts or replace objects created by C git?
- What happens when the index file was written by a different git version with different index format version (v2, v3, v4)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Test suite MUST provide shared test harness utilities (helper functions for running C git and gitr, comparing outputs, setting up repos) in a reusable module to avoid code duplication across test files.
- **FR-002**: Test suite MUST pin all environment variables that affect git output (GIT_AUTHOR_NAME, GIT_AUTHOR_EMAIL, GIT_AUTHOR_DATE, GIT_COMMITTER_NAME, GIT_COMMITTER_EMAIL, GIT_COMMITTER_DATE, TZ, LC_ALL, LANG) to fixed values to ensure deterministic output.
- **FR-003**: Each test MUST compare both stdout output and exit codes between gitr and C git for the same command invocation.
- **FR-004**: Tests involving repository state MUST compare on-disk state (object store hashes, ref values, index content) in addition to command output.
- **FR-005**: Test suite MUST verify cross-tool compatibility by running C git `fsck` on repos created/modified by gitr, and vice versa.
- **FR-006**: Test suite MUST cover the basic workflow: `init`, `add`, `commit`, `status`, `diff` with byte-identical output comparison.
- **FR-007**: Test suite MUST cover branching operations: `branch` (create, list, delete), `checkout`/`switch`, and `merge` (fast-forward, three-way, conflict).
- **FR-008**: Test suite MUST cover history inspection: `log` (with --oneline, --graph, --format, --stat flags), `show`, `blame`, `rev-list`.
- **FR-009**: Test suite MUST cover plumbing commands: `cat-file` (-t, -s, -p), `hash-object`, `rev-parse`, `ls-tree`, `ls-files`, `for-each-ref`, `write-tree`, `commit-tree`.
- **FR-010**: Test suite MUST cover remote operations using local file:// transport: `clone`, `fetch`, `push`, `pull`.
- **FR-011**: Test suite MUST cover advanced operations: `rebase`, `cherry-pick`, `stash` (push/pop/list), `tag` (lightweight and annotated), `revert`.
- **FR-012**: Test suite MUST cover edge cases: binary files, empty repositories, unicode file paths, empty commits, and deeply nested directories.
- **FR-013**: Test suite MUST be organized as integration tests under `crates/git-cli/tests/` to follow the existing project test structure.
- **FR-014**: Test suite MUST use `tempfile` crate for temporary directory management to ensure test isolation and cleanup.
- **FR-015**: Test suite MUST be runnable via `cargo test --workspace` without any external setup beyond having C git installed.
- **FR-016**: Test suite MUST handle stderr comparison where relevant (error messages, warnings) using both exact match and pattern match strategies as appropriate.
- **FR-017**: Test suite MUST support a comparison mode that allows both byte-exact comparison and a normalized comparison (stripping timestamps, OIDs where they are expected to differ due to timing).

### Key Entities

- **Test Harness**: Shared module providing functions to spawn C git and gitr processes with pinned environment, capture stdout/stderr/exit code, and compare results. Includes repo setup helpers for common scenarios (empty repo, linear history, branched history, merge conflicts).
- **Test Scenario**: A self-contained test case that sets up a repository state, executes a sequence of commands with both tools, and asserts equivalence. Each scenario is independent and idempotent.
- **Comparison Result**: The outcome of comparing gitr output to C git output for a given command. Includes stdout diff, stderr diff, exit code match, and optionally on-disk state diff (refs, objects, index).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All test scenarios pass in CI — gitr output matches C git output for 100% of covered command/flag combinations.
- **SC-002**: Test suite covers at least 30 distinct git commands across plumbing, porcelain, history, and advanced categories.
- **SC-003**: Repositories created by gitr pass C git `fsck --full` with zero errors in 100% of cross-compatibility test scenarios.
- **SC-004**: Repositories modified by gitr remain fully operable by C git (and vice versa) across at least 5 multi-step workflow scenarios.
- **SC-005**: Test suite completes within 5 minutes on a standard CI runner (parallel test execution enabled).
- **SC-006**: Every test uses deterministic environment pinning such that test results are reproducible across machines and CI runs.
- **SC-007**: Test suite adds at least 50 new test cases beyond the existing plumbing/porcelain/history test suites.

## Assumptions

- C git (version 2.x) is available on the PATH in all test environments (CI and local development).
- Local file:// transport is sufficient for testing remote operations; network protocols (SSH, HTTPS) are out of scope for this test suite.
- The existing test helper pattern (gitr_bin(), git(), gitr(), setup_test_repo()) will be refactored into a shared module to avoid duplication across 3+ test files.
- Tests use SHA-1 repositories; SHA-256 interop testing is out of scope for this spec.
- Submodule interop testing is deferred to a future spec due to complexity.
- The test suite targets the commands gitr currently implements; tests for unimplemented commands will be marked `#[ignore]` with a TODO note.

# Feature Specification: Comprehensive E2E Interop Test Coverage

**Feature Branch**: `021-e2e-interop-coverage`
**Created**: 2026-02-07
**Status**: Draft
**Input**: User description: "Write comprehensive e2e interop tests to close all gaps in the gitr Git implementation's test coverage. Currently ~44 of 73 implemented commands have e2e interop tests. The remaining ~29 commands need coverage to ensure gitr is a production-ready, fully interchangeable Git replacement."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Porcelain Command Interop Coverage (Priority: P1)

A developer using gitr as their daily Git replacement needs confidence that all commonly-used porcelain commands produce identical behavior to C git. This story covers the highest-impact untested commands: `clean`, `submodule`, `worktree`, and the `am`/`format-patch` email workflow. These commands are used frequently in real-world projects (CI pipelines, monorepos with submodules, patch-based code review).

**Why this priority**: These are the most commonly used commands that currently lack e2e testing. Submodule support alone is a blocker for many real-world projects, and `clean` is used in virtually every CI pipeline.

**Independent Test**: Can be fully tested by running each command with both gitr and C git on identical repos and comparing outputs byte-for-byte. Delivers confidence that gitr handles the most common Git workflows.

**Acceptance Scenarios**:

1. **Given** a repo with submodules configured, **When** gitr runs `submodule init`, `submodule update`, `submodule status`, `submodule sync`, `submodule deinit`, and `submodule foreach`, **Then** the output, exit codes, and resulting repo state match C git exactly.
2. **Given** a repo with untracked/ignored files, **When** gitr runs `clean -n`, `clean -f`, `clean -fd`, and `clean -fx`, **Then** the output and resulting file system state match C git exactly.
3. **Given** a repo with multiple branches, **When** gitr runs `worktree add`, `worktree list`, `worktree remove`, and `worktree prune`, **Then** the output and on-disk worktree structures match C git exactly.
4. **Given** a repo with commits, **When** gitr runs `format-patch` to create patches and `am` to apply them, **Then** the generated patches are byte-identical and the resulting repo state matches C git exactly.

---

### User Story 2 - Plumbing Command Interop Coverage (Priority: P2)

A tool author or script writer building automation on top of gitr needs assurance that low-level plumbing commands behave identically to C git. This story covers: `mktag`, `mktree`, `commit-tree`, `pack-objects`, `index-pack`, `update-index`, `update-ref`, `check-attr`, `check-ignore`, and `verify-pack`.

**Why this priority**: Plumbing commands are the foundation for scripts and tools that depend on Git internals. While less frequently used directly by humans, incorrect behavior here silently breaks automation.

**Independent Test**: Can be fully tested by piping identical inputs to both gitr and C git plumbing commands and comparing outputs and side effects (created objects, updated refs).

**Acceptance Scenarios**:

1. **Given** valid tag/tree/commit content on stdin, **When** gitr runs `mktag`, `mktree`, and `commit-tree`, **Then** the created object IDs and stored content match C git exactly.
2. **Given** a set of loose objects, **When** gitr runs `pack-objects` followed by `verify-pack` and `index-pack`, **Then** the generated packfile is structurally valid and objects are retrievable identically to C git.
3. **Given** a working tree with modified files, **When** gitr runs `update-index --add`, `update-index --remove`, and `update-index --refresh`, **Then** the index state matches C git exactly.
4. **Given** a repo with refs, **When** gitr runs `update-ref` to create, update, and delete refs, **Then** the ref state and reflog entries match C git exactly.
5. **Given** a repo with `.gitattributes` and `.gitignore`, **When** gitr runs `check-attr` and `check-ignore`, **Then** the output matches C git exactly.

---

### User Story 3 - Bundle, Archive & Notes Interop (Priority: P2)

A developer who needs to transfer repos offline (bundles), create release artifacts (archives), or annotate commits (notes) needs these operations to be interchangeable with C git. This story covers: `bundle create/verify/unbundle`, `archive --format=tar/zip`, `notes add/show/list/remove`, and `replace/replace -d`.

**Why this priority**: These are specialized but important commands for release engineering, offline workflows, and code review annotation. Bundle and archive are critical for air-gapped environments.

**Independent Test**: Can be fully tested by creating bundles/archives/notes with both tools and verifying the outputs are functionally equivalent (byte-identical where possible, structurally equivalent for binary formats).

**Acceptance Scenarios**:

1. **Given** a repo with history, **When** gitr runs `bundle create`, **Then** the bundle can be verified and unbundled by C git, and vice versa.
2. **Given** a repo tree, **When** gitr runs `archive --format=tar` and `archive --format=zip`, **Then** the archive contents match C git's output (same files, same content, same permissions).
3. **Given** a repo with commits, **When** gitr runs `notes add`, `notes show`, `notes list`, and `notes remove`, **Then** the output and notes ref state match C git exactly.
4. **Given** a repo with objects, **When** gitr runs `replace` and `replace -d`, **Then** the replacement refs and object resolution match C git exactly.

---

### User Story 4 - Maintenance & Integrity Interop (Priority: P2)

A repository maintainer running housekeeping operations needs confidence that gitr's maintenance commands produce repos that remain fully compatible with C git. This story covers: standalone `prune` (not via gc), and `fast-import` for bulk data ingestion.

**Why this priority**: Maintenance operations directly affect repository integrity. Incorrect prune behavior can cause data loss; broken fast-import blocks migration workflows.

**Independent Test**: Can be fully tested by running maintenance operations with both tools and verifying the resulting object database state matches.

**Acceptance Scenarios**:

1. **Given** a repo with unreachable objects, **When** gitr runs `prune`, **Then** the same objects are removed as C git would remove, and `fsck` passes on both.
2. **Given** a fast-import input stream with commits, trees, and blobs, **When** gitr runs `fast-import`, **Then** the resulting repository state matches C git exactly.

---

### User Story 5 - Hook Execution Interop (Priority: P3)

A developer relying on Git hooks for workflow enforcement needs gitr to execute hooks at the same trigger points as C git. This covers: `pre-commit`, `post-commit`, `pre-push`, and `commit-msg` hooks.

**Why this priority**: Hooks are critical for enforcing code quality, running linters, and triggering CI. However, they are lower priority because most hook failures are visible and diagnosable.

**Independent Test**: Can be fully tested by installing identical hook scripts in both gitr and C git repos, performing trigger operations, and comparing hook execution evidence (marker files, exit code behavior).

**Acceptance Scenarios**:

1. **Given** a repo with an executable `pre-commit` hook that creates a marker file, **When** gitr runs `commit`, **Then** the marker file is created (hook was executed).
2. **Given** a repo with a `pre-commit` hook that exits non-zero, **When** gitr runs `commit`, **Then** the commit is aborted (exit code 1) just like C git.
3. **Given** a repo with `post-commit` and `commit-msg` hooks, **When** gitr runs `commit`, **Then** hooks execute in the correct order with the correct arguments.

---

### User Story 6 - Large Repository Scalability (Priority: P3)

A developer working on large projects needs confidence that gitr handles repositories with many commits, branches, and files without diverging from C git behavior. This covers scalability testing with 100+ commits, many branches, and large directory trees.

**Why this priority**: Small test repos may mask edge cases in pagination, packfile handling, and ref iteration that only surface at scale. Lower priority because core correctness is validated by other stories.

**Independent Test**: Can be fully tested by generating large repos with both tools and comparing key operations (log, rev-list, branch listing, fsck).

**Acceptance Scenarios**:

1. **Given** a repo with 100+ sequential commits, **When** gitr runs `log --oneline`, `rev-list --count HEAD`, and `fsck`, **Then** all outputs match C git exactly.
2. **Given** a repo with 50+ branches, **When** gitr runs `branch --list` and `for-each-ref`, **Then** the output matches C git exactly.
3. **Given** a repo with 500+ files across nested directories, **When** gitr runs `ls-files`, `ls-tree -r HEAD`, and `status`, **Then** all outputs match C git exactly.

---

### User Story 7 - Config Scoping Interop (Priority: P3)

A developer with repository-local, global, and system Git configurations needs gitr to resolve config values using the same precedence rules as C git. This covers: local overrides global, `--local`/`--global` flags, and `config --list --show-origin`.

**Why this priority**: Config scoping bugs cause subtle, hard-to-diagnose issues. Lower priority because the basic config path is already tested.

**Independent Test**: Can be fully tested by setting configs at different scopes and comparing resolution output.

**Acceptance Scenarios**:

1. **Given** conflicting config values at local and global scope, **When** gitr runs `config --get`, **Then** the local value wins, matching C git behavior.
2. **Given** multiple config entries, **When** gitr runs `config --list --show-origin`, **Then** the output matches C git exactly (scope labels, file paths, values).

---

### Edge Cases

- What happens when `submodule update` targets a submodule whose remote URL is unreachable?
- What happens when `clean -f` is run with no untracked files present?
- What happens when `worktree add` targets a path that already exists?
- What happens when `bundle create` is run on an empty repo with no commits?
- What happens when `am` receives a malformed patch file?
- What happens when `prune` is run while objects are still reachable (should be no-ops)?
- What happens when `fast-import` receives an empty input stream?
- What happens when a hook script is not executable (no +x permission)?
- What happens when `pack-objects` is run with zero objects?
- What happens when `archive` is run on an empty tree?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: All new e2e tests MUST use the existing test harness (`common/mod.rs`) with deterministic environment pinning (dates, authors, timezone, locale).
- **FR-002**: All new e2e tests MUST compare gitr output against C git output using `assert_output_eq`, `assert_stdout_eq`, or `assert_exit_code_eq` helpers.
- **FR-003**: All new e2e tests MUST use `tempfile::tempdir()` for complete filesystem isolation.
- **FR-004**: Tests MUST verify cross-tool compatibility: repos created by gitr must be operable by C git, and vice versa.
- **FR-005**: Tests MUST validate that `git fsck --full` passes on repositories modified by gitr commands.
- **FR-006**: Each untested command MUST have at least one e2e interop test covering its primary use case.
- **FR-007**: Commands with multiple modes (e.g., `clean -f` vs `clean -fd` vs `clean -fx`) MUST have tests for each mode.
- **FR-008**: Error paths (non-zero exit codes) MUST be tested where C git returns specific error codes.
- **FR-009**: New test files MUST be organized by category (one file per user story) following the existing naming convention.
- **FR-010**: Tests for plumbing commands that accept stdin MUST pipe identical input to both gitr and C git.

### Assumptions

- C git is available on the test machine's PATH (consistent with existing test suite).
- The gitr binary is compiled and available via the `gitr_bin()` helper (consistent with existing test suite).
- All 73 commands are implemented and functional (this spec covers testing, not implementation).
- Network-dependent tests (SSH/HTTPS remotes) are out of scope; only `file://` protocol is used.
- GPG-dependent tests (`verify-commit`, `verify-tag`) are out of scope as they require key management infrastructure.
- `credential` command tests are out of scope as they require interactive terminal or credential store setup.
- `daemon` command tests are out of scope as they require network socket binding.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every implemented command that has a deterministic, non-interactive interface has at least one e2e interop test (target: 60+ commands covered, up from ~44).
- **SC-002**: All new tests pass with zero failures when run via `cargo test`.
- **SC-003**: All repositories created or modified by gitr in the new tests pass `git fsck --full` validation.
- **SC-004**: Cross-tool roundtrips succeed for all testable commands (gitr-created artifacts readable by C git, and vice versa).
- **SC-005**: The test suite runs in under 60 seconds total (including existing tests).
- **SC-006**: Zero ignored or skipped tests in the new test files.

# Research: End-to-End Git Interoperability Tests

**Feature**: 019-e2e-interop-tests
**Date**: 2026-02-07

## R-001: Shared Test Harness Design

**Decision**: Create a `tests/common/mod.rs` module under `crates/git-cli/tests/` containing all shared test utilities. Rust integration tests support shared modules via the `mod common;` pattern in each test file.

**Rationale**: The existing 3 test files duplicate identical helper functions (`gitr_bin()`, `git()`, `gitr()`, `gitr_full()`, `setup_test_repo()`, `setup_history_repo()`). This duplication will only worsen as we add 4+ new test files. Consolidating into a shared module ensures consistency and reduces maintenance burden.

**Alternatives considered**:
- **Separate test utility crate**: Too heavyweight for internal test helpers; adds workspace noise.
- **Build script generating helpers**: Over-engineered; plain module works fine.
- **Macro-based test generation**: Reduces boilerplate but hurts readability and debuggability.

## R-002: Test Organization Strategy

**Decision**: Add new e2e test files alongside existing ones in `crates/git-cli/tests/`, organized by test category:
- `e2e_workflow_tests.rs` — Multi-step workflows (P1 stories 1, 2, 8)
- `e2e_remote_tests.rs` — Clone, fetch, push, pull (P2 story 5)
- `e2e_advanced_tests.rs` — Rebase, stash, cherry-pick, annotated tags (P3 story 6)
- `e2e_edge_case_tests.rs` — Binary files, unicode, empty repos (P3 story 7)

Existing files (`plumbing_tests.rs`, `porcelain_tests.rs`, `history_tests.rs`) remain unchanged — they already provide good single-command interop coverage. The new e2e files focus on multi-step workflows and cross-tool scenarios that the existing tests do not cover.

**Rationale**: Adding files rather than modifying existing ones minimizes disruption and merge conflicts. The existing tests are working well and cover P2 stories 3 and 4 (history inspection and plumbing commands) already. The new suite fills the gaps: workflows, remotes, advanced ops, edge cases, and cross-tool compatibility.

**Alternatives considered**:
- **Single large test file**: Would become unwieldy (1000+ lines). Multiple files allow parallel compilation and focused CI debugging.
- **Replacing existing test files**: Unnecessary churn. Existing tests are passing and valuable.
- **Separate test crate**: Cargo workspace supports this but adds complexity for no benefit since `git-cli` already has a `tests/` directory.

## R-003: Environment Pinning Strategy

**Decision**: Extend the current environment variable pinning to include locale and timezone variables. The shared harness will set:
- `GIT_AUTHOR_NAME=Test Author`
- `GIT_AUTHOR_EMAIL=test@example.com`
- `GIT_AUTHOR_DATE=1234567890 +0000`
- `GIT_COMMITTER_NAME=Test Committer`
- `GIT_COMMITTER_EMAIL=test@example.com`
- `GIT_COMMITTER_DATE=1234567890 +0000`
- `TZ=UTC`
- `LC_ALL=C`
- `LANG=C`
- `GIT_CONFIG_NOSYSTEM=1` (prevent system-level git config from affecting tests)
- `HOME` set to tempdir (prevent user-level git config from affecting tests)

For tests requiring multiple commits with distinct timestamps, the harness will provide a helper that increments the date by 1 second per commit to ensure deterministic ordering.

**Rationale**: Locale and timezone differences between CI runners and developer machines are a common source of test flakiness. `GIT_CONFIG_NOSYSTEM=1` and `HOME` override prevent machine-specific git configs from interfering.

**Alternatives considered**:
- **Only pin author/committer (current approach)**: Insufficient — locale differences can cause output divergence in date formatting.
- **Use `--date` flags on each command**: More fragile and command-specific.

## R-004: Comparison Strategy

**Decision**: Implement three comparison modes in the harness:
1. **Exact**: Byte-for-byte stdout comparison (default for most commands).
2. **Normalized**: Strip or replace variable content (OIDs, timestamps) before comparison. Used for commands where gitr generates new objects with different OIDs than C git would (e.g., `commit` creates different OIDs due to metadata differences).
3. **Structural**: Verify structural properties (exit code, line count, field presence) without exact matching. Used for commands whose output includes platform-specific content.

**Rationale**: Most plumbing commands produce deterministic output given identical inputs (and pinned env vars), so exact comparison works. But operations that create new objects (commit, merge, rebase) may differ in OID if any metadata differs, requiring normalization.

**Alternatives considered**:
- **Always exact match**: Too brittle for operations that create new objects.
- **Always normalized**: Loses the precision that makes these tests valuable.
- **Snapshot testing (insta crate)**: Doesn't fit the dual-binary comparison model.

## R-005: Cross-Tool Compatibility Testing Approach

**Decision**: Use a "ping-pong" pattern where operations alternate between gitr and C git on the same repository, with `fsck` validation after each switch:
1. Tool A performs operation → `fsck` by Tool B → continue
2. Tool B performs operation → `fsck` by Tool A → continue

This validates that each tool's output is consumable by the other.

**Rationale**: This is the strongest possible test of drop-in compatibility. If both tools can operate on each other's output and both `fsck` passes, the repos are structurally compatible.

**Alternatives considered**:
- **Parallel repos only**: Misses cross-tool corruption scenarios.
- **Only fsck at end**: Misses which specific operation caused divergence.

## R-006: Remote Operation Testing via Local Transport

**Decision**: Test remote operations using `file://` protocol with bare repositories as remotes. Set up:
1. A bare "origin" repo (created by C git)
2. Two working clones (one by gitr, one by C git)
3. Push/fetch between them and compare resulting state

**Rationale**: `file://` transport exercises the real pack protocol (pack-objects, unpack-objects, ref negotiation) without network dependencies. This is the same approach used by git's own test suite.

**Alternatives considered**:
- **`git daemon` for git:// protocol**: Adds process management complexity.
- **Mock transport layer**: Doesn't test real interop.
- **HTTP transport with local server**: Too heavyweight for integration tests.

## R-007: Handling Unimplemented Commands

**Decision**: Tests for commands that are wired up in gitr but have incomplete implementations should be marked `#[ignore]` with a comment explaining which functionality is missing. This allows the test suite to be forward-looking without breaking CI.

**Rationale**: All 71 commands have dispatch entries in gitr, but some may have stub implementations. `#[ignore]` keeps tests visible (shown as "ignored" in test output) without failing the build.

**Alternatives considered**:
- **Feature-gated tests**: More complex, less visible.
- **Skip at runtime**: `#[ignore]` is the idiomatic Rust approach.
- **Don't write tests for stubs**: Loses the documentation value of what needs to pass.

## R-008: Existing Test Gap Analysis

**Currently covered** (by existing test files, ~75 tests):
- Single-command plumbing: cat-file, hash-object, rev-parse, show-ref, symbolic-ref, ls-files, ls-tree, check-ref-format, var, write-tree, commit-tree, update-ref
- Single-command porcelain: init, add, rm, status, commit, branch, switch, tag, reset, mv, clean, restore
- Single-command history: log, rev-list, show, diff, blame, shortlog, describe, grep, reflog, bisect, cherry-pick, revert, format-patch

**Gaps to fill** (new e2e suite):
- Multi-step workflows (init→add→commit→branch→merge cycle)
- Cross-tool repo exchange (gitr creates, C git operates, and vice versa)
- fsck validation of gitr-created repos
- Remote operations (clone, fetch, push, pull)
- Merge conflict scenarios with conflict marker comparison
- Rebase workflows
- Stash push/pop/list
- Annotated tag creation and verification
- Binary file handling
- Unicode path handling
- Empty repo edge cases
- Deeply nested directory structures
- gc/repack and resulting packfile compatibility

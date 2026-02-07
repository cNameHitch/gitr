# Implementation Plan: Comprehensive E2E Interop Test Coverage

**Branch**: `021-e2e-interop-coverage` | **Date**: 2026-02-07 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/021-e2e-interop-coverage/spec.md`

## Summary

Close all e2e interop test gaps in the gitr Git implementation by adding ~70 new test cases across 4 new test files, covering the 21 commands that currently lack e2e coverage. All tests follow the existing dual-repo comparison pattern: run identical operations with both gitr and C git, compare outputs byte-for-byte. Extend the common test harness with stdin piping helpers needed by plumbing commands.

## Technical Context

**Language/Version**: Rust 1.75+ (Cargo workspace, 19 crates)
**Primary Dependencies**: `tempfile` 3 (test isolation), `std::process::Command` (subprocess execution)
**Storage**: N/A (test-only feature)
**Testing**: `cargo test` — all tests are `#[test]` functions in `crates/git-cli/tests/`
**Target Platform**: macOS/Linux (wherever C git is available)
**Project Type**: Single Cargo workspace
**Performance Goals**: Full test suite (existing + new) under 60 seconds
**Constraints**: Deterministic output required (pinned dates, authors, locale)
**Scale/Scope**: ~70 new test cases across 4 new files, ~1 file modified (common/mod.rs)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Safety-First | PASS | Test code only; no unsafe, no library changes |
| II. C-Compatibility | PASS | Tests enforce byte-identical output between gitr and C git |
| III. Modular Crates | PASS | All new code in `git-cli` test directory; no crate changes |
| IV. Trait-Based Abstraction | N/A | No new traits or abstractions |
| V. Test-Driven | PASS | This IS the test coverage feature; increases coverage from ~44 to 60+ commands |

**Gate result**: PASS — no violations.

## Project Structure

### Documentation (this feature)

```text
specs/021-e2e-interop-coverage/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
crates/git-cli/tests/
├── common/
│   └── mod.rs                              # MODIFIED: add stdin helpers
├── e2e_porcelain_coverage_tests.rs         # NEW: US1 — clean, submodule, worktree, am/format-patch
├── e2e_plumbing_coverage_tests.rs          # NEW: US2 — mktag, mktree, commit-tree, pack/index/verify, update-index/ref, check-attr/ignore
├── e2e_bundle_archive_notes_tests.rs       # NEW: US3 — bundle, archive, notes, replace
├── e2e_maintenance_hooks_scale_tests.rs    # NEW: US4-7 — prune, fast-import, hooks, large repos, config scoping
├── e2e_workflow_tests.rs                   # EXISTING (unchanged)
├── e2e_remote_tests.rs                     # EXISTING (unchanged)
├── e2e_advanced_tests.rs                   # EXISTING (unchanged)
├── e2e_edge_case_tests.rs                  # EXISTING (unchanged)
├── history_tests.rs                        # EXISTING (unchanged)
├── parity_tests.rs                         # EXISTING (unchanged)
├── porcelain_tests.rs                      # EXISTING (unchanged)
└── plumbing_tests.rs                       # EXISTING (unchanged)
```

**Structure Decision**: All new code goes into the existing `crates/git-cli/tests/` directory following the established naming convention. One minor modification to `common/mod.rs` to add stdin pipe helpers. No new crates, no structural changes.

## Complexity Tracking

No violations to justify — all gates pass cleanly.

## Design Decisions

### D1: Stdin Pipe Helpers

Add `git_stdin()` and `gitr_stdin()` to `common/mod.rs` that accept `&[u8]` input and pipe it to the process stdin. This mirrors the existing `git()` / `gitr()` pattern but adds stdin support needed by plumbing commands (mktag, mktree, commit-tree, pack-objects, update-index --stdin, update-ref --stdin, fast-import).

### D2: Setup Helpers for New Scenarios

Add setup helpers to `common/mod.rs`:
- `setup_submodule_repo(dir, sub_dir)` — creates a repo with a submodule pointing to a file:// URL
- `setup_untracked_files(dir)` — creates a committed repo with untracked and ignored files for clean tests
- `setup_large_repo(dir, commits, branches, files)` — parameterized large repo generator

### D3: Archive Comparison Strategy

For `archive` tests, compare extracted file contents rather than raw archive bytes. Tar/zip headers contain timestamps that may differ between implementations. The spec requires "same files, same content, same permissions" — not byte-identical archive headers.

### D4: Bundle Cross-Tool Verification

For `bundle` tests, verify that gitr-created bundles can be unbundled by C git and vice versa, rather than comparing bundle bytes. Bundle format includes packfile data which may have different delta choices.

### D5: Test Grouping by File

| File | # Tests | Commands Covered |
|------|---------|-----------------|
| `e2e_porcelain_coverage_tests.rs` | ~22 | clean (5), submodule (7), worktree (5), am/format-patch (5) |
| `e2e_plumbing_coverage_tests.rs` | ~20 | mktag (2), mktree (2), commit-tree (2), pack-objects (3), index-pack (2), update-index (3), update-ref (3), check-attr (2), check-ignore (2), verify-pack (2) |
| `e2e_bundle_archive_notes_tests.rs` | ~16 | bundle (4), archive (4), notes (5), replace (3) |
| `e2e_maintenance_hooks_scale_tests.rs` | ~14 | prune (3), fast-import (3), hooks (4), large repos (3), config scoping (2) |
| **Total** | **~72** | **21 commands** |

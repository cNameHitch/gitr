# Quickstart: 021-e2e-interop-coverage

**Date**: 2026-02-07

## Overview

This feature adds ~72 new e2e interop tests across 4 test files, covering the 21 Git commands that currently lack e2e coverage in the gitr project.

## Prerequisites

- Rust 1.75+ with `cargo`
- C git installed and on PATH
- The gitr project compiles: `cargo build`

## Running the Tests

```bash
# Run all tests (existing + new)
cargo test

# Run only the new test files
cargo test --test e2e_porcelain_coverage_tests
cargo test --test e2e_plumbing_coverage_tests
cargo test --test e2e_bundle_archive_notes_tests
cargo test --test e2e_maintenance_hooks_scale_tests

# Run a specific test
cargo test --test e2e_porcelain_coverage_tests test_clean_force
```

## File Layout

| File | Tests | Commands |
|------|-------|----------|
| `crates/git-cli/tests/common/mod.rs` | Modified | stdin helpers, new setup functions |
| `crates/git-cli/tests/e2e_porcelain_coverage_tests.rs` | ~22 | clean, submodule, worktree, am, format-patch |
| `crates/git-cli/tests/e2e_plumbing_coverage_tests.rs` | ~20 | mktag, mktree, commit-tree, pack/index/verify, update-index/ref, check-attr/ignore |
| `crates/git-cli/tests/e2e_bundle_archive_notes_tests.rs` | ~16 | bundle, archive, notes, replace |
| `crates/git-cli/tests/e2e_maintenance_hooks_scale_tests.rs` | ~14 | prune, fast-import, hooks, large repos, config |

## Test Pattern

Every test follows the same pattern:

1. Create two identical temp directories
2. Set up identical repo state using C git
3. Run the command under test with C git in dir A
4. Run the same command with gitr in dir B
5. Compare outputs with `assert_output_eq()` (byte-identical stdout + exit code)
6. Optionally compare filesystem state or verify with `git fsck`

## Verification

After implementation, verify:

```bash
# All tests pass
cargo test 2>&1 | grep "test result"

# No ignored tests
cargo test 2>&1 | grep "ignored" | grep -v "0 ignored"

# Total test count increased
cargo test 2>&1 | grep "passed" | awk '{sum += $1} END {print "Total:", sum}'
# Expected: ~1280+ (up from 1210)
```

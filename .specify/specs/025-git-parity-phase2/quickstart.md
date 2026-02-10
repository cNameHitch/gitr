# Quickstart: Git Behavioral Parity — Phase 2

**Feature**: 025-git-parity-phase2 | **Date**: 2026-02-09

## Prerequisites

- Rust 1.75+ with Cargo
- Git v2.39+ (for comparison testing)
- macOS or Linux

## Build

```bash
cargo build --workspace
```

## Verify Current Gaps

Run comparison tests to see the 35 differences:

```bash
# Build gitr
cargo build --release

# Test missing flags (should fail before implementation)
./target/release/gitr --version            # Error: unexpected argument
./target/release/gitr switch -c test       # Error: unexpected argument
./target/release/gitr merge feature --no-edit  # Error: unexpected argument
./target/release/gitr config --global user.name  # Error: unexpected argument
./target/release/gitr log --date=iso       # Error: unexpected argument
./target/release/gitr log --merges         # Error: unexpected argument
./target/release/gitr diff --word-diff     # Error: unexpected argument
./target/release/gitr show -s HEAD         # Error: unexpected argument
./target/release/gitr branch --contains HEAD  # Error: unexpected argument
```

## Run Tests

```bash
# Unit tests
cargo test --workspace

# Specific parity tests (from previous phases)
cargo test --test parity_polish_tests
cargo test --test e2e_porcelain_coverage_tests

# Clippy
cargo clippy --workspace -- -D warnings
```

## Implementation Order

1. **Flag additions** (FR-001 through FR-013): Pure clap attribute changes, no logic
2. **Date parsing** (FR-014): Verify env var date parsing chain
3. **Reflog recording** (FR-015): Wire `append_reflog_entry` into HEAD-modifying ops
4. **Output formatting** (FR-016 through FR-034): Command output adjustments
5. **Integration tests**: E2E parity comparison tests for all 34 items

## Verification

After implementation, run the scripted comparison:

```bash
# Compare gitr vs git for each requirement
cargo test --test parity_phase2_tests
```

Each test creates an isolated temp repo, runs the same command with both `git` and `gitr`, and asserts identical output.

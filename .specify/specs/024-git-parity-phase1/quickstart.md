# Quickstart: Git Behavioral Parity Polish

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## Prerequisites

- Rust 1.75+ toolchain installed
- C git installed (for interop testing)
- macOS or Linux

## Build

```bash
cargo build --workspace
```

## Run Tests

### All tests (existing + new)
```bash
cargo test --workspace
```

### Only parity polish tests
```bash
cargo test --package git-cli --test parity_polish_tests
```

### Specific test categories
```bash
# P0 — Core command fixes
cargo test --package git-cli --test parity_polish_tests p0_

# P1 — Missing flags
cargo test --package git-cli --test parity_polish_tests p1_

# P2 — Formatting fixes
cargo test --package git-cli --test parity_polish_tests p2_

# P2 — Exit codes
cargo test --package git-cli --test parity_polish_tests exit_code_

# P3 — Config/init
cargo test --package git-cli --test parity_polish_tests p3_
```

## Verify Specific Fixes

### Pathspec disambiguation (FR-001/003)
```bash
# Create a test repo
tmp=$(mktemp -d)
cd "$tmp"
git init && git config user.name "Test" && git config user.email "test@test.com"
echo "hello" > file.txt
git add file.txt && git commit -m "init"
echo "world" >> file.txt

# These should work identically:
git diff file.txt
target/debug/gitr diff file.txt

git diff -- file.txt
target/debug/gitr diff -- file.txt

git diff HEAD -- file.txt
target/debug/gitr diff HEAD -- file.txt
```

### Exit code mapping (FR-038)
```bash
# Invalid flag should exit 128, not 2
target/debug/gitr log --bogus-flag; echo "Exit: $?"
git log --bogus-flag; echo "Exit: $?"
# Both should show: Exit: 128 (after error message)
```

### Date formatting (FR-023)
```bash
tmp=$(mktemp -d)
cd "$tmp"
git init && git config user.name "Test" && git config user.email "test@test.com"
echo "test" > file.txt && git add . && git commit -m "test"

# Compare date output — should show "Feb 9" not "Feb  9"
git log -1 --format="%ad"
target/debug/gitr log -1 --format="%ad"
```

### Log --decorate (FR-014)
```bash
git log --oneline --decorate
target/debug/gitr log --oneline --decorate
# Both should show (HEAD -> main) decoration
```

## Lint

```bash
cargo clippy --workspace
```

## Full Regression Check

```bash
# Run all existing tests + new tests
cargo test --workspace 2>&1 | tail -5

# Run existing parity tests specifically
cargo test --package git-cli --test parity_tests
cargo test --package git-cli --test e2e_workflow_tests
cargo test --package git-cli --test e2e_porcelain_coverage_tests
cargo test --package git-cli --test e2e_plumbing_coverage_tests
```

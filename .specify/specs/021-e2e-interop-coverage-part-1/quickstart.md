# Quickstart: End-to-End Git Interoperability Tests

**Feature**: 019-e2e-interop-tests
**Date**: 2026-02-07

## Prerequisites

- Rust 1.75+ with `cargo`
- C git 2.x on PATH (`git --version` should work)
- The gitr workspace builds successfully (`cargo build --workspace`)

## Running the Tests

### Run all tests (including new e2e suite)

```bash
cargo test --workspace
```

### Run only the e2e interop tests

```bash
cargo test -p git-cli --test e2e_workflow_tests
cargo test -p git-cli --test e2e_remote_tests
cargo test -p git-cli --test e2e_advanced_tests
cargo test -p git-cli --test e2e_edge_case_tests
```

### Run a specific test

```bash
cargo test -p git-cli --test e2e_workflow_tests -- test_name
```

### Run including ignored tests (for commands with known incomplete implementations)

```bash
cargo test -p git-cli --test e2e_advanced_tests -- --ignored
```

## File Layout

```text
crates/git-cli/tests/
├── common/
│   └── mod.rs              # Shared test harness (gitr_bin, git, gitr, assert_*, setup_*)
├── e2e_workflow_tests.rs   # Multi-step workflows: init→add→commit→branch→merge, cross-tool compat
├── e2e_remote_tests.rs     # Clone, fetch, push, pull over file:// transport
├── e2e_advanced_tests.rs   # Rebase, stash, cherry-pick, annotated tags, gc
├── e2e_edge_case_tests.rs  # Binary files, unicode paths, empty repos, nested dirs
├── plumbing_tests.rs       # (existing) Single-command plumbing tests
├── porcelain_tests.rs      # (existing) Single-command porcelain tests
└── history_tests.rs        # (existing) Single-command history tests
```

## Writing a New Test

1. Import the shared harness:

```rust
mod common;
use common::*;
```

2. Use a repo setup helper:

```rust
#[test]
fn my_new_test() {
    let dir = tempfile::tempdir().unwrap();
    setup_linear_history(dir.path(), 3); // 3 commits

    let git_result = git(dir.path(), &["log", "--oneline"]);
    let gitr_result = gitr(dir.path(), &["log", "--oneline"]);

    assert_output_eq(&git_result, &gitr_result);
}
```

3. For cross-tool tests:

```rust
#[test]
fn gitr_repo_passes_git_fsck() {
    let dir = tempfile::tempdir().unwrap();
    // Use gitr to create the repo
    gitr(dir.path(), &["init"]);
    // ... add, commit with gitr ...

    // Validate with C git
    let result = git(dir.path(), &["fsck", "--full"]);
    assert_eq!(result.exit_code, 0);
}
```

## Environment Variables

All tests automatically pin:

| Variable | Value | Purpose |
|----------|-------|---------|
| GIT_AUTHOR_NAME | Test Author | Deterministic author |
| GIT_AUTHOR_EMAIL | test@example.com | Deterministic email |
| GIT_AUTHOR_DATE | 1234567890 +0000 | Fixed timestamp |
| GIT_COMMITTER_NAME | Test Committer | Deterministic committer |
| GIT_COMMITTER_EMAIL | test@example.com | Deterministic email |
| GIT_COMMITTER_DATE | 1234567890 +0000 | Fixed timestamp |
| TZ | UTC | Deterministic timezone |
| LC_ALL | C | Deterministic locale |
| LANG | C | Deterministic locale |
| GIT_CONFIG_NOSYSTEM | 1 | Ignore system git config |
| HOME | (tempdir) | Ignore user git config |

# Test Harness API Contract

**Feature**: 019-e2e-interop-tests
**Date**: 2026-02-07
**Module**: `crates/git-cli/tests/common/mod.rs`

## Overview

This contract defines the public API of the shared test harness module used by all integration test files in `crates/git-cli/tests/`.

## Types

### CommandResult

```
CommandResult {
    stdout: String     — captured standard output
    stderr: String     — captured standard error
    exit_code: i32     — process exit code
}
```

## Process Execution Functions

### git(dir, args) → CommandResult

Run C git in `dir` with the given arguments. All environment variables are pinned for determinism.

- **Input**: `dir: &Path` — working directory; `args: &[&str]` — command arguments
- **Output**: `CommandResult` with stdout, stderr, and exit code
- **Side effects**: May modify filesystem under `dir`
- **Environment**: Full pinning applied (see Environment Pinning section)

### gitr(dir, args) → CommandResult

Run the gitr binary in `dir` with the given arguments. Same environment pinning as `git()`.

- **Input**: `dir: &Path` — working directory; `args: &[&str]` — command arguments
- **Output**: `CommandResult` with stdout, stderr, and exit code
- **Side effects**: May modify filesystem under `dir`

### gitr_bin() → PathBuf

Discover the path to the compiled gitr binary from cargo's target directory.

- **Input**: None
- **Output**: `PathBuf` to the gitr binary
- **Panics**: If the binary cannot be found

## Assertion Functions

### assert_output_eq(git_result, gitr_result)

Assert that stdout and exit code are byte-identical between C git and gitr results.

- **Input**: Two `CommandResult` references
- **Panics**: With a diff-style message if stdout or exit code differs

### assert_stdout_eq(git_result, gitr_result)

Assert that only stdout matches (ignoring exit code). Useful when exit code semantics differ.

- **Input**: Two `CommandResult` references
- **Panics**: With a diff-style message if stdout differs

### assert_exit_code_eq(git_result, gitr_result)

Assert that only exit codes match.

- **Input**: Two `CommandResult` references
- **Panics**: If exit codes differ

### assert_repo_state_eq(dir_a, dir_b)

Compare two repository directories for equivalent state: HEAD ref value, all refs under refs/, and the set of object IDs present.

- **Input**: Two `&Path` references to repo roots
- **Panics**: With details of first divergence found

### fsck(dir) → CommandResult

Run `git fsck --full` on the given directory using C git.

- **Input**: `dir: &Path`
- **Output**: `CommandResult`

### assert_fsck_clean(dir)

Run fsck and assert exit code is 0 with no error output.

- **Input**: `dir: &Path`
- **Panics**: If fsck reports errors

## Repository Setup Functions

### setup_empty_repo(dir)

Initialize a repo with `git init -b main` and basic config. No commits.

### setup_linear_history(dir, n)

Create a repo with `n` sequential commits, each adding/modifying a file. Deterministic content and timestamps.

### setup_branched_history(dir)

Create a repo with `main` (3 commits) and `feature` (2 commits diverging from commit 2). Non-conflicting changes.

### setup_merge_conflict(dir)

Create a repo with `main` and `feature` branches where both modify the same lines of the same file.

### setup_bare_remote(dir)

Create a bare repository suitable for push/fetch testing. Pre-populated with 2 commits.

### setup_binary_files(dir)

Create a repo with binary file content (random bytes with known seed for determinism).

### setup_unicode_paths(dir)

Create a repo with files named using unicode characters (e.g., `café.txt`, `日本語.txt`).

### setup_nested_dirs(dir)

Create a repo with 10+ levels of nested directories, each containing a file.

## Environment Pinning

All process execution functions apply these environment variables:

| Variable | Value |
|----------|-------|
| GIT_AUTHOR_NAME | Test Author |
| GIT_AUTHOR_EMAIL | test@example.com |
| GIT_AUTHOR_DATE | 1234567890 +0000 |
| GIT_COMMITTER_NAME | Test Committer |
| GIT_COMMITTER_EMAIL | test@example.com |
| GIT_COMMITTER_DATE | 1234567890 +0000 |
| TZ | UTC |
| LC_ALL | C |
| LANG | C |
| GIT_CONFIG_NOSYSTEM | 1 |
| HOME | (set to parent of working dir) |

### with_date(date_epoch) → environment override

Returns env var overrides with a specific GIT_AUTHOR_DATE and GIT_COMMITTER_DATE. Used for multi-commit scenarios requiring distinct timestamps.

### next_date(counter) → epoch string

Increment a counter and return `(1234567890 + counter) +0000`. Ensures deterministic but distinct commit timestamps.

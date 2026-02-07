# Data Model: End-to-End Git Interoperability Tests

**Feature**: 019-e2e-interop-tests
**Date**: 2026-02-07

## Entities

### TestHarness

The shared test infrastructure providing process execution, comparison, and repo setup utilities.

**Attributes**:
- `gitr_binary_path`: Path to the compiled gitr binary, discovered from cargo's target directory
- `pinned_env`: Fixed set of environment variables applied to every git/gitr invocation (author, committer, date, locale, timezone, config isolation)
- `incremental_date_counter`: Atomic counter used to generate distinct but deterministic timestamps for multi-commit scenarios

**Operations**:
- `git(dir, args) → CommandResult`: Execute C git with pinned env, return stdout/stderr/exit_code
- `gitr(dir, args) → CommandResult`: Execute gitr with pinned env, return stdout/stderr/exit_code
- `assert_output_eq(git_result, gitr_result)`: Assert stdout and exit code match exactly
- `assert_output_normalized(git_result, gitr_result, normalizers)`: Assert outputs match after applying normalization (OID stripping, timestamp normalization)
- `assert_exit_code_eq(git_result, gitr_result)`: Assert only exit codes match
- `assert_repo_state_eq(git_dir, gitr_dir)`: Compare on-disk state (refs, HEAD, objects) between two repos
- `fsck(dir) → CommandResult`: Run C git fsck on a directory

**Relationships**:
- Used by all TestScenario instances
- Contains RepoSetup helpers

### RepoSetup

Pre-built repository states for common test patterns.

**Variants**:
- `empty_repo(dir)`: Initialized repo with no commits
- `linear_history(dir, n_commits)`: Repo with N sequential commits modifying known files
- `branched_history(dir)`: Repo with main + feature branch, divergent changes
- `merge_conflict(dir)`: Repo with two branches modifying the same file lines
- `bare_remote(dir)`: Bare repo suitable as a push/fetch target
- `binary_files(dir)`: Repo with binary content (random bytes, PNG-like header)
- `unicode_paths(dir)`: Repo with files containing unicode characters in names
- `nested_dirs(dir)`: Repo with deeply nested directory structure (10+ levels)

**Attributes** (common across variants):
- `dir`: Path to the temporary directory
- `branch_name`: Explicit branch name (always "main" for determinism)
- `file_contents`: Map of file paths to their content at each commit

### CommandResult

The captured output of a single command execution.

**Attributes**:
- `stdout`: String — captured standard output
- `stderr`: String — captured standard error
- `exit_code`: i32 — process exit code
- `command`: String — the command string for error reporting

### ComparisonMode

Strategy for comparing CommandResults between gitr and C git.

**Variants**:
- `Exact`: Byte-for-byte comparison of stdout, stderr, and exit code
- `Normalized(normalizers)`: Apply transformations before comparison (strip OIDs, normalize dates)
- `Structural(checks)`: Verify structural properties (line count, field presence, exit code) without exact text matching

### TestScenario

A self-contained test case combining setup, execution, and assertions.

**Attributes**:
- `name`: Human-readable test name
- `setup`: RepoSetup variant to use
- `steps`: Ordered list of (command, comparison_mode) pairs
- `cross_tool_validation`: Whether to run fsck after each step

**Lifecycle**:
1. Create tempdir
2. Run setup to build repo state
3. For each step: run command with both C git and gitr, compare using specified mode
4. If cross_tool_validation: run fsck between steps
5. Tempdir auto-cleaned on drop

## Relationships

```text
TestHarness ─────────┐
  ├── git()          │
  ├── gitr()         │
  ├── assert_*()     │
  └── fsck()         │
                     ▼
TestScenario ───── uses ───── RepoSetup
  ├── steps[]                  ├── empty_repo
  ├── comparison_mode          ├── linear_history
  └── cross_tool_validation    ├── branched_history
                               ├── merge_conflict
         CommandResult         ├── bare_remote
           ├── stdout          ├── binary_files
           ├── stderr          ├── unicode_paths
           └── exit_code       └── nested_dirs
```

## State Transitions

```text
Repo States During a Test:

  [Empty Dir] ──init──▶ [Initialized] ──add+commit──▶ [Has History]
                                                          │
                                    ┌─────────────────────┤
                                    ▼                     ▼
                              [Branched]            [Remote Cloned]
                                    │                     │
                                    ▼                     ▼
                              [Merged/Conflicted]   [Pushed/Fetched]
                                    │
                                    ▼
                              [fsck Validated]
```

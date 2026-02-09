# Contract: Exit Code Mapping

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## Overview

Defines the exit code behavior for gitr commands to match C git's conventions exactly.

## Exit Code Table

| Code | Meaning | When Used |
|------|---------|-----------|
| 0 | Success | Command completed successfully |
| 1 | Expected failure / "false" result | `show-ref --verify` ref not found, `branch -d` nonexistent branch, `checkout` nonexistent ref, `diff --quiet` with changes, `grep` no matches |
| 128 | Fatal error | Invalid arguments (CLI parse error), unrecoverable errors, missing repo, permission denied |
| 129 | Usage error (git-specific) | Reserved but rarely used in practice |

## Command-Specific Exit Codes

### show-ref --verify (FR-035)
- Ref exists → exit 0, print `<oid> <refname>`
- Ref not found → exit 1, print `fatal: '<ref>' - not a valid ref` to stderr
- Current behavior: Already returns 1 via `verify_refs()` return value — verify correctness

### branch -d (FR-036)
- Branch exists and is merged → exit 0, delete branch
- Branch does not exist → exit 1, print `error: branch '<name>' not found.` to stderr
- Branch exists but not merged (without -D) → exit 1, print warning

### checkout (FR-037)
- Valid branch/commit → exit 0, switch to it
- Nonexistent ref → exit 1, print `error: pathspec '<name>' did not match any file(s) known to git` to stderr

### Invalid CLI arguments (FR-038)
- Current: clap exits with code 2
- Required: exit with code 128
- Implementation: Override clap's error handler in `main()`:
  ```
  match Cli::try_parse_from(preprocess_args()) {
      Ok(cli) => run(cli),
      Err(e) if e.kind() == ErrorKind::DisplayHelp => { e.print(); exit(0) }
      Err(e) if e.kind() == ErrorKind::DisplayVersion => { e.print(); exit(0) }
      Err(e) => { e.print(); exit(128) }
  }
  ```

## Error Message Format

Fatal errors must use the prefix `fatal: ` (lowercase, with colon and space):
```
fatal: ambiguous argument 'X': unknown revision or path not in the working tree.
fatal: not a git repository (or any of the parent directories): .git
fatal: '<ref>' - not a valid ref
```

Non-fatal errors use `error: ` prefix:
```
error: branch 'X' not found.
error: pathspec 'X' did not match any file(s) known to git
```

## Implementation Notes

- The `run()` function in `commands/mod.rs` returns `Result<i32>` where the integer is the exit code
- The `main()` function maps `Err(e)` to exit code 128
- Commands that need to return exit code 1 for "expected failure" must return `Ok(1)`, NOT `Err(...)`
- Help and version display should exit with code 0

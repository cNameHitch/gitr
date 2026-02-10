# Contract: Pathspec Disambiguation

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## Overview

Defines the argument disambiguation contract for commands that accept both revisions and pathspecs (primarily `diff` and `reset`).

## Algorithm

```
disambiguate(args: &[String], repo: &Repository) -> (Vec<Revision>, Vec<Pathspec>)

  Input: Non-flag arguments from the command line (before any `--` separator)

  1. If `--` is present in the original args:
     - Everything before `--` → revisions
     - Everything after `--` → pathspecs
     - Return immediately (no disambiguation needed)

  2. For each arg in order:
     a. Try resolve_revision(arg, repo)
     b. If Ok → classify as revision
     c. If Err:
        - If Path::new(arg).exists() in working directory → classify as pathspec
        - If neither:
          - If we've already seen at least one pathspec → classify as pathspec
          - Else → error: "fatal: ambiguous argument '{arg}': unknown revision or path not in the working tree."

  3. Constraint: Once an arg is classified as pathspec, all subsequent args
     must also be pathspecs (no interleaving rev-pathspec-rev)
```

## Command-Specific Behavior

### diff
- Accepts 0, 1, or 2 revisions + 0..N pathspecs
- `diff file.txt` → 0 revisions, 1 pathspec (working tree vs index)
- `diff HEAD file.txt` → 1 revision, 1 pathspec (HEAD vs working tree for file)
- `diff HEAD~1 HEAD file.txt` → 2 revisions, 1 pathspec (commit range diff for file)

### reset
- Accepts 0 or 1 revision + 0..N pathspecs
- `reset file.txt` → 0 revisions (implicit HEAD), 1 pathspec
- `reset HEAD file.txt` → 1 revision (HEAD), 1 pathspec
- `reset --soft HEAD~1` → 1 revision, 0 pathspecs (mode reset)

## Error Messages

Must match git's error format:
```
fatal: ambiguous argument 'X': unknown revision or path not in the working tree.
Use '--' to separate paths from revisions, like this:
'git <command> [<revision>...] -- [<file>...]'
```

## Exit Code

Disambiguation failure → exit code 128 (fatal error).

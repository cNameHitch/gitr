# Implementation Plan: History & Inspection Commands

**Branch**: `017-history-and-inspection-commands` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/017-history-and-inspection-commands/spec.md`

## Summary

Implement ~14 commands for viewing and manipulating history: log, show, diff, blame, bisect, shortlog, describe, grep, cherry-pick, revert, format-patch, am, reflog, and rev-list. These commands build on the revision walking, diff engine, and merge engine libraries.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: All `git-*` library crates, especially `git-revwalk`, `git-diff`, `git-merge`, `clap` 4.x, `anyhow`
**Testing**: Integration tests comparing against C git output
**Target Platform**: Linux, macOS, Windows
**Project Type**: Part of CLI binary
**Scale/Scope**: ~14 commands, ~4K lines

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Read-only commands are safe; cherry-pick/revert use merge engine |
| C-Compatibility | ✅ Pass | All output verified against C git |
| Modular Crates | ✅ Pass | Thin CLI wrappers over library crates |
| Trait-Based | ✅ Pass | Uses library APIs |
| Test-Driven | ✅ Pass | Integration tests for each command |

## Project Structure

### Source Code

```text
src/commands/
├── log.rs          # git log
├── show.rs         # git show
├── diff.rs         # git diff
├── blame.rs        # git blame
├── bisect.rs       # git bisect
├── shortlog.rs     # git shortlog
├── describe.rs     # git describe
├── grep.rs         # git grep
├── cherry_pick.rs  # git cherry-pick
├── revert.rs       # git revert
├── format_patch.rs # git format-patch
├── am.rs           # git am
├── reflog.rs       # git reflog
└── rev_list.rs     # git rev-list

tests/
├── history/
│   ├── log_tests.rs
│   ├── blame_tests.rs
│   ├── bisect_tests.rs
│   └── patch_roundtrip.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Log implementation | RevWalk + pretty-print from git-revwalk | Reuse library, no duplication |
| Blame algorithm | Port C git's incremental blame | Complex but well-understood algorithm |
| Bisect state | Files in .git/BISECT_* matching C git | Interop with C git |
| Grep implementation | Walk trees + regex matching | Could use ripgrep engine for speed |
| Format-patch | Generate unified diff with email headers | Standard mbox format |

# Implementation Plan: Core Porcelain Commands

**Branch**: `016-core-porcelain-commands` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/016-core-porcelain-commands/spec.md`

## Summary

Implement ~21 core user-facing commands that form the essential git workflow: init, clone, add, commit, status, branch, switch, checkout, merge, fetch, pull, push, remote, rebase, reset, tag, stash, clean, restore, rm, mv. These are the commands most git users use daily.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: All `git-*` library crates, `clap` 4.x, `anyhow`
**Storage**: N/A (delegates to libraries)
**Testing**: Integration tests comparing against C git
**Target Platform**: Linux, macOS, Windows
**Project Type**: Part of CLI binary
**Performance Goals**: Match C git command performance
**Constraints**: Output, arguments, and exit codes match C git.
**Scale/Scope**: ~21 commands, ~5K lines total

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Destructive commands warn or require confirmation |
| C-Compatibility | ✅ Pass | All output matches C git |
| Modular Crates | ✅ Pass | Commands delegate to library APIs |
| Trait-Based | ✅ Pass | Uses library traits |
| Test-Driven | ✅ Pass | Integration tests against C git |

## Project Structure

### Source Code

```text
src/commands/
├── init.rs         # git init
├── clone.rs        # git clone
├── add.rs          # git add
├── rm.rs           # git rm
├── mv.rs           # git mv
├── status.rs       # git status
├── commit.rs       # git commit
├── branch.rs       # git branch
├── switch.rs       # git switch
├── checkout.rs     # git checkout
├── merge.rs        # git merge
├── fetch.rs        # git fetch
├── pull.rs         # git pull
├── push.rs         # git push
├── remote.rs       # git remote
├── rebase.rs       # git rebase
├── reset.rs        # git reset
├── tag.rs          # git tag
├── stash.rs        # git stash
├── clean.rs        # git clean
└── restore.rs      # git restore

tests/
├── porcelain/
│   ├── init_clone.rs
│   ├── staging.rs
│   ├── commit.rs
│   ├── branch.rs
│   ├── remote.rs
│   └── rebase.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Command pattern | Each command = separate file with run() function | Easy to test, locate, and maintain |
| Error display | User-friendly messages with `anyhow` | Library errors are wrapped with context |
| Interactive mode | Use `dialoguer` for interactive prompts | Cross-platform terminal interaction |
| Editor launch | Use git_utils::subprocess for editor | Respects GIT_EDITOR, core.editor, VISUAL, EDITOR |
| Output formatting | Direct stdout writes, respecting --porcelain | Performance and compatibility |

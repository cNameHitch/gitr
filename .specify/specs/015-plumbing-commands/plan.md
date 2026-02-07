# Implementation Plan: Plumbing Commands

**Branch**: `015-plumbing-commands` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/015-plumbing-commands/spec.md`

## Summary

Implement ~17 low-level plumbing commands as part of the `git-cli` binary. These commands are thin wrappers around the library crates (git-odb, git-ref, git-index, etc.) and provide scriptable interfaces to git's internals. Each command is a clap subcommand in the main binary.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: All `git-*` library crates, `clap` 4.x, `anyhow`
**Storage**: N/A (delegates to library crates)
**Testing**: Integration tests running commands and comparing output to C git
**Target Platform**: Linux, macOS, Windows
**Project Type**: Part of CLI binary
**Performance Goals**: Match C git's command performance
**Constraints**: Output must be byte-identical to C git. Exit codes must match.
**Scale/Scope**: ~17 commands, ~2K lines total

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | CLI uses `anyhow` for errors, libraries use `thiserror` |
| C-Compatibility | Pass | Output and exit codes match C git |
| Modular Crates | Pass | Commands are thin wrappers, no business logic in CLI |
| Trait-Based | Pass | Uses library APIs, no new abstractions |
| Test-Driven | Pass | Integration tests compare against C git output |

## Project Structure

### Source Code

```text
src/
├── main.rs             # CLI entry point, clap setup
├── commands/
│   ├── mod.rs          # Command registry
│   ├── cat_file.rs     # git cat-file
│   ├── hash_object.rs  # git hash-object
│   ├── rev_parse.rs    # git rev-parse
│   ├── update_ref.rs   # git update-ref
│   ├── for_each_ref.rs # git for-each-ref
│   ├── show_ref.rs     # git show-ref
│   ├── symbolic_ref.rs # git symbolic-ref
│   ├── ls_files.rs     # git ls-files
│   ├── ls_tree.rs      # git ls-tree
│   ├── update_index.rs # git update-index
│   ├── check_ignore.rs # git check-ignore
│   ├── check_attr.rs   # git check-attr
│   ├── mktree.rs       # git mktree
│   ├── mktag.rs        # git mktag
│   ├── commit_tree.rs  # git commit-tree
│   ├── verify_pack.rs  # git verify-pack
│   ├── check_ref_format.rs # git check-ref-format
│   ├── var.rs          # git var
│   └── write_tree.rs   # git write-tree
tests/
├── plumbing/
│   ├── cat_file.rs
│   ├── hash_object.rs
│   ├── rev_parse.rs
│   └── ...
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Error handling | `anyhow` in CLI, `thiserror` in libraries | CLI doesn't need typed errors; display is sufficient |
| Output | Write directly to stdout/stderr | Performance, avoid buffering |
| Command structure | Each command in its own file, register via clap | Clean separation, easy to add commands |
| Batch mode | Streaming I/O (read line → process → output) | Memory-efficient for large inputs |

# Implementation Plan: Configuration System

**Branch**: `009-configuration-system` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/009-configuration-system/spec.md`

## Summary

Implement the `git-config` crate providing reading, writing, and querying of git configuration files across all scopes. This crate parses git's INI-like format, supports typed access, handles include directives, and respects environment variable overrides. It depends on foundation utilities for path handling and lock files.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils` (paths, lock files, byte strings), `bstr`, `thiserror`
**Storage**: File system (config files at various locations)
**Testing**: `cargo test`, compatibility tests against C git's t1300-config.sh
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Config load < 5ms for typical setup (3-4 files), key lookup < 1μs
**Constraints**: Must preserve file formatting on write. Case-insensitive keys, case-sensitive subsections.
**Scale/Scope**: ~3 C files replaced (config.c ~3K lines, repo-settings.c ~500 lines, environment.c ~400 lines) → ~2K Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | File writes through lock file, all parsing returns Result |
| C-Compatibility | ✅ Pass | Format parsing tested against C git config files |
| Modular Crates | ✅ Pass | `git-config` depends only on `git-utils` |
| Trait-Based | ✅ Pass | Config source trait for pluggable backends |
| Test-Driven | ✅ Pass | C git test suite t1300 as reference |

## Project Structure

### Source Code

```text
crates/git-config/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Public API: ConfigSet, ConfigScope
│   ├── parse.rs        # Config file parser
│   ├── file.rs         # Single config file representation
│   ├── set.rs          # Merged multi-scope config view
│   ├── write.rs        # Config file writer (preserves formatting)
│   ├── types.rs        # Typed value conversion (bool, int, path, color)
│   ├── include.rs      # include.path and includeIf processing
│   ├── env.rs          # Environment variable overrides
│   └── error.rs        # Error types
├── tests/
│   ├── parse_compat.rs    # Parse C git config files
│   ├── write_roundtrip.rs # Write → read → verify
│   ├── type_conversion.rs # Typed value tests
│   └── include_tests.rs   # Include directive tests
└── benches/
    └── config_bench.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Parser approach | Custom line-by-line parser | Git config format is simple; no need for a parser combinator |
| Internal representation | Preserve raw text + parsed entries | Must preserve comments and formatting on write |
| Scope merging | Lazy: parse each file, merge on lookup | Avoids parsing unused config files |
| Key normalization | Lowercase section.key, preserve subsection case | Matches C git behavior |
| Write strategy | Read file, modify in-memory, write through lock file | Atomic writes, preserves formatting |

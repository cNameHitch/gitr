# Implementation Plan: Git Command Parity

**Branch**: `020-git-parity` | **Date**: 2026-02-07 | **Spec**: `/specs/020-git-parity/spec.md`
**Input**: Feature specification from `.specify/specs/020-git-parity/spec.md`

## Summary

Make gitr a drop-in replacement for C git by implementing all missing functionality revealed by interop test gaps. The spec defines 23 functional requirements across 8 user stories (merge parity, diff hunk content, output format parity, packfile reading, remote operations, stash, plumbing parity, rebase). The approach is incremental: fix shared infrastructure (date format, path quoting), then foundational packfile reading, then P1 stories (merge, diff, output format, packfile), then P2 (remote, stash, plumbing), then P3 (rebase). All changes target existing crates — no new crates are introduced.

## Technical Context

**Language/Version**: Rust 1.75+ (Cargo workspace, 16 crates)
**Primary Dependencies**: `bstr` 1, `sha1`/`sha2` 0.10, `flate2` 1, `memmap2` 0.9, `crc32fast` 1, `clap` 4, `thiserror` 2 / `anyhow` 1, `rayon` 1
**Storage**: Git on-disk format (loose objects, packfiles v2, refs, index, config) — all file-based
**Testing**: `cargo test --workspace`, `tempfile` 3 for test isolation, interop tests via `std::process::Command`
**Target Platform**: macOS / Linux (CLI binary)
**Project Type**: Cargo workspace (16 library crates + 1 CLI binary crate)
**Performance Goals**: Byte-identical output to C git for all supported operations
**Constraints**: SHA-1 only, file:// transport only for remote ops, v2 pack index only
**Scale/Scope**: 25+ parity interop tests to pass, 47 implementation tasks across 11 phases

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Safety-First | ✅ PASS | All APIs use `Result<T, E>`. No `unsafe` introduced. Bounds checking on packfile reads via existing `memmap2` + offset validation. |
| II. C-Compatibility | ✅ PASS | This is the primary goal — achieving byte-identical output for all covered commands. |
| III. Modular Crates | ✅ PASS | All changes go into existing crates following the dependency graph. No new crates. No circular deps. |
| IV. Trait-Based Abstraction | ✅ PASS | Cross-pack resolver uses `Fn` callback (trait object compatible). No concrete types where polymorphism needed. |
| V. Test-Driven | ✅ PASS | 25+ interop tests defined in spec. Tests written alongside each user story implementation. |
| Error Handling | ✅ PASS | `thiserror` in library crates, `anyhow` only in CLI. Error chains preserved. |
| Performance | ✅ PASS | Packfile access via `memmap2`. Lazy delta resolution. No eager loading. |
| Byte String Handling | ✅ PASS | Path quoting uses `&[u8]` input. `bstr` for git paths. UTF-8 conversion only at display boundary. |
| Serialization | ✅ PASS | Manual serialization for all git formats. No serde for git objects. |

**Gate result**: ALL PASS — no violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/020-git-parity/
├── plan.md              # This file
├── research.md          # Phase 0 output — 15 research decisions
├── data-model.md        # Phase 1 output — 6 entity modifications
├── quickstart.md        # Phase 1 output — build/test/verify guide
├── contracts/
│   └── cli-behavior.md  # Phase 1 output — 15 CLI behavior contracts
└── tasks.md             # Phase 2 output — 47 tasks across 11 phases
```

### Source Code (repository root)

```text
crates/
├── git-utils/src/
│   ├── date.rs          # T001: Add DateFormat::Default variant
│   └── path.rs          # T003: Add quote_path() utility
├── git-pack/src/
│   └── pack.rs          # T004: Cross-pack REF_DELTA resolver
├── git-odb/src/
│   └── lib.rs           # T005: Wire cross-pack resolver
├── git-revwalk/src/
│   ├── pretty.rs        # T002: Change FormatOptions default
│   └── range.rs         # T038: Add ^{type} peeling parser
├── git-merge/src/
│   ├── content.rs       # T008: Verify conflict markers
│   └── strategy/ort.rs  # T009: Verify tree completeness
├── git-diff/src/
│   └── format/unified.rs # R2: Formatter already correct
├── git-transport/src/
│   └── local.rs         # T025: Fix file:// URL handling
├── git-cli/src/commands/
│   ├── merge.rs         # T006-T007: Output messages, exit codes
│   ├── diff.rs          # T011-T012: Blob reading, format passthrough
│   ├── log.rs           # T014-T016: Date, newlines, empty repo
│   ├── show.rs          # T017: Date format
│   ├── blame.rs         # T018: Date+time format
│   ├── status.rs        # T019: Detached HEAD OID
│   ├── ls_files.rs      # T020: Unicode path escaping
│   ├── for_each_ref.rs  # T037: HEAD exclusion
│   ├── rev_parse.rs     # T039: Wire peeling
│   ├── clone.rs         # T026-T027: Remote config, bare clone
│   ├── push.rs          # T028: Push over file://
│   ├── fetch.rs         # T029: Fetch over file://
│   ├── pull.rs          # T030: Pull integration
│   ├── stash.rs         # T032-T035: Reflog stack, pop, list, --include-untracked
│   └── rebase.rs        # T041-T042: Committer date, author preservation
└── git-cli/tests/
    ├── parity_tests.rs  # T010,T013,T021,T024,T031,T036,T040,T043: All interop tests
    └── common/mod.rs    # Shared test helpers
```

**Structure Decision**: Existing Cargo workspace structure. All changes modify existing files. The only new file is `parity_tests.rs` for interop tests (and potentially `crates/git-utils/src/path.rs` if it doesn't exist yet for `quote_path()`).

## Complexity Tracking

> No constitution violations — table not needed.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |

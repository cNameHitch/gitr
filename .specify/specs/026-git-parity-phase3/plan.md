# Implementation Plan: Full Git CLI Parity (Phase 3)

**Branch**: `026-git-parity-phase3` | **Date**: 2026-02-09 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `.specify/specs/026-git-parity-phase3/spec.md`

## Summary

Close all remaining gaps between gitr and git: integrate existing color/pager infrastructure into all commands, fix top-level CLI global flag conflicts, implement 60+ missing flags across 30+ existing commands, add 18 missing commands (plumbing and porcelain), and achieve behavioral parity for `.gitattributes`, `.mailmap`, config includes, hooks, merge strategies, GPG signing, credential helpers, and interactive modes. The approach extends all 16 existing crates with targeted modules — no new crates needed since color, pager, gitattributes, and config include infrastructure already exist.

## Technical Context

**Language/Version**: Rust 1.75+ (Cargo workspace, 16 crates — no new crates needed)
**Primary Dependencies**: `clap` 4 (CLI), `bstr` 1 (byte strings), `sha1`/`sha2` 0.10 (hashing), `flate2` 1 (zlib), `memmap2` 0.9 (packfile), `crc32fast` 1 (pack index), `thiserror` 2 / `anyhow` 1 (errors), `rayon` 1 (parallelism), `regex` 1 (grep/pickaxe), `tempfile` 3 (test isolation)
**Storage**: Git on-disk format (loose objects, packfiles v2, refs, index, config, reflog, attributes, mailmap) — all file-based
**Testing**: `cargo test --workspace`, end-to-end parity tests comparing gitr vs git output via `std::process::Command`
**Target Platform**: Linux, macOS (primary); Windows (best-effort for CRLF/path handling)
**Project Type**: Cargo workspace (16 crates → 17)
**Performance Goals**: Color/pager must not add measurable latency to command execution; pager handoff must be immediate
**Constraints**: Byte-identical output to C git for all supported operations; no external runtime dependencies beyond `gpg` (for signing) and system pager
**Scale/Scope**: 102+ functional requirements across 6 categories; 18 new commands; ~30 existing commands receiving new flags; modules across 10 existing crates (no new crates)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Phase 0 Check

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Safety-First** | PASS | All new APIs return `Result<T, E>`. No `unsafe` needed. Color output uses safe ANSI writes. Pager uses `std::process::Command` (RAII). GPG delegates to external binary via `Command`. |
| **II. C-Compatibility** | PASS | This IS the goal — every FR targets byte-identical output with C git. Color codes, pager behavior, flag semantics all mirror git 2.39+. |
| **III. Modular Crates** | PASS | No new crates. Color/pager already in `git-utils`, gitattributes in `git-index`, config includes in `git-config`. All additions are modules in existing crates following dependency graph. CLI-only concerns (interactive) stay in `git-cli`. |
| **IV. Trait-Based Abstraction** | PASS | Merge strategies already trait-based. Credential helpers follow git's external process protocol (no trait needed — delegates to external binary). Diff algorithms already enum-based. |
| **V. Test-Driven** | PASS | Each FR maps to acceptance scenarios defined in spec. E2E parity tests compare `gitr` vs `git` output. New crate gets unit tests. |

### Post-Phase 1 Check

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Safety-First** | PASS | No `unsafe` in any new code. All subprocess interactions via safe `Command` API. |
| **II. C-Compatibility** | PASS | Data model entities designed to produce byte-identical output. Color scheme matches git's defaults exactly. |
| **III. Modular Crates** | PASS | No new crates added. All extensions to existing crate boundaries remain clean. 16 crates unchanged. |
| **IV. Trait-Based Abstraction** | PASS | `AttributeProvider` trait considered but rejected as premature — single implementation sufficient. Can be extracted later if needed. |
| **V. Test-Driven** | PASS | Data model includes validation rules. Each entity maps to testable acceptance scenarios. |

## Project Structure

### Documentation (this feature)

```text
specs/026-git-parity-phase3/
├── plan.md              # This file
├── research.md          # Phase 0 output (completed)
├── data-model.md        # Phase 1 output (completed)
├── quickstart.md        # Phase 1 output (completed)
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
crates/
├── git-utils/
│   └── src/
│       ├── color.rs         # EXTEND: Add ColorConfig struct, per-command/slot config reading
│       ├── pager.rs         # EXISTS: Already complete — integrate into CLI main loop
│       └── mailmap.rs       # NEW: .mailmap parsing and lookup
├── git-config/
│   └── src/
│       └── include.rs       # EXISTS: Already complete — verify and add E2E tests
├── git-diff/
│   └── src/
│       ├── color.rs         # NEW: Colored diff output formatting using git-utils color
│       └── pickaxe.rs       # NEW: -S/-G pickaxe search implementation
├── git-merge/
│   └── src/
│       ├── strategy/
│       │   └── octopus.rs   # NEW: Octopus merge strategy
│       └── rerere.rs        # NEW: Recorded resolution database
├── git-repository/
│   └── src/
│       ├── gpg.rs           # NEW: GPG signing delegation
│       └── hooks.rs         # EXTEND: Complete hook lifecycle (partial exists)
├── git-transport/
│   └── src/
│       └── credential.rs    # EXTEND: Full helper invocation (parsing exists)
├── git-revwalk/
│   └── src/
│       ├── merge_base.rs    # NEW: Common ancestor computation
│       └── cherry.rs        # NEW: Cherry/cherry-pick filtering
├── git-index/
│   └── src/
│       ├── attributes.rs    # EXTEND: Add eol normalization, diff/merge drivers, filters (parsing exists)
│       └── sparse.rs        # NEW: Sparse checkout support
├── git-cli/
│   └── src/
│       ├── main.rs          # EXTEND: Pager setup, global flags (-p/-P/--work-tree/etc.)
│       ├── interactive.rs   # NEW: Interactive hunk selector (-p mode)
│       └── commands/
│           ├── apply.rs           # NEW
│           ├── cherry.rs          # NEW
│           ├── count_objects.rs   # NEW
│           ├── diff_files.rs      # NEW
│           ├── diff_index.rs      # NEW
│           ├── diff_tree.rs       # NEW
│           ├── difftool.rs        # NEW
│           ├── fmt_merge_msg.rs   # NEW
│           ├── ls_remote.rs       # NEW
│           ├── maintenance.rs     # NEW
│           ├── merge_base.rs      # NEW
│           ├── merge_file.rs      # NEW
│           ├── merge_tree.rs      # NEW
│           ├── name_rev.rs        # NEW
│           ├── range_diff.rs      # NEW
│           ├── read_tree.rs       # NEW
│           ├── request_pull.rs    # NEW
│           ├── rerere.rs          # NEW
│           ├── sparse_checkout.rs # NEW
│           ├── stripspace.rs      # NEW
│           └── whatchanged.rs     # NEW
│
└── tests/
    ├── e2e_color_pager.rs         # NEW: Color and pager parity tests
    ├── e2e_missing_flags.rs       # NEW: Flag parity tests
    ├── e2e_new_commands.rs        # NEW: New command parity tests
    └── e2e_behavioral_parity.rs   # NEW: Behavioral feature parity tests
```

**Structure Decision**: Extends existing 16-crate workspace with no new crates. Key infrastructure already exists: color (`git-utils/src/color.rs`), pager (`git-utils/src/pager.rs`), gitattributes (`git-index/src/attributes.rs`), config includes (`git-config/src/include.rs`). Phase 3 work is primarily integration (wiring existing infra into commands) and extension (adding missing behaviors to existing modules). New CLI commands follow the established pattern in `git-cli/src/commands/`. The CLI binary remains a thin dispatch layer with no business logic — all heavy lifting delegated to library crates.

## Complexity Tracking

> **No constitution violations. Table included for documentation of architectural decisions.**

| Decision | Rationale | Alternative Rejected |
|----------|-----------|---------------------|
| Extend existing `git-utils/color.rs` not new crate | Infrastructure already exists; only need `ColorConfig` struct | `git-color` crate — code already in `git-utils`, no reason to extract |
| Extend existing `git-index/attributes.rs` not new crate | Parsing/lookup already exists; only need behavioral hooks (eol, drivers) | `git-attributes` crate — would require moving existing code for no benefit |
| Mailmap in `git-utils` not new crate | ~100 LOC parser + lookup table | `git-mailmap` crate — insufficient scope for standalone crate |
| Interactive modes in `git-cli` not crate | CLI-only concern per constitution principle III | `git-interactive` crate — violates principle III (output concern in library) |
| GPG via external process not native crypto | Matches git's approach, avoids crypto dep | Native `gpg` crate — massive dep, not needed since git itself delegates |
| No new crates in phase 3 | All subsystems have existing homes; dependency graph stays clean | Adding crates — would add workspace churn for features that fit naturally in existing crates |

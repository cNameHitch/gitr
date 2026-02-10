# Implementation Plan: Git Behavioral Parity — Phase 2

**Branch**: `025-git-parity-phase2` | **Date**: 2026-02-09 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification covering 34 in-scope parity fixes (FR-001 through FR-034)

## Summary

Close all remaining behavioral gaps between gitr and git v2.39.5. The audit identified 35 differences: 16 missing flags/features that cause hard errors, and 19 output format mismatches. This plan implements 34 fixes (FR-035 `add -p` is deferred). The work spans 6 crates: `git-cli` (command argument additions and output formatting), `git-utils` (date format extension), `git-revwalk` (graph fix, fuller format), `git-diff` (word-diff), `git-config` (unset/global), and `git-ref` (reflog recording).

## Technical Context

**Language/Version**: Rust 1.75+ (Cargo workspace, 16 crates)
**Primary Dependencies**: clap 4 (CLI), bstr 1 (byte strings), chrono (date formatting via git-utils), thiserror 2 / anyhow 1 (errors), git-diff (diffstat), git-revwalk (format/graph), git-ref (reflog)
**Storage**: Git on-disk format (loose objects, packfiles, refs, index, config, reflog) — all file-based
**Testing**: `cargo test --workspace` + E2E interop comparison tests (tempfile + std::process::Command)
**Target Platform**: macOS (Apple Git v2.39.5), Linux
**Project Type**: Cargo workspace (16 crates)
**Performance Goals**: N/A — correctness-focused feature; no new hot paths
**Constraints**: Output must match git v2.39.5 character-for-character (excluding hashes/timestamps)
**Scale/Scope**: 34 discrete fixes across ~20 source files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Safety-First | **PASS** | All changes use safe Rust. No new `unsafe`. All new operations return `Result<T, E>`. |
| II. C-Compatibility | **PASS** | This feature's entire purpose is achieving byte-identical output with C git. |
| III. Modular Crates | **PASS** | Changes respect crate boundaries: date logic in git-utils, format logic in git-revwalk, diff logic in git-diff, CLI wiring in git-cli. No circular deps introduced. |
| IV. Trait-Based Abstraction | **PASS** | No new trait requirements. Existing trait boundaries maintained. |
| V. Test-Driven | **PASS** | E2E interop tests compare gitr output against git for every FR. Unit tests for new date format, word-diff, config unset. |

**Post-Phase 1 Re-check**: All gates still pass. No new dependencies, no new crates, no architectural changes.

## Project Structure

### Documentation (this feature)

```text
specs/025-git-parity-phase2/
├── plan.md              # This file
├── research.md          # Phase 0: 17 research decisions
├── data-model.md        # Phase 1: modified entities and behavioral requirements
├── quickstart.md        # Phase 1: build and verification guide
└── tasks.md             # Phase 2 output (created by /speckit.tasks)
```

### Source Code (repository root)

```text
crates/
├── git-cli/src/
│   ├── main.rs                    # FR-001: --version flag
│   └── commands/
│       ├── merge.rs               # FR-002: --no-edit flag + FR-019: diffstat output
│       ├── revert.rs              # FR-003: --no-edit flag
│       ├── switch.rs              # FR-004: -c short flag
│       ├── config.rs              # FR-005: --unset + FR-006: --global
│       ├── log.rs                 # FR-007/008/009/010: --date, --merges, --no-merges, -- <path>
│       ├── diff.rs                # FR-011: --word-diff
│       ├── show.rs                # FR-012: -s flag + FR-030: tag message indent
│       ├── branch.rs              # FR-013: --contains
│       ├── commit.rs              # FR-014: ISO date env vars + FR-016/017: diffstat output
│       ├── reflog.rs              # FR-015: reflog display (already works, needs recording)
│       ├── cherry_pick.rs         # FR-020: branch name in output
│       ├── stash.rs               # FR-021: full status + 40-char hash
│       ├── reset.rs               # FR-022/023: mixed/hard output messages
│       ├── status.rs              # FR-024/027/034: rename detection, sorting, unstage hint
│       ├── rebase.rs              # FR-025: progress format + success message
│       ├── gc.rs                  # FR-026: silent output
│       ├── describe.rs            # FR-028: error message fix
│       ├── tag.rs                 # FR-029: lightweight tag -n commit subject
│       ├── shortlog.rs            # FR-032: stdin reading
│       └── init.rs                # FR-033: symlink resolution
├── git-utils/src/
│   └── date.rs                    # FR-007: Custom(String) DateFormat variant
├── git-revwalk/src/
│   ├── pretty.rs                  # FR-031: Merge: line in fuller format
│   └── graph.rs                   # FR-018: fix extra | lines
├── git-diff/src/
│   ├── lib.rs                     # FR-011: WordDiff output format variant
│   └── format/word_diff.rs        # FR-011: word-level diff formatter (new file)
├── git-config/src/
│   └── lib.rs                     # FR-005: unset() method
└── git-ref/src/
    └── reflog.rs                  # FR-015: append_reflog_entry (exists, needs wiring)

crates/git-cli/tests/
└── parity_phase2_tests.rs         # E2E comparison tests for all 34 FRs (new file)
```

**Structure Decision**: Existing Cargo workspace structure with 16 crates. Changes touch 6 crates with no new crate additions. One new file (`word_diff.rs`) and one new test file.

## Implementation Phases

### Phase A: Flag Additions (FR-001 through FR-013)

Pure clap attribute and argument additions. No behavioral logic changes.

1. **FR-001**: Add `#[command(version = ...)]` to `Cli` struct
2. **FR-002**: Add `--no-edit` to `MergeArgs`
3. **FR-003**: Add `--no-edit` to `RevertArgs`
4. **FR-004**: Add `short = 'c'` to `SwitchArgs::create`
5. **FR-005**: Add `--unset` to `ConfigArgs` + implement `unset()` in git-config
6. **FR-006**: Add `--global` to `ConfigArgs` + wire global scope reads/writes
7. **FR-007**: Add `--date` to `LogArgs` + `DateFormat::Custom` variant
8. **FR-008**: Add `--merges` to `LogArgs` + filter logic
9. **FR-009**: Add `--no-merges` to `LogArgs` + filter logic
10. **FR-010**: Wire `-- <path>` filtering in log (pathspecs already parsed but unused)
11. **FR-011**: Add `--word-diff` to `DiffArgs` + word-diff formatter
12. **FR-012**: Add `short = 's'` to `ShowArgs::no_patch`
13. **FR-013**: Add `--contains` to `BranchArgs` + ancestor check filtering

### Phase B: Date & Reflog (FR-014, FR-015)

1. **FR-014**: Ensure `GitDate::parse_raw()` falls back to `GitDate::parse()` for ISO 8601
2. **FR-015**: Wire `append_reflog_entry()` into all HEAD-modifying operations (commit, checkout, switch, reset, merge, rebase, cherry-pick, stash pop)

### Phase C: Output Format Parity (FR-016 through FR-034)

1. **FR-016**: Add diffstat to `commit` output via `print_summary()`
2. **FR-017**: Add Date line + diffstat to `commit --amend` output
3. **FR-018**: Fix `GraphDrawer::draw_commit()` extra `|` lines
4. **FR-019**: Add diffstat after merge strategy message
5. **FR-020**: Include branch name in cherry-pick output
6. **FR-021**: Full working tree status + 40-char hash in stash pop
7. **FR-022**: Show "Unstaged changes after reset:" for mixed reset
8. **FR-023**: Show `HEAD is now at <hash> <subject>` for hard reset
9. **FR-024**: Enable rename detection in `status --short`
10. **FR-025**: Add `Rebasing (N/M)` progress + success message to rebase
11. **FR-026**: Make gc silent by default (suppress progress messages)
12. **FR-027**: Sort untracked files alphabetically + collapse directories in status
13. **FR-028**: Fix describe error message wording (no doubled `fatal:`)
14. **FR-029**: Show commit subject for lightweight tags in `tag -n`
15. **FR-030**: Remove 4-space indent from annotated tag message in `show`
16. **FR-031**: Add `Merge:` line to `log --pretty=fuller` for merge commits
17. **FR-032**: Read from stdin in shortlog when non-tty
18. **FR-033**: Resolve symlinks in init output path, omit `.git` suffix
19. **FR-034**: Use `git rm --cached` unstage hint for initial commits in status

### Phase D: Testing

1. Create `parity_phase2_tests.rs` with E2E comparison tests for all 34 FRs
2. Each test: create temp repo → run command with git → run with gitr → assert output match
3. Run full regression suite to verify no existing tests broken

## Complexity Tracking

> No constitution violations. All changes are within existing crate boundaries.

| Aspect | Assessment |
|--------|-----------|
| New crates | 0 |
| New files | 2 (word_diff.rs formatter, parity_phase2_tests.rs) |
| Modified crates | 6 (git-cli, git-utils, git-revwalk, git-diff, git-config, git-ref) |
| Modified command files | ~20 |
| Estimated new LOC | ~1500-2000 (mostly test code) |
| Risk areas | Word-diff algorithm, graph drawer fix, reflog wiring (many touch points) |

# Research: 021-e2e-interop-coverage

**Date**: 2026-02-07

## R1: Implementation Status of Untested Commands

**Decision**: All 21 untested commands are fully implemented and functional. No stubs or TODOs found.

**Rationale**: Each command was inspected for completeness. All parse CLI args via clap, integrate with git object/ref storage APIs, and handle errors with Result types.

**Details**:

| Command | Status | Flags/Modes |
|---------|--------|-------------|
| clean | Complete | `-f`, `-d`, `-n`, `-x`, `-X`, `-q` |
| submodule | Complete | add, status, init, deinit, update, foreach, sync, summary |
| worktree | Complete | add, list, remove, lock, unlock, move, prune |
| am | Complete | `--abort`, `--continue`, `--skip`, `--three-way` |
| format-patch | Complete | `-o`, `--cover-letter`, `-n`, `--thread`, `--subject-prefix` |
| bundle | Complete | create, verify, list-heads, unbundle (v2/v3) |
| notes | Complete | list, add, show, remove, copy, append, prune, get-ref |
| replace | Complete | `-l`, `-d`, `-f`, `--graft`, `--format` |
| prune | Complete | `-n`, `-v`, `--progress`, `--expire` |
| archive | Complete | `--format=tar/tar.gz/zip`, `--prefix`, `-o` |
| mktag | Complete | stdin only, validates target object |
| mktree | Complete | `--missing`, `--batch`, `-z` |
| commit-tree | Complete | `-p`, `-m`, `-F`, stdin message |
| pack-objects | Complete | `--stdout`, `--revs`, `--all`, `--window`, `--depth`, `--threads` |
| index-pack | Complete | `-v`, `-o`, `--keep`, `--verify`, `--strict`, `--stdin` |
| update-index | Complete | `--add`, `--remove`, `--cacheinfo`, `--stdin`, `--refresh` |
| update-ref | Complete | `-d`, `--stdin` (transactions), `-m`, `--no-deref` |
| check-attr | Complete | `-a`, `--stdin`, `-z` |
| check-ignore | Complete | `-v`, `-n`, `--stdin`, `-z` |
| verify-pack | Complete | `-v`, `-s` |
| fast-import | Complete | `--quiet`, `--stats`, `--import-marks`, `--export-marks`, `--done` |

## R2: Stdin Piping in Test Harness

**Decision**: No existing stdin helper in `common/mod.rs`. Tests requiring stdin must use `std::process::Command` directly with `Stdio::piped()`.

**Rationale**: The existing `git()` and `gitr()` helpers use `cmd.output()` which doesn't support stdin. Adding a `git_stdin()` / `gitr_stdin()` helper pair would follow the existing pattern cleanly.

**Alternatives considered**:
- Modifying existing helpers to accept optional stdin: rejected, would break backward compatibility.
- Using temporary files instead of stdin: rejected, doesn't test the actual stdin code path.

## R3: Test File Organization

**Decision**: Create 4 new test files aligned with spec user stories, plus extend existing common/mod.rs with stdin helpers.

**Rationale**: Follows existing naming convention (`e2e_*.rs`) and keeps test files focused.

| New File | Covers |
|----------|--------|
| `e2e_porcelain_coverage_tests.rs` | US1: clean, submodule, worktree, am/format-patch |
| `e2e_plumbing_coverage_tests.rs` | US2: mktag, mktree, commit-tree, pack-objects, index-pack, update-index, update-ref, check-attr, check-ignore, verify-pack |
| `e2e_bundle_archive_notes_tests.rs` | US3: bundle, archive, notes, replace |
| `e2e_maintenance_hooks_scale_tests.rs` | US4-7: prune, fast-import, hooks, large repos, config scoping |

## R4: Cross-Tool Compatibility Strategy

**Decision**: Use the dual-repo pattern established by existing tests: set up identical repos with C git, run the command under test with both gitr and C git, compare outputs.

**Rationale**: This is the proven pattern used by all 100+ existing e2e tests. For commands that modify repos (prune, clean, etc.), compare the resulting filesystem state as well.

**Special cases**:
- **Submodule**: Requires a separate "remote" repo as the submodule source. Use `file://` URLs.
- **Worktree**: Creates additional directories outside `.git`. Use nested tempdirs.
- **Bundle**: Binary format — verify cross-tool unbundling rather than byte comparison.
- **Archive**: Compare extracted contents rather than raw archive bytes (timestamps may differ).
- **Fast-import**: Use identical input streams, compare resulting repo state.

# Research: Git Behavioral Parity — Phase 2

**Feature**: 025-git-parity-phase2 | **Date**: 2026-02-09

## R1: `--version` Flag Implementation

**Decision**: Add `#[command(version)]` attribute to the `Cli` struct in `main.rs`. Clap 4 derives the version from `Cargo.toml` automatically. The output will be `gitr 0.1.1-alpha.10`. To match git's format (`git version X.Y.Z`), override with `#[command(version = format!("gitr version {}", env!("CARGO_PKG_VERSION")))]`.

**Rationale**: Clap 4's `#[command(version)]` attribute is the idiomatic way to handle `--version`. It automatically generates the `--version` flag and prints the version when invoked. The current `Cli` struct has no `version` attribute, so `gitr --version` fails with "unexpected argument".

**Alternatives considered**:
- Manual version subcommand: Rejected — `--version` is a global flag, not a subcommand, and clap handles this natively.

## R2: `switch -c` Short Flag

**Decision**: Add `#[arg(short = 'c')]` to the existing `create` field in `SwitchArgs`. The `--create` long form already exists. The `-c` short form is missing.

**Rationale**: Git's `switch -c <branch>` is the short form of `switch --create <branch>`. The current code has `#[arg(long, value_name = "new-branch")]` but no `short` attribute. Adding `short = 'c'` enables the common usage pattern.

**Alternatives considered**: None — straightforward clap attribute addition.

## R3: `merge --no-edit` and `revert --no-edit`

**Decision**: Add a `--no-edit` flag to `MergeArgs`. When set, skip editor invocation and use the auto-generated merge message. For `revert`, add `--no-edit` to `RevertArgs`.

**Rationale**: The merge command currently has `--message` but no `--no-edit`. When `--no-edit` is present, the merge should use the default merge message without opening an editor. The current code already generates a default message via `build_merge_message()` and never opens an editor, so the flag mostly needs to exist for compatibility. The revert command similarly lacks `--no-edit`.

**Alternatives considered**: None — these are flag-acceptance changes.

## R4: `config --unset` and `config --global`

**Decision**: Add `--unset` flag to `ConfigArgs` that removes a key from the config file. Add `--global` flag that reads/writes `~/.gitconfig` instead of the repo-local config.

**Rationale**: The `git-config` crate already supports multiple scopes (`ConfigScope::Local`, `ConfigScope::Global`, `ConfigScope::System`). The CLI just needs to wire through the `--global` scope for reads and writes, and implement an `unset()` method on the config. The `git-config` crate's `GitConfig::remove()` or equivalent needs to exist — if not, it must be added.

**Alternatives considered**: None.

## R5: `log --date=<format>` Integration

**Decision**: Add `--date` flag to `LogArgs` that accepts `iso`, `relative`, `short`, `default`, and custom `format:` strings. Pass the parsed `DateFormat` into `FormatOptions::date_format`.

**Rationale**: The `DateFormat` enum in `git-utils/src/date.rs` already has all needed variants (`Relative`, `Iso`, `Short`, `Default`). The `FormatOptions` in `git-revwalk` already has a `date_format` field. The only missing piece is the CLI flag that bridges them. Custom strftime via `--date=format:%Y-%m-%d` requires adding a `Custom(String)` variant to `DateFormat`.

**Alternatives considered**:
- Parse format strings in the CLI layer: Rejected — `DateFormat` is the correct abstraction boundary, and a `Custom` variant keeps the format logic centralized.

## R6: `log --merges` / `--no-merges` / `-- <path>` Filtering

**Decision**: Add `--merges` and `--no-merges` flags to `LogArgs`. Filter in the walk loop: `--merges` skips non-merge commits (`parents.len() < 2`); `--no-merges` skips merge commits (`parents.len() > 1`). For `-- <path>`, the existing pathspec parsing code already collects `_pathspecs` but never uses them — wire the pathspecs into the commit filtering by checking if each commit touches the given paths.

**Rationale**: The log command already parses `--` separators and collects pathspecs (log.rs line 136-148) but stores them as `_pathspecs` (unused). The merge/no-merge filter is a simple parent count check. Path filtering requires diffing each commit against its parent and checking if any changed file matches the pathspec.

**Alternatives considered**:
- Push path filtering into `RevWalk`: Rejected — the walker is OID-based and doesn't know about trees. Filtering at the CLI level is simpler.

## R7: `diff --word-diff`

**Decision**: Add `--word-diff` flag to `DiffArgs`. Implement word-level diffing as a new `DiffOutputFormat::WordDiff` variant. The default mode is `plain` which uses `[-removed-]{+added+}` markers.

**Rationale**: Word-diff requires splitting lines into word tokens, diffing at the token level, and formatting with word-level markers. This is a new output mode in `git-diff`. The algorithm can reuse the existing Myers/patience diff engine by treating words (instead of lines) as the diff units.

**Alternatives considered**:
- Implement in the CLI layer: Rejected — this is a formatting concern that belongs in `git-diff`.

## R8: `show -s` (Suppress Diff)

**Decision**: Add `#[arg(short = 's')]` as an alias for the existing `--no-patch` flag in `ShowArgs`.

**Rationale**: `show -s` is equivalent to `show --no-patch`. The `--no-patch` flag already exists in `ShowArgs` and works correctly. Adding `-s` as a short form is a one-line change.

**Alternatives considered**: None.

## R9: `branch --contains`

**Decision**: Add `--contains` flag to `BranchArgs` that accepts an optional commit (defaulting to HEAD). Filter the branch listing to only show branches whose tip is reachable from the specified commit, or more precisely, branches that contain the specified commit in their history.

**Rationale**: This requires walking each branch's history to check if the target commit is an ancestor. The `git_revwalk::is_ancestor()` function already exists and can be used directly.

**Alternatives considered**: None.

## R10: ISO 8601 Date Parsing in Environment Variables

**Decision**: The existing `GitDate::parse()` in `git-utils/src/date.rs` already handles ISO 8601 via `DateTime::parse_from_rfc3339()`. Verify that `GitDate::parse_raw()` (used by `get_signature()`) falls through to `GitDate::parse()` for non-raw formats. If not, ensure the fallback chain works: raw → rfc3339 → rfc2822 → short date.

**Rationale**: The `get_signature()` function in `commit.rs` calls `GitDate::parse_raw(&date_str)` for env var dates. If `parse_raw()` only handles `"timestamp +offset"` format, ISO 8601 strings will fail. The fix is to have `parse_raw()` fall back to `GitDate::parse()` on failure.

**Alternatives considered**: None — the parse chain should handle all formats.

## R11: Reflog Entry Recording

**Decision**: Currently, reflog entries are read (`read_reflog`) and the `append_reflog_entry` function exists in `git-ref/src/reflog.rs`. The issue is that operations (commit, checkout, reset, merge, rebase, cherry-pick, stash pop) don't call `append_reflog_entry`. Each operation needs to append a reflog entry after updating HEAD.

**Rationale**: The infrastructure exists but isn't wired in. Each HEAD-modifying operation needs to call `append_reflog_entry` with the appropriate message format (e.g., `commit: <subject>`, `checkout: moving from X to Y`, `reset: moving to <ref>`).

**Alternatives considered**: None — the plumbing exists, just needs wiring.

## R12: Output Format — Commit Diffstat

**Decision**: After creating a commit, compute a diffstat (tree diff between parent and new commit) and print it. Reuse `git_diff::tree::diff_trees()` with `DiffOutputFormat::Stat`.

**Rationale**: Git's commit output includes a diffstat summary like `1 file changed, 1 insertion(+)`. The current `print_summary()` in `commit.rs` only prints `[branch hash] subject` and a file count for root commits. The diffstat calculation requires reading the parent tree and the new tree, diffing them, and formatting the stat output.

**Alternatives considered**: None.

## R13: Graph Rendering — Extra Lines

**Decision**: Fix the `GraphDrawer::draw_commit()` method in `git-revwalk` to not insert extra `|` lines between commits on a single linear branch. The current implementation may be inserting inter-commit padding lines.

**Rationale**: On a linear history, `git log --graph --oneline` should show `* <hash> <msg>` for each commit without `|` lines between them. The graph drawer needs review to ensure it only emits `|` continuation lines when there are multiple active branches.

**Alternatives considered**: None — this is a bug fix in the graph drawer.

## R14: `gc` Silent Output

**Decision**: The current `gc` implementation prints progress messages (`Packing refs...`, `Expiring reflogs...`, etc.) even without `--verbose`. Change the default behavior to be silent (matching git), moving progress output behind a `!args.quiet` check... which it already has, but the `--quiet` flag defaults to false. The fix is to suppress output by default (don't print progress messages unless explicitly asked for, or only print them in specific conditions matching git's behavior).

**Rationale**: Git's `gc` is silent by default. The current gitr `gc` prints progress on stderr. The simplest fix is to default `quiet` to true or remove the progress messages entirely.

**Alternatives considered**: None.

## R15: `shortlog` Stdin Reading

**Decision**: When `shortlog` is invoked in a non-tty context (piped input), it should read commit data from stdin instead of walking HEAD. Check `atty::is(Stream::Stdin)` or equivalent to detect piped input.

**Rationale**: Git's `shortlog` reads from stdin when piped (e.g., `git log | git shortlog`). The current implementation always walks from HEAD/revisions, ignoring stdin.

**Alternatives considered**:
- Add `atty` dependency: Could use `std::io::stdin().is_terminal()` (stabilized in Rust 1.70) instead.

## R16: `status` Untracked File Sorting and Directory Collapsing

**Decision**: Sort untracked files alphabetically. Collapse directories: if all files in a directory are untracked, show `subdir/` instead of individual files.

**Rationale**: The current `find_untracked_recursive()` in `status.rs` lists individual files and relies on filesystem ordering (not alphabetical). Directory collapsing requires checking if any tracked files exist within a directory — if none, show the directory name with trailing `/`.

**Alternatives considered**: None.

## R17: `status --short` Rename Detection

**Decision**: Add rename detection to the short status output. When a file is deleted from the index and a similar file is added, detect the rename and show `R  old -> new`.

**Rationale**: This requires enabling rename detection in the diff options used by the short status. The `git-diff` crate already has `detect_renames` in `DiffOptions` and produces `FileStatus::Renamed` entries.

**Alternatives considered**: None.

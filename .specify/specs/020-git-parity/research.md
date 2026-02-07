# Research: Git Command Parity

**Feature**: 020-git-parity | **Date**: 2026-02-07

## R1: Default Date Format Mismatch

**Decision**: Add a new `DateFormat::Default` variant that formats as `"%a %b %e %H:%M:%S %Y %z"` using the **commit's timezone offset** (not local time), and make it the default in `FormatOptions`.

**Rationale**: C git's default date format is `"Thu Feb 13 23:31:30 2009 +0000"`. Gitr currently defaults to `DateFormat::Iso` which produces `"2009-02-13 23:31:30 +0000"`. The existing `DateFormat::Local` variant converts to local system time AND omits the timezone offset, which is wrong for two reasons: (1) it produces different output on different machines, and (2) C git's default uses the commit's stored timezone, not the local one. Adding a `DateFormat::Default` variant that matches C git's behavior is the correct fix.

**Alternatives considered**:
- Reuse `DateFormat::Local` with a format string fix: Rejected because `Local` semantically means "convert to local time" which is a different C git date mode (`--date=local`).
- Reuse `DateFormat::Rfc2822`: Rejected because RFC 2822 uses `"Wed, 15 Jan 2025 12:00:00 +0000"` (different order and comma separator).

## R2: Diff Hunk Content Output

**Decision**: No changes needed to the diff engine. The unified diff formatter in `git-diff/src/format/unified.rs` correctly emits hunk headers (`@@`), context lines, additions (`+`), and deletions (`-`).

**Rationale**: Code review confirms `format_hunk()` (lines 135-174) properly emits all hunk content. If tests show hunk content missing, the issue is likely in the diff CLI command's blob-reading path (not finding the correct blob content to diff), not in the formatter. The investigation should focus on how the diff command fetches object content when the ODB includes packfiles.

**Alternatives considered**: None — the formatter is correct.

## R3: Packfile Reading — OFS_DELTA and REF_DELTA Resolution

**Decision**: The packfile reader in `git-pack/src/pack.rs` correctly handles OFS_DELTA (intra-pack) resolution. REF_DELTA resolution needs a fallback path: when the base object referenced by a REF_DELTA is not in the same packfile, the reader must query the full object database (loose objects + other packs).

**Rationale**: C git's `gc` can produce REF_DELTA entries where the base object is in a different pack or in loose storage. The current implementation only looks within the same packfile for REF_DELTA bases (line ~133 in pack.rs). Delta chain resolution for OFS_DELTA is correct (uses byte offsets within the same pack). The pack index v2 reading (fanout, binary search, 64-bit offsets) is complete and correct.

**Alternatives considered**:
- Resolve all objects through ODB and never read packs directly: Rejected because OFS_DELTA needs pack-level access for offset-based lookups, and this would be a major refactor.
- Pre-resolve all REF_DELTA entries on pack load: Rejected because it's wasteful for packs with many REF_DELTA entries; lazy resolution is better.

## R4: Merge — Fast-Forward and Three-Way

**Decision**: The merge implementation is fundamentally correct. Fast-forward properly advances refs via `update_head_to()`. Three-way merge via ORT strategy correctly includes files from both branches by starting from the base tree and applying all diffs from both sides. Merge commits are created with two parents.

**Rationale**: Code review of `merge.rs` (lines 119-147 for FF, lines 158-219 for three-way) and `strategy/ort.rs` confirms correct behavior. The ORT strategy builds the result tree by starting with base tree entries and applying all changes detected in both `base→ours` and `base→theirs` diffs. If interop tests reveal merge failures, the root cause is likely in tree write-back, index update, or working tree checkout — not in the merge logic itself.

**Alternatives considered**: None — the implementation is sound.

## R5: Merge — Conflict Markers and Exit Codes

**Decision**: Verify conflict marker format matches C git exactly. C git produces markers in this format:
```
<<<<<<< HEAD
our content
=======
their content
>>>>>>> branch-name
```
The conflict recording in the index (stages 1/2/3) appears correct. Exit code for conflicts should be 1.

**Rationale**: The `conflict.rs` module correctly manages index stages. The content merge in `content.rs` generates markers but needs verification that the exact format (marker length, labels) matches C git. The merge command should exit with code 1 on conflicts and 0 on clean merges.

**Alternatives considered**: None.

## R6: Output Format — `log --format=` Newlines

**Decision**: The log command's custom format output must append a newline after each entry (record separator), matching C git's behavior. The current implementation uses `saw_separator` logic (line 124 of log.rs) which may not correctly handle all cases.

**Rationale**: C git emits a newline after each `--format=` entry. The spec says "`log --format=%s` omits newlines between entries." The fix is to ensure the separator logic in the log output loop correctly emits `\n` between entries.

**Alternatives considered**: None.

## R7: Output Format — Blame OID Prefix and Date+Time

**Decision**: The blame command needs two fixes: (1) the date format must include time (not just date), and (2) verify OID prefix length matches C git's dynamic abbreviation (typically 7-8 chars).

**Rationale**: Current blame uses `DateFormat::Iso` then truncates to 10 chars (YYYY-MM-DD), omitting time. C git's default blame format shows date+time. The OID prefix at 7 chars is likely correct for small repos but may differ from C git's automatic abbreviation in larger repos.

**Alternatives considered**:
- Use auto-abbreviation based on ODB size: Deferred — 7 chars is correct for the test scenarios in the spec.

## R8: Output Format — Detached HEAD Status

**Decision**: The status command must include the short commit OID when in detached HEAD state. Change the output from `"HEAD detached"` to `"HEAD detached at <short-oid>"`.

**Rationale**: C git shows `"HEAD detached at abc1234"`. The current gitr code (status.rs line 113) outputs `"HEAD detached"` without the OID, which breaks tooling that parses status output.

**Alternatives considered**: None — straightforward fix.

## R9: Output Format — Empty Repo `log` Exit Code

**Decision**: The log command must return exit code 128 when run on a repository with no commits, matching C git's `"fatal: your current branch 'main' does not have any commits yet"`.

**Rationale**: Currently, `walker.push_head()?` (log.rs line 143) returns an error which may not produce the correct exit code. The fix is to detect the unborn branch condition and exit with code 128 with the appropriate error message.

**Alternatives considered**: None.

## R10: Output Format — `ls-files` Unicode Path Escaping

**Decision**: Implement C git's default path quoting in `ls-files`: non-ASCII bytes are octal-escaped within double quotes. E.g., `café.txt` becomes `"caf\303\251.txt"`.

**Rationale**: C git's default `core.quotePath=true` behavior encodes non-ASCII bytes as octal escape sequences. The current implementation outputs raw bytes. The escaping should only apply when not using `-z` flag (NUL-terminated output mode).

**Alternatives considered**:
- Use the `bstr` crate's display formatting: Rejected because bstr doesn't implement C git's specific octal quoting.
- Implement at the `git-utils` level as a shared utility: Accepted — this quoting logic will be useful for other commands (status, diff --name-only, etc.).

## R11: Remote Operations — file:// Protocol

**Decision**: The existing clone/push/fetch/pull implementations use the transport layer which supports file:// URLs via local transport. The key issues to verify are: (1) correct URL parsing for `file:///path/to/repo`, (2) proper pack exchange over local protocol, and (3) correct remote config setup after clone.

**Rationale**: The transport layer (`git-transport/src/local.rs`) handles local repos. The protocol layer handles pack negotiation. These are already implemented. Issues are likely in edge cases: stripping `file://` scheme prefix, handling bare vs non-bare repos, and writing correct `remote.origin.url` and `remote.origin.fetch` config entries.

**Alternatives considered**:
- Direct file copy for local clones: Rejected in favor of using the transport layer for consistency.

## R12: Stash — Multiple Entries and --include-untracked

**Decision**: Three fixes needed: (1) stash must use reflog for multiple entries instead of overwriting `refs/stash`, (2) pop must restore actual working tree state (not just index state), (3) `--include-untracked` must capture and restore untracked files.

**Rationale**: C git's stash implementation stores entries as a reflog of `refs/stash`. Each stash push creates a new reflog entry. The current gitr implementation overwrites the single `refs/stash` reference. For `--include-untracked`, C git creates a third parent commit containing untracked files.

**Alternatives considered**: None — must match C git's internal structure.

## R13: Plumbing — for-each-ref HEAD Exclusion

**Decision**: The `for-each-ref` command must exclude HEAD from its output. Currently HEAD is included (likely because the ref iteration includes symbolic refs).

**Rationale**: C git's `for-each-ref` only iterates over refs under `refs/` — it does not include HEAD. The fix is to filter out HEAD from the iteration results, or to restrict the iteration to the `refs/` namespace.

**Alternatives considered**: None.

## R14: Plumbing — rev-parse ^{type} Peeling Syntax

**Decision**: Add support for `^{tree}`, `^{commit}`, `^{blob}`, `^{tag}`, and `^{}` (recursive peel) peeling syntax in the revision parser (`git-revwalk/src/range.rs`).

**Rationale**: The current parser only supports `~N` (ancestor) and `^N` (parent) suffixes. The `^{type}` syntax is essential for scripting (e.g., `git rev-parse HEAD^{tree}` returns the tree OID of HEAD's commit). The fix requires: (1) parsing `^{...}` in `split_revision_suffix()`, and (2) implementing object type peeling that reads the object and follows references until the target type is found.

**Alternatives considered**: None — standard git syntax that must be supported.

## R15: Rebase — Commit Replay OID Matching

**Decision**: The rebase commit replay preserves author dates (correct) and sets new committer dates (correct). OID mismatches are likely caused by environment variable handling — specifically, GIT_COMMITTER_DATE should be preserved during rebase to produce deterministic results in test environments. The fix is to ensure the rebase command respects GIT_COMMITTER_DATE when set.

**Rationale**: C git's rebase preserves both author info and respects GIT_COMMITTER_DATE. In test environments where GIT_COMMITTER_DATE is pinned, both implementations should produce identical OIDs. If gitr's rebase uses `GitDate::now()` ignoring GIT_COMMITTER_DATE, OIDs will differ. The `build_committer()` function needs to check for GIT_COMMITTER_DATE before falling back to current time.

**Alternatives considered**:
- Always use `--committer-date-is-author-date`: Rejected because it changes semantics for non-test use.

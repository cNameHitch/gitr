# Research: Git Behavioral Parity Polish

**Feature**: 024-git-parity-polish | **Date**: 2026-02-09

## R1: Pathspec Disambiguation Strategy

**Decision**: Implement git's "try as revision first, fall back to pathspec" algorithm in `diff` and `reset` argument parsing. For each non-flag argument before the `--` separator, call `resolve_revision()`. If resolution fails and the argument exists as a file/directory on disk, treat it as a pathspec. If it exists as neither, report an ambiguous argument error matching git's `fatal: ambiguous argument 'X': unknown revision or path not in the working tree.`

**Rationale**: C git implements this in `setup_revisions()` via `verify_filename()`. The current gitr implementation only supports explicit `--` separation in diff (`parse_diff_args()` at diff.rs) and positional parsing in reset. The disambiguation algorithm is well-defined: revision resolution is attempted first because revision names are more constrained than paths, so a failed resolution is a strong signal that the argument is a pathspec.

**Alternatives considered**:
- Always require `--` separator: Rejected because it breaks compatibility with common usage patterns (`git diff file.txt`, `git reset file.txt`).
- Check filesystem first: Rejected because git prioritizes revision resolution (a branch named `file.txt` takes precedence over a file named `file.txt` without `--`).

## R2: Hunk Header Off-by-One in New Files

**Decision**: Fix the start-line calculation in the unified diff formatter. When diffing a new file (source is `/dev/null`), the source range must be `0,0` producing `@@ -0,0 +1,N @@`. The current code produces `@@ -1,0 +1,N @@` because the start line defaults to 1.

**Rationale**: C git's `xdl_emit_hunk_hdr()` outputs `0,0` for the old side when the file is entirely new. The fix is in the hunk header formatting, not in the diff algorithm — the diff correctly identifies all lines as additions, but the header serialization starts at line 1 instead of 0.

**Alternatives considered**: None — straightforward off-by-one fix.

## R3: Check-Ignore Directory Pattern Matching

**Decision**: Fix the `WildmatchPattern` matching for directory patterns ending in `/`. A pattern like `build/` must match: (1) the directory itself (`build`), (2) any path under the directory (`build/output.o`). The current implementation only matches exact paths, not prefix matches.

**Rationale**: C git's gitignore matching treats trailing-slash patterns as "match if the target is a directory OR any path within it." The fix requires the pattern matcher to strip the trailing `/` and check both exact match and prefix match (`path.starts_with("build/")`).

**Alternatives considered**:
- Expand patterns at load time: Rejected because the number of paths is unbounded.
- Check directory status via stat(): Considered but insufficient — `check-ignore` receives paths as strings without filesystem access being mandatory.

## R4: Log Date Filtering (--since/--until)

**Decision**: Filter commits by author date (not committer date) in the revision walker. Add optional `since` and `until` timestamps to `WalkOptions`. During traversal, skip commits whose `author.date.timestamp` falls outside the range. Commits with author dates before `--until` but with parents that might be in range must still be walked (only output is filtered, not traversal).

**Rationale**: C git's `--since`/`--until` filter by author date by default (matching `--date-order` semantics). The current `log.rs` accepts the flags but doesn't use them for filtering. The filter must be applied post-traversal (not as a walk termination condition) because git histories can have non-monotonic author dates.

**Alternatives considered**:
- Filter by committer date: Rejected — git defaults to author date for `--since`/`--until`.
- Terminate walk at boundary: Rejected because author dates are not monotonic (rebased commits may have old author dates).

## R5: Init Directory Creation

**Decision**: Use `std::fs::create_dir_all()` for the target path before repository initialization. This matches `git init /path/to/new/nested/repo` which creates the entire hierarchy.

**Rationale**: The current `init.rs` fails if the parent directory doesn't exist. C git calls `mkdir_p()` on the target path. The fix is a single `create_dir_all()` call before `Repository::init_opts()`.

**Alternatives considered**: None — trivial fix.

## R6: Show Command — Stat, Merge Header, Format Modes

**Decision**: Multiple fixes in `show.rs`:
1. When `--stat` is present, do NOT emit the full unified diff after the stat summary. Currently `show_commit()` emits both.
2. For merge commits, emit `Merge: <short-parent1> <short-parent2>` header between `commit` and `Author:` lines.
3. For `--format=oneline`, use `oid.to_hex()` (full 40-char) instead of abbreviated hash.
4. For `--format=raw`, indent each message line with 4 spaces (matching `cat-file commit` output format).

**Rationale**: Each of these matches specific C git output formatting. The `--stat` fix is a conditional gate in the diff output path. The merge header is a new `writeln!` call checking `commit.parents.len() > 1`. The oneline hash fix changes which hash method is called. The raw indent is a per-line transformation on the message body.

**Alternatives considered**: None — all straightforward formatting fixes.

## R7: Format String Completeness

**Decision**: Audit the format placeholder list in `pretty.rs` against git's `pretty-formats` documentation. Add missing placeholders: `%d` (ref names, like `(HEAD -> main, tag: v1.0)`), `%D` (ref names without wrapping parens), and verify all existing placeholders produce correct output. The `format_commit()` function already supports the core set (`%H`, `%h`, `%T`, `%t`, `%P`, `%p`, `%an`, `%ae`, `%ad`, `%cn`, `%ce`, `%cd`, `%s`, `%b`, `%B`, `%n`, `%%`).

**Rationale**: C git's format strings are heavily used in scripts and aliases. Missing placeholders cause empty output or literal `%X` in the output stream. The `%d` and `%D` placeholders require ref decoration information, which needs a ref-to-commit mapping passed to the formatter.

**Alternatives considered**:
- Implement all 60+ git format placeholders: Deferred — focus on the commonly used subset. Exotic placeholders (`%G?` for GPG status, `%gD` for reflog selector) are out of scope.

## R8: -N Shorthand for Log

**Decision**: Use clap's `preprocess_args()` function (already in `main.rs`) to transform `-N` into `--max-count=N` before clap parsing. This avoids modifying clap's derive-based parser which doesn't support arbitrary `-<digit>` flags natively.

**Rationale**: C git accepts `-1`, `-5`, `-100` as shorthand for `--max-count=N`. The current `preprocess_args()` already handles some argument transformations. Adding a regex or simple pattern match for `-\d+` → `--max-count=N` is the cleanest approach.

**Alternatives considered**:
- Add a clap `value_parser` on an optional positional: Rejected because `-3` is not a valid clap argument format.
- Parse manually after clap: Rejected because clap would reject `-3` as an unknown flag before we get a chance.

## R9: Decorate Flag for Log

**Decision**: The `--decorate` flag for `log` requires building a map of commit OID → ref names at the start of the log command. Iterate all refs (branches, tags, HEAD) and build a `HashMap<ObjectId, Vec<String>>`. When formatting each commit, look up decorations and append `(HEAD -> main, tag: v1.0)` after the hash.

**Rationale**: C git's decoration support is a core feature used in `--oneline --decorate` output. The ref map construction is O(number of refs) which is typically small. The decoration format must match git's exact spacing and ordering: HEAD first, then local branches, then remote-tracking branches, then tags.

**Alternatives considered**:
- Compute decorations lazily per-commit: Rejected because ref iteration is cheaper as a single pass.

## R10: Branch -v Verbose Output

**Decision**: Add `-v`/`--verbose` flag to `BranchArgs`. When set, for each branch, resolve the tip commit, read its commit object, and display `<branch-name> <short-hash> <subject-line>`. The current branch gets a `*` prefix.

**Rationale**: C git's `branch -v` is one of the most commonly used branch listing modes. The implementation reads one commit per branch which is acceptable for typical branch counts (< 100).

**Alternatives considered**: None — standard implementation.

## R11: Exit Code 128 for Invalid CLI Arguments

**Decision**: Override clap's error handler in `main.rs`. Clap exits with code 2 for usage errors. Git exits with 128. Use `clap::Command::error()` override or catch the clap error in `main()` and re-exit with 128.

**Rationale**: Scripts using `set -e` or checking `$?` rely on specific exit codes. Clap's default exit code (2) differs from git's convention (128 for fatal errors including usage errors). The override is a well-documented clap pattern.

**Alternatives considered**:
- Use clap's `AppSettings::WaitOnError`: Irrelevant — doesn't change exit codes.
- Parse args manually: Rejected — defeats the purpose of using clap.

## R12: System Config File Discovery

**Decision**: Add platform-conditional system config loading in `git-config`:
- macOS: Check `/Library/Developer/CommandLineTools/usr/share/git-core/gitconfig` and `$(git --exec-path)/../etc/gitconfig`
- Linux: Check `/etc/gitconfig`
- All platforms: Check `$GIT_CONFIG_SYSTEM` env var override

Load system config first in the cascade (system → global → local), matching git's precedence.

**Rationale**: C git loads system-level configuration which can affect behavior (e.g., `credential.helper`, `http.sslCAInfo`). The current gitr implementation skips system config entirely, which causes subtle differences when system-level settings are present.

**Alternatives considered**:
- Shell out to `git config --system --list`: Rejected — depends on C git being installed.
- Hard-code only `/etc/gitconfig`: Rejected — macOS Xcode CLT installs git config in a non-standard location.

## R13: macOS Init Config and Sample Hooks

**Decision**: During `Repository::init_opts()`:
1. On macOS (`#[cfg(target_os = "macos")]`), set `core.ignorecase = true` and `core.precomposeunicode = true` in the local config.
2. Copy standard sample hook files to `.git/hooks/`. The sample hooks are static content — embed them as `include_str!()` constants or as a template directory.

**Rationale**: C git's `init.c` calls `create_default_files()` which checks the filesystem case-sensitivity and sets platform-appropriate defaults. On macOS (HFS+ is case-insensitive by default), `core.ignorecase = true` is standard. The sample hooks (`pre-commit.sample`, `commit-msg.sample`, etc.) are part of git's template directory and are expected by many developer tools.

**Alternatives considered**:
- Actually probe filesystem case-sensitivity: Considered — would be more correct but adds complexity. For now, assume macOS is case-insensitive (true for 99%+ of installations).
- Skip sample hooks: Rejected — spec FR-042 requires them.

## R14: Shortlog Commit Ordering

**Decision**: Within each author group in `shortlog`, reverse the commit list so oldest commits appear first. The current implementation likely appends commits in walk order (newest-first from RevWalk), then displays in that order.

**Rationale**: C git's `shortlog` sorts commits within each author chronologically oldest-first. The fix is to either reverse the per-author commit vector before display, or insert at the front during collection.

**Alternatives considered**:
- Sort by timestamp: More correct but `reverse()` produces the same result since RevWalk yields newest-first.

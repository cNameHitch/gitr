# Feature Specification: Full Git CLI Parity (Phase 3)

**Feature Branch**: `026-git-parity-phase3`
**Created**: 2026-02-09
**Status**: Draft
**Input**: User description: "Full git CLI parity — close all remaining gaps between gitr and git, covering top-level CLI fixes, color/pager support, missing flags across 30+ commands, missing commands, and behavioral/data gaps."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Color and Pager Output (Priority: P1)

A developer runs `gitr diff`, `gitr log`, or `gitr status` in their terminal and sees colored output identical to git: red for removals, green for additions, yellow for commit hashes, cyan for branch names. When output exceeds the terminal height, it is automatically piped through a pager (e.g., `less`) so the developer can scroll through results exactly as they would with git.

**Why this priority**: Color and pager are the most visible UI differences. Every interactive session exposes this gap. Without them, gitr feels fundamentally different from git regardless of feature completeness.

**Independent Test**: Run `gitr log --oneline --all` in a repo with 100+ commits and verify colored output appears and a pager is invoked; run `gitr diff` on a modified file and verify red/green coloring matches git.

**Acceptance Scenarios**:

1. **Given** a terminal that supports ANSI colors, **When** a user runs `gitr diff` on a file with changes, **Then** removed lines appear in red and added lines appear in green, matching git's default color scheme.
2. **Given** a terminal with `$PAGER` set to `less`, **When** a user runs `gitr log` with output exceeding terminal height, **Then** output is piped through the pager automatically.
3. **Given** a user runs `gitr diff --color=never`, **When** output is produced, **Then** no ANSI escape codes appear in the output.
4. **Given** `color.diff=false` is set in config, **When** `gitr diff` is run without `--color` flag, **Then** output is uncolored.
5. **Given** output is piped to a file (not a terminal), **When** `gitr log` is run with default `--color=auto`, **Then** no ANSI codes appear and no pager is invoked.

---

### User Story 2 - Top-Level CLI Parity (Priority: P1)

A developer who is accustomed to git's CLI invokes gitr with the same global flags and expects identical behavior: `gitr --work-tree=/path`, `gitr -P log` (no pager), `gitr --version`, and `gitr --help` all produce output matching git's format and conventions.

**Why this priority**: The top-level CLI is the entry point for every command. Global flag conflicts (e.g., `-C`/`-c` stealing subcommand short flags) break fundamental workflows like `gitr switch -c new-branch`.

**Independent Test**: Run `gitr --version` and verify format matches `gitr version X.Y.Z`; run `gitr --help` and verify categorized output; run `gitr switch -c mybranch` and verify it creates a branch (not interpreted as config flag).

**Acceptance Scenarios**:

1. **Given** a user runs `gitr --version`, **Then** output is `gitr version X.Y.Z` (not `gitr X.Y.Z`).
2. **Given** a user runs `gitr --help`, **Then** commands are grouped by category (start a working area, examine history, grow/mark/tweak, collaborate) matching git's layout.
3. **Given** a user runs `gitr --work-tree=/other/path status`, **Then** the status is computed against the specified working tree.
4. **Given** a user runs `gitr -P log`, **Then** output is not piped through a pager.
5. **Given** a user runs `gitr switch -c new-branch`, **Then** a new branch is created (the `-c` is interpreted as the switch subcommand's create flag, not the global config override).

---

### User Story 3 - Missing Command Flags for Core Workflows (Priority: P1)

A developer uses gitr for their daily workflow and expects every flag they use with git to work identically: `gitr commit -s`, `gitr diff -S "search"`, `gitr log --follow`, `gitr merge --continue`, `gitr stash --keep-index`, `gitr config --get-all`, etc. Every flag listed in `git <cmd> -h` is accepted and behaves correctly.

**Why this priority**: Missing flags cause hard failures or silent behavioral differences that erode trust. This is the bulk of the parity work and directly determines whether gitr can be a drop-in replacement.

**Independent Test**: For each command, run `git <cmd> -h` and `gitr <cmd> -h` side by side and verify identical flag sets; run common workflows (commit with signoff, interactive rebase, stash with keep-index) and verify identical behavior.

**Acceptance Scenarios**:

1. **Given** a user runs `gitr commit -s -m "msg"`, **Then** the commit message includes a `Signed-off-by:` trailer matching git's format.
2. **Given** a user runs `gitr merge --continue` after resolving conflicts, **Then** the merge completes (not rejected due to unrecognized flag).
3. **Given** a user runs `gitr diff -S "functionName"`, **Then** only diffs that add or remove "functionName" are shown.
4. **Given** a user runs `gitr log --follow -- file.txt` after a rename, **Then** history continues across the rename boundary.
5. **Given** a user runs `gitr stash push --keep-index`, **Then** staged changes remain in the index and only unstaged changes are stashed.
6. **Given** a user runs `gitr rebase -i HEAD~3`, **Then** an interactive editor opens with the last 3 commits listed for reordering/squashing/editing.
7. **Given** a user runs `gitr config --get-all user.name`, **Then** all values for the key across all scopes are listed.

---

### User Story 4 - Missing Plumbing and Porcelain Commands (Priority: P2)

A developer or CI script invokes plumbing commands like `gitr merge-base`, `gitr diff-tree`, `gitr ls-remote`, `gitr apply`, or `gitr name-rev` and expects them to work identically to git. Scripts that depend on these commands should work without modification.

**Why this priority**: Missing commands cause immediate hard failures for any script or tool that calls them. These are less frequent than flag gaps but are absolute blockers when encountered.

**Independent Test**: Run `gitr merge-base HEAD HEAD~1` and verify it outputs the common ancestor; run `gitr ls-remote origin` and verify it lists remote refs; run `gitr apply < patch.diff` and verify it applies the patch.

**Acceptance Scenarios**:

1. **Given** a repo with branches A and B, **When** `gitr merge-base A B` is run, **Then** the output matches `git merge-base A B`.
2. **Given** a patch file, **When** `gitr apply patch.diff` is run, **Then** the working tree is modified identically to `git apply patch.diff`.
3. **Given** a remote repository, **When** `gitr ls-remote origin` is run, **Then** all remote refs and their OIDs are listed.
4. **Given** a commit, **When** `gitr diff-tree -r HEAD` is run, **Then** the tree diff output matches git's format.
5. **Given** a commit hash, **When** `gitr name-rev <hash>` is run, **Then** the output shows the symbolic name.

---

### User Story 5 - Behavioral and Data Format Parity (Priority: P2)

A developer relies on advanced git features: `.gitattributes` for CRLF handling, `[include]` directives in config, `.mailmap` for author normalization, diff3 conflict markers, the full hook lifecycle (pre-push, post-checkout, prepare-commit-msg, etc.), and merge strategies (recursive, ort, octopus, ours, subtree). All of these work identically in gitr.

**Why this priority**: These are deeper behavioral differences that affect correctness in specific (but real) workflows. They don't block basic usage but prevent gitr from being a true drop-in replacement in complex projects.

**Independent Test**: Create a `.gitattributes` file with `*.txt text eol=crlf` and verify gitr normalizes line endings; add a `[include]` directive in `.gitconfig` and verify included config is loaded; run a merge with `--strategy=ours` and verify only the current branch's tree is kept.

**Acceptance Scenarios**:

1. **Given** a `.gitattributes` file with `*.txt text eol=crlf`, **When** `gitr add file.txt` is run on a Unix system, **Then** the blob stored has LF line endings (normalized) and checkout produces CRLF.
2. **Given** a `.gitconfig` with `[include] path = ~/.gitconfig.local`, **When** `gitr config --list` is run, **Then** values from the included file appear in the output.
3. **Given** a `.mailmap` file mapping old email to new email, **When** `gitr log --use-mailmap` is run, **Then** the mapped author name/email appears.
4. **Given** a merge conflict, **When** `merge.conflictStyle=diff3` is configured, **Then** conflict markers include the `|||||||` base section.
5. **Given** a `pre-push` hook, **When** `gitr push` is run, **Then** the hook executes before push proceeds and can abort the push.
6. **Given** a merge with `--strategy=ours`, **Then** the resulting tree is identical to the current branch's tree (theirs is discarded).

---

### User Story 6 - Interactive Modes (Priority: P3)

A developer uses interactive features like `gitr add -p`, `gitr rebase -i`, `gitr stash -p`, `gitr reset -p`, and `gitr checkout -p` to selectively stage hunks, reorder commits, or partially stash changes.

**Why this priority**: Interactive modes are complex to implement (require TUI-like hunk selection) and are used less frequently, but they represent a significant UX gap for power users.

**Independent Test**: Run `gitr add -p` on a file with multiple hunks and verify the interactive prompt allows staging individual hunks; run `gitr rebase -i HEAD~3` and verify the editor opens with pick/squash/edit options.

**Acceptance Scenarios**:

1. **Given** a file with 3 modified hunks, **When** `gitr add -p` is run, **Then** each hunk is presented individually with y/n/s/q options matching git's interactive staging.
2. **Given** 3 recent commits, **When** `gitr rebase -i HEAD~3` is run, **Then** an editor opens with `pick`, `reword`, `edit`, `squash`, `fixup`, `drop` options per commit.
3. **Given** staged and unstaged changes, **When** `gitr reset -p` is run, **Then** each staged hunk can be individually unstaged.

---

### Edge Cases

- What happens when `--color=auto` is used but `$TERM` is set to `dumb`? (No color output, matching git)
- What happens when `$PAGER` is empty string vs unset? (Empty string = no pager; unset = use default `less`)
- What happens when a subcommand short flag conflicts with a global short flag? (Subcommand flag takes precedence within subcommand context)
- What happens when `gitr merge --continue` is run with no merge in progress? (Error message matching git: "There is no merge in progress")
- What happens when `gitr stash drop stash@{5}` is run but only 3 entries exist? (Error matching git: "stash@{5} is not a valid reference")
- What happens when `gitr config --include` encounters a circular include? (Error with cycle detection, matching git's behavior)
- What happens when `gitr diff --no-index` is run outside a git repository? (Succeeds — this is one of the few commands that works without a repo)
- What happens when `gitr apply` receives a corrupt patch? (Error with descriptive message matching git)
- What happens when GPG signing is requested but no GPG key is configured? (Error matching git: "gpg failed to sign the data")
- What happens when interactive rebase encounters a conflict during `edit`? (Pauses rebase, user resolves, runs `--continue`)

## Requirements *(mandatory)*

### Functional Requirements

**Top-Level CLI**

- **FR-001**: System MUST output `gitr version X.Y.Z` when `--version` is passed (matching git's `git version X.Y.Z` format)
- **FR-002**: System MUST display help output grouped by category (start a working area, examine history, grow/mark/tweak, collaborate) when `--help` is passed
- **FR-003**: System MUST support global flags: `--work-tree`, `--bare`, `-p`/`--paginate`, `-P`/`--no-pager`, `--no-replace-objects`, `--config-env`, `--exec-path`, `--namespace`
- **FR-004**: System MUST resolve global `-C`/`-c` flag conflicts so that subcommand-specific short flags (e.g., `switch -c`, `diff -C`, `branch -c/-C`, `grep -C`) take precedence within their subcommand context

**Color Output**

- **FR-010**: System MUST support `--color=auto|always|never` on all commands that produce colored output (diff, status, log, branch, grep, show, blame, shortlog)
- **FR-011**: In `auto` mode, system MUST output ANSI color codes only when stdout is a terminal (not when piped or redirected)
- **FR-012**: System MUST respect `color.*` config options (e.g., `color.diff`, `color.status`, `color.branch`, `color.ui`)
- **FR-013**: System MUST use git's default color scheme: red for deletions, green for additions, yellow for commit hashes, cyan for branch/tag names, bold for headers
- **FR-014**: System MUST support `--color-words` for word-level colored diff output

**Pager Integration**

- **FR-020**: System MUST auto-invoke pager for commands: log, diff, show, blame, shortlog, grep, branch, tag, help (when output is a terminal)
- **FR-021**: Pager resolution MUST follow git's cascade: `$GIT_PAGER` > `core.pager` config > `$PAGER` > `less`
- **FR-022**: System MUST support `-p`/`--paginate` (force pager) and `-P`/`--no-pager` (suppress pager) global flags
- **FR-023**: System MUST pass `LESS=FRX` and `LV=-c` environment variables to pager when not already set (matching git's defaults)

**commit Command Flags**

- **FR-030**: System MUST support `-F`/`--file` to read commit message from a file
- **FR-031**: System MUST support `-C`/`--reuse-message` and `-c`/`--reedit-message` to reuse a previous commit's message
- **FR-032**: System MUST support `--fixup` and `--squash` to create fixup/squash commits
- **FR-033**: System MUST support `-s`/`--signoff` to append `Signed-off-by` trailer
- **FR-034**: System MUST support `--trailer` to append arbitrary trailers
- **FR-035**: System MUST support `-n`/`--no-verify` to skip pre-commit and commit-msg hooks
- **FR-036**: System MUST support `--dry-run` to show what would be committed without committing
- **FR-037**: System MUST support `-v`/`--verbose` to show unified diff in the commit message editor
- **FR-038**: System MUST support `--date` to override author date
- **FR-039**: System MUST support `--reset-author` to reset author to committer
- **FR-040**: System MUST display `create mode` and `delete mode` lines in commit summary output

**status Command Flags**

- **FR-050**: System MUST support `-v`/`--verbose` to show diff of staged changes
- **FR-051**: System MUST support `-z` for NUL-terminated output
- **FR-052**: System MUST support `-u`/`--untracked-files` with modes: `no`, `normal`, `all`
- **FR-053**: System MUST support `--ignored` to show ignored files
- **FR-054**: System MUST support `--column`/`--no-column` for columnar untracked file listing
- **FR-055**: System MUST support `--ahead-behind`/`--no-ahead-behind` for upstream comparison

**diff Command Flags**

- **FR-060**: System MUST support `--full-index` for full SHA in diff headers
- **FR-061**: System MUST support `-R` for reverse diff
- **FR-062**: System MUST support `-S`/`--pickaxe` to search for string changes and `-G` for regex matching in diff content
- **FR-063**: System MUST support `--diff-filter=ACDMRTUXB*` to filter by change type
- **FR-064**: System MUST support `--patience`, `--histogram`, `--minimal` diff algorithm selection
- **FR-065**: System MUST support `--no-index` to diff two files outside a repository
- **FR-066**: System MUST support `--check` for whitespace error detection
- **FR-067**: System MUST support `--src-prefix`/`--dst-prefix`/`--no-prefix` for custom path prefixes
- **FR-068**: System MUST fix `diff --stat` with a single commit argument to show working-tree vs commit diff (currently produces empty output)
- **FR-069**: System MUST support `-z` for NUL-terminated output

**log Command Flags**

- **FR-070**: System MUST support `-L` for line-range based log
- **FR-071**: System MUST support `--follow` to track renames across history
- **FR-072**: System MUST support `--diff-filter` to filter commits by change type
- **FR-073**: System MUST support `--abbrev-commit` and `--no-decorate`
- **FR-074**: System MUST support `--walk-reflogs`/`-g` for reflog walking
- **FR-075**: System MUST support `--left-right` and `--cherry-pick`/`--cherry-mark`
- **FR-076**: System MUST auto-decorate when stdout is a terminal (matching `log.decorate=auto` behavior)
- **FR-077**: System MUST support `--ancestry-path` and `--simplify-by-decoration`
- **FR-078**: System MUST support `--decorate-refs`/`--decorate-refs-exclude` patterns
- **FR-079**: System MUST support `--source` to show which ref each commit was reached from
- **FR-080**: System MUST support `--use-mailmap` to apply `.mailmap` transformations

**show Command Flags**

- **FR-085**: System MUST support `--decorate` for ref decoration
- **FR-086**: System MUST support `-q`/`--quiet` to suppress diff output
- **FR-087**: System MUST show full diff output when showing annotated tags (tag header + tagged commit + diff)

**branch Command Flags**

- **FR-090**: System MUST support `-t`/`--track` and `--no-track` for upstream tracking configuration
- **FR-091**: System MUST support `-u`/`--set-upstream-to` and `--unset-upstream`
- **FR-092**: System MUST support `-c`/`-C` for branch copying
- **FR-093**: System MUST support `--merged`/`--no-merged` for merge-status filtering
- **FR-094**: System MUST support `--sort` for custom sort keys
- **FR-095**: System MUST support `-f`/`--force` for force-creating branches

**switch Command Flags**

- **FR-100**: System MUST support `-c` as short flag for `--create` (resolving global `-c` conflict)
- **FR-101**: System MUST support `--guess`/`--no-guess` for DWIM remote tracking branch creation
- **FR-102**: System MUST support `-q`/`--quiet`, `-m`/`--merge`, `--conflict`
- **FR-103**: System MUST support `--orphan` for creating orphan branches
- **FR-104**: System MUST support `-t`/`--track`/`--no-track`

**checkout Command Flags**

- **FR-110**: System MUST support `-q`, `-m`, `--conflict`, `--ours`/`--theirs`
- **FR-111**: System MUST support `-p`/`--patch` for interactive hunk checkout
- **FR-112**: System MUST support `-t`/`--track` and `--orphan`

**merge Command Flags**

- **FR-120**: System MUST rename `--cont` to `--continue` for git compatibility
- **FR-121**: System MUST support `--strategy`/`-s` and `-X`/`--strategy-option`
- **FR-122**: System MUST support `-v`/`--verbose`, `-q`/`--quiet`, `--stat`/`--no-stat`, `-e`/`--edit`
- **FR-123**: System MUST support `--allow-unrelated-histories`
- **FR-124**: System MUST support `-s`/`--signoff` and `--verify`/`--no-verify`

**rebase Command Flags**

- **FR-130**: System MUST implement `-i`/`--interactive` rebase with full editor-based commit manipulation (pick, reword, edit, squash, fixup, drop, exec)
- **FR-131**: System MUST support `-q`/`--quiet`, `-v`/`--verbose`, `--signoff`
- **FR-132**: System MUST support `-f`/`--force-rebase`, `--autosquash`/`--no-autosquash`
- **FR-133**: System MUST support `--autostash`/`--no-autostash`
- **FR-134**: System MUST support `--update-refs`, `-x`/`--exec`, `--root`
- **FR-135**: System MUST support `-s`/`--strategy`, `-X`/`--strategy-option`

**cherry-pick Command Flags**

- **FR-140**: System MUST support `-m`/`--mainline` for cherry-picking merge commits
- **FR-141**: System MUST support `-x` to append `(cherry picked from commit ...)` note
- **FR-142**: System MUST support `-s`/`--signoff`, `--ff`, `--strategy`/`-X`
- **FR-143**: System MUST support `--allow-empty`/`--allow-empty-message`

**revert Command Flags**

- **FR-150**: System MUST support `-m`/`--mainline` for reverting merge commits
- **FR-151**: System MUST support `-s`/`--signoff`, `--strategy`/`-X`

**tag Command Flags**

- **FR-160**: System MUST support `-s`/`--sign` and `-u`/`--local-user` for GPG signing
- **FR-161**: System MUST support `-F` for reading tag message from file
- **FR-162**: System MUST support `--sort`, `--contains`/`--no-contains`, `--merged`/`--no-merged`, `--format`, `--points-at`

**stash Command Flags**

- **FR-170**: System MUST support `--keep-index`/`-k` to keep staged changes in index during stash
- **FR-171**: System MUST support `--staged`/`-S` to stash only staged changes
- **FR-172**: System MUST support `-p`/`--patch` for interactive hunk stashing
- **FR-173**: System MUST support `branch` subcommand to create branch from stash
- **FR-174**: System MUST support `create`/`store` subcommands
- **FR-175**: System MUST support `stash drop stash@{N}` for any valid index N (not just 0)

**reset Command Flags**

- **FR-180**: System MUST support `--keep`, `-q`/`--quiet`, `-p`/`--patch`, `-N`/`--no-refresh`

**restore Command Flags**

- **FR-185**: System MUST support `--overlay`/`--no-overlay`, `-p`/`--patch`, `--conflict`, `--ours`/`--theirs`, `--pathspec-from-file`

**clean Command Flags**

- **FR-190**: System MUST support `-i` for interactive clean and `-e`/`--exclude` for additional exclude patterns

**blame Command Flags**

- **FR-195**: System MUST support `-b`, `--root`, `-f`/`--show-name`, `-p`/`--line-porcelain`, `-c`, `-t`, `-l`, `-s`, `--incremental`
- **FR-196**: System MUST implement `-C` (detect copies) and `-M` (detect moves) beyond stubs
- **FR-197**: System MUST support `--ignore-rev`/`--ignore-revs-file`

**shortlog Command Flags**

- **FR-200**: System MUST support `-c`/`--committer`, `-w` (line wrapping), `--group`

**reflog Subcommands**

- **FR-205**: System MUST implement `expire` and `delete` subcommands (currently stubs)
- **FR-206**: System MUST support `--date` flag for show subcommand
- **FR-207**: System MUST implement `exists` subcommand

**clone Command Flags**

- **FR-210**: System MUST support `-v`/`--verbose`, `--mirror`, `-l`/`--local`, `--single-branch`/`--no-single-branch`, `--recurse-submodules`, `--shallow-since`, `--shallow-exclude`, `--no-tags`, `--filter`, `--origin`, `--sparse`

**fetch Command Flags**

- **FR-215**: System MUST support `-v`, `-f`/`--force`, `--dry-run`, `-j`/`--jobs`, `--shallow-since`/`--shallow-exclude`, `--unshallow`/`--deepen`, `--recurse-submodules`, `--set-upstream`

**pull Command Flags**

- **FR-220**: System MUST support `-v`, `--stat`/`--no-stat`, `--log`/`--no-log`, `--squash`, `--commit`/`--no-commit`, `-e`/`--edit`, `--ff`/`--no-ff`, `--strategy`/`-X`, `--all`, `--depth`, `--tags`, `-p`/`--prune`, `--autostash`

**push Command Flags**

- **FR-225**: System MUST support `--all`, `--mirror`, `--thin`/`--no-thin`, `--signed`, `--recurse-submodules`

**remote Subcommands**

- **FR-230**: System MUST implement `set-head`, `prune`, `update`, `set-branches`, `get-url` subcommands

**config Command Flags**

- **FR-235**: System MUST support `--system`, `-f`/`--file`, `--get-all`, `--get-regexp`, `--replace-all`, `--add`, `--unset-all`, `--rename-section`, `--remove-section`, `-e`/`--edit`, `--type`/`--bool`/`--int`/`--path`, `-z`, `--name-only`, `--includes`

**describe Command Flags**

- **FR-240**: System MUST support `--contains`, `--all`, `--first-parent`, `--exact-match`, `--candidates`, `--match`/`--exclude`

**format-patch Command Flags**

- **FR-245**: System MUST support `-s`/`--signoff`, `-N`/`--no-numbered`, `-k`, `--to`, `--cc`, `--from`, `--in-reply-to`, `--base`, `-v`/`--reroll-count`, `--range-diff`

**gc Command Flags**

- **FR-250**: System MUST support `--cruft` for cruft pack generation

**Missing Commands**

- **FR-300**: System MUST implement `apply` (apply patches)
- **FR-301**: System MUST implement `cherry` (find commits not yet applied upstream)
- **FR-302**: System MUST implement `count-objects` (count unpacked objects and disk consumption)
- **FR-303**: System MUST implement `diff-files`, `diff-index`, `diff-tree` plumbing commands
- **FR-304**: System MUST implement `ls-remote` (list remote refs)
- **FR-305**: System MUST implement `merge-base` (find common ancestor)
- **FR-306**: System MUST implement `merge-file` (three-way file merge) and `merge-tree` (three-way tree merge)
- **FR-307**: System MUST implement `name-rev` (find symbolic names for revisions)
- **FR-308**: System MUST implement `range-diff` (compare two commit ranges)
- **FR-309**: System MUST implement `read-tree` (read tree information into index)
- **FR-310**: System MUST implement `fmt-merge-msg` (produce merge commit message)
- **FR-311**: System MUST implement `stripspace` (strip unnecessary whitespace)
- **FR-312**: System MUST implement `whatchanged` (show logs with diff for each commit)
- **FR-313**: System MUST implement `maintenance` (run maintenance tasks)
- **FR-314**: System MUST implement `sparse-checkout` (initialize/modify sparse checkout)
- **FR-315**: System MUST implement `rerere` (reuse recorded resolution of conflicted merges)
- **FR-316**: System MUST implement `difftool` (show changes using external diff tool)
- **FR-317**: System MUST implement `request-pull` (generate summary of pending changes)

**Behavioral / Data Parity**

- **FR-400**: System MUST support `.gitattributes` for line-ending normalization (CRLF/LF), diff drivers, merge drivers, and clean/smudge filters
- **FR-401**: System MUST support the full hook lifecycle: `pre-commit`, `prepare-commit-msg`, `commit-msg`, `post-commit`, `pre-rebase`, `post-rewrite`, `post-checkout`, `post-merge`, `pre-push`, `pre-receive`, `post-receive`, `update`, `pre-auto-gc`
- **FR-402**: System MUST support GPG signing for commits (`-S`/`--gpg-sign`) and tags (`-s`/`--sign`) using `gpg` or `gpg.program` config
- **FR-403**: System MUST support merge strategies: recursive (with options), ort, octopus, ours, subtree
- **FR-404**: System MUST support configurable rename/copy detection thresholds (`-M<n>`, `-C<n>`)
- **FR-405**: System MUST support shallow clone operations: `--deepen`, `--unshallow`, `--shallow-since`, `--shallow-exclude`
- **FR-406**: System MUST support `.mailmap` for author/committer name and email normalization
- **FR-407**: System MUST support config include directives (`[include]` and `[includeIf]`)
- **FR-408**: System MUST support credential helper protocol (`credential.helper` config)
- **FR-409**: System MUST support alternate object databases (`objects/info/alternates`)
- **FR-410**: System MUST follow git's editor resolution cascade: `$GIT_EDITOR` > `core.editor` config > `$VISUAL` > `$EDITOR` > default (`vi`)
- **FR-411**: System MUST support `merge.conflictStyle=diff3` producing `|||||||` base section in conflict markers
- **FR-412**: System MUST support interactive patch modes (`-p`/`--patch`) for `add`, `reset`, `stash`, `checkout`, `restore`

### Key Entities

- **ColorConfig**: Represents color settings per command area (diff, status, branch, etc.) with auto/always/never modes and customizable color slots
- **PagerConfig**: Represents pager selection cascade and per-command pager overrides (`pager.<cmd>`)
- **MergeStrategy**: Represents a merge algorithm (recursive, ort, octopus, ours, subtree) with strategy-specific options
- **Gitattributes**: Represents per-path attribute rules for line endings, diff drivers, merge drivers, and filters
- **Mailmap**: Represents author/committer identity mappings for normalization
- **HookType**: Represents a git hook event with its expected arguments and stdin behavior
- **CredentialHelper**: Represents an external credential storage provider following git's credential helper protocol

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For every command, `gitr <cmd> -h` and `git <cmd> -h` output identical flag sets (100% flag coverage across all 73+ commands)
- **SC-002**: All 18 missing commands are implemented and produce output matching git's format for standard inputs
- **SC-003**: `gitr diff`, `gitr log`, `gitr status`, `gitr branch`, `gitr grep`, and `gitr show` produce colored output matching git's default color scheme when run in a terminal
- **SC-004**: Pager is automatically invoked for long output in all applicable commands, following git's pager resolution cascade
- **SC-005**: `gitr --version` output format matches `git --version` pattern
- **SC-006**: `gitr --help` output has categorized command grouping matching git's layout
- **SC-007**: Global `-C`/`-c` flag conflicts are resolved — `gitr switch -c`, `gitr diff -C`, `gitr branch -c/-C` all work as their subcommand-specific meanings
- **SC-008**: Interactive rebase (`gitr rebase -i`) supports all standard operations (pick, reword, edit, squash, fixup, drop)
- **SC-009**: All behavioral features (`.gitattributes`, `.mailmap`, config includes, hooks, merge strategies, GPG signing, credential helpers) pass end-to-end tests comparing output with git
- **SC-010**: A comprehensive end-to-end test suite validates output parity between gitr and git for 50+ common workflows

## Assumptions

- Git 2.39+ is the reference version for parity comparison
- GPG signing depends on the user having `gpg` installed and configured; gitr delegates to the external `gpg` binary (matching git's approach) rather than implementing crypto natively
- Interactive modes (`-p`/`-i`) use the standard terminal (`/dev/tty`) for input, matching git's approach
- Pager integration reuses `std::process::Command` to spawn the pager process and pipe stdout to it
- The `ort` merge strategy may initially be implemented as an alias for the recursive strategy, with full ort optimization deferred if scope is too large
- `difftool` delegates to external diff tools configured via `diff.tool` config — gitr does not implement a built-in visual diff
- Pack protocol v2 is desirable but not required for parity with git 2.39 (which supports both v1 and v2)
- `maintenance` command implements `gc`, `commit-graph`, and `prefetch` tasks; `incremental-repack` and `pack-refs` tasks are included; `loose-objects` cleanup is included
- `sparse-checkout` implements cone mode as the default (matching git's current default)

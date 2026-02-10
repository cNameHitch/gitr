# Research: Full Git CLI Parity (Phase 3)

**Feature Branch**: `026-git-parity-phase3`
**Date**: 2026-02-09

## Research Task 1: Color Output Approach

**Context**: Color infrastructure already exists in `git-utils/src/color.rs` with `ColorMode` (Auto/Always/Never), `Color` enum (15 colors + attributes), `use_color()`, and `colorize()` functions. What's missing is per-command/per-slot `ColorConfig` reading from git config, and integration of color into each command's output.

### Decision: Extend existing `git-utils/src/color.rs` with `ColorConfig` struct; integrate into commands

**Rationale**: The foundation is already in place. The remaining work is: (1) a `ColorConfig` struct that reads `color.ui`, `color.<cmd>`, and `color.<cmd>.<slot>` from config, (2) adding `--color` flag to each command's clap args, and (3) wrapping output strings in color escapes.

**Alternatives considered**:
- **New `git-color` crate**: Rejected — existing module already well-placed in `git-utils`, adding a crate for a config struct is overkill.
- **External crate (e.g., `termcolor`, `colored`)**: Rejected — existing hand-written ANSI code mapping gives exact control needed for git byte-parity.

**Implementation approach**:
- Extend `git-utils/src/color.rs`: Add `ColorConfig` struct with per-command and per-slot color resolution from `GitConfig`
- Add `--color=auto|always|never` to each command's clap args (can be a shared arg group)
- Each command resolves effective color mode: CLI flag > per-command config > `color.ui` > default `Auto`
- Terminal detection already uses `std::io::IsTerminal` and respects `NO_COLOR`/`GIT_NO_COLOR`

---

## Research Task 2: Pager Integration

**Context**: Pager infrastructure already exists in `git-utils/src/pager.rs` with `setup_pager()`, `PagerGuard` RAII struct, `resolve_pager()` cascade (`$GIT_PAGER` > `core.pager` > `$PAGER` > `less`), `LESS=FRX` / `LV=-c` env setup, and terminal detection. What's missing is integration into the CLI main loop and per-command pager overrides.

### Decision: Integrate existing `git-utils` pager into CLI main.rs dispatch; add per-command config overrides

**Rationale**: The pager implementation is complete. The remaining work is: (1) calling `setup_pager()` from `main.rs` before command dispatch for auto-paged commands, (2) adding `-p`/`--paginate` and `-P`/`--no-pager` global flags, (3) supporting per-command `pager.<cmd>` config overrides.

**Implementation approach**:
- Add `-p`/`--paginate` and `-P`/`--no-pager` to the `Cli` struct's global flags
- In `main()`, after parsing but before `run()`, set up pager for auto-paged commands (log, diff, show, blame, shortlog, grep, branch, tag, help) unless `--no-pager` is set
- Read `pager.<cmd>` config to allow per-command pager override (e.g., `pager.log=less -S`)
- Pipe command stdout through `PagerGuard`'s stdin pipe

---

## Research Task 3: Global Flag Conflict Resolution (`-C`/`-c`)

**Context**: Git's `-C` (change directory) and `-c` (config override) are global flags, but several subcommands use the same short flags with different meanings: `switch -c` (create), `diff -C` (copy detection), `branch -c/-C` (copy), `grep -C` (context lines).

### Decision: Remove `-C`/`-c` global short flags; keep only `--git-dir`, long forms, and positional dispatch

**Rationale**: Git itself handles this by having the top-level parser consume known global flags before dispatching to subcommands. Clap's `global = true` propagates short flags into all subcommands, creating conflicts. The solution is to **not use `-C` and `-c` as global short flags** — instead, process them manually in `preprocess_args()` before clap parsing, or use clap's external subcommand pattern.

**Alternatives considered**:
- **Clap `global = true` with subcommand overrides**: Rejected — clap does not allow subcommand-level short flags to shadow global short flags. This is a known limitation.
- **Two-pass parsing**: Viable — first pass extracts global `-C`/`-c` args before the subcommand name, second pass parses normally. This is the chosen approach.

**Implementation approach**:
- In `preprocess_args()`, scan for `-C <path>` and `-c key=value` that appear *before* the subcommand name
- Strip them from args and store in a side-channel (environment variables or a pre-parse struct)
- Remove `#[arg(short = 'C', global = true)]` and `#[arg(short = 'c', global = true)]` from the `Cli` struct
- Subcommand-specific `-c` (switch) and `-C` (diff, branch) are defined normally in their arg structs

---

## Research Task 4: Interactive Patch Mode (`add -p`, `reset -p`, etc.)

**Context**: Git's interactive patch mode presents individual hunks for y/n/s/q selection via `/dev/tty`. This is used by `add -p`, `reset -p`, `stash -p`, `checkout -p`, `restore -p`.

### Decision: Interactive hunk selector module in `git-cli` using `/dev/tty` direct I/O

**Rationale**: Interactive mode is entirely a CLI concern. The diff engine (`git-diff`) already produces hunks; the interactive layer just presents them to the user and collects responses.

**Alternatives considered**:
- **External TUI framework (ratatui, crossterm)**: Rejected — git's interactive mode is simple line-based I/O (not a full TUI). Using a TUI framework adds complexity and deps for a simple y/n prompt loop.
- **Library-level interactive trait**: Rejected — interactive I/O is a CLI concern per constitution principle III.

**Implementation approach**:
- `git-cli/src/interactive.rs`: `InteractiveHunkSelector` struct
- Open `/dev/tty` directly for user input (matching git's approach, works even when stdin is piped)
- Present each hunk with context, accept commands: `y` (yes), `n` (no), `q` (quit), `a` (all remaining), `d` (done), `s` (split), `e` (edit), `?` (help)
- Hunk splitting: re-run diff on the hunk's range with smaller context to produce sub-hunks
- Used by `add -p`, `reset -p`, `stash -p`, `checkout -p`, `restore -p`

---

## Research Task 5: Interactive Rebase (`rebase -i`)

**Context**: `rebase -i` opens an editor with a todo list of commits (pick/reword/edit/squash/fixup/drop/exec). The user edits the file, saves, and the rebase engine processes the instructions.

### Decision: Editor-based todo list in `git-cli/src/commands/rebase.rs` with sequencer integration

**Rationale**: Interactive rebase is just non-interactive rebase with a user-edited todo list. The sequencer in `git-merge` already handles sequential commit application. The interactive layer adds: (1) generate todo file, (2) open editor, (3) parse edited todo, (4) execute via sequencer.

**Implementation approach**:
- Generate `.git/rebase-merge/git-rebase-todo` file with `pick <hash> <subject>` lines
- Open editor using git's editor cascade (`$GIT_EDITOR` > `core.editor` > `$VISUAL` > `$EDITOR` > `vi`)
- Parse the edited file: handle `pick`, `reword`, `edit`, `squash`, `fixup`, `drop`, `exec`, `break`, `label`, `reset`, `merge`
- For each instruction, delegate to the existing sequencer
- `--autosquash`: reorder fixup!/squash! commits before presenting the todo
- `--autostash`: stash before rebase, pop after
- State files in `.git/rebase-merge/` for `--continue`/`--abort`/`--skip`

---

## Research Task 6: `.gitattributes` Support

**Context**: Attribute infrastructure already exists in `git-index/src/attributes.rs` with `AttributeStack`, `AttributeValue` (Set/Unset/Value/Unspecified), `AttributeRule`, `add_file()`, `add_patterns()`, `get()`, `get_all()`, and path-matching. What's missing is: (1) EOL normalization behavior during add/checkout, (2) diff/merge driver integration, (3) clean/smudge filter execution, (4) loading from `core.attributesFile` and `$GIT_DIR/info/attributes`.

### Decision: Extend existing `git-index/src/attributes.rs`; add behavior modules for eol/drivers/filters

**Rationale**: The parsing and lookup infrastructure is in place in `git-index`. Rather than creating a new crate, extend the existing module with behavioral hooks. `git-diff` can depend on `git-index` for attribute lookup (this dependency direction already exists or is natural since diff needs index access for staged diffs).

**Alternatives considered**:
- **New `git-attributes` crate**: Rejected — the parsing already lives in `git-index` and works. Extracting would require moving code and adding a new workspace member for little benefit. `git-diff` already naturally depends on (or can depend on) `git-index`.
- **Leave as-is and just extend**: This is the chosen approach — lowest friction, avoids workspace churn.

**Implementation approach**:
- Extend `AttributeStack` to load from `core.attributesFile` config and `$GIT_DIR/info/attributes`
- Add `is_binary()`, `eol_for()`, `diff_driver()`, `merge_driver()`, `filter_for()` convenience methods
- Add EOL normalization in `git-index` add/checkout paths (LF→CRLF on checkout, CRLF→LF on add)
- Add clean/smudge filter execution via `std::process::Command`
- `git-diff` reads attributes for diff driver selection via `git-index` dependency

---

## Research Task 7: `.mailmap` Support

**Context**: `.mailmap` maps old author/committer names and emails to canonical forms. Used by `log --use-mailmap`, `shortlog`, `blame`.

### Decision: Module in `git-utils` crate (not a standalone crate)

**Rationale**: Mailmap is a simple text file parser (~100 lines) with a lookup function. It doesn't warrant its own crate. `git-utils` is the natural home for small utilities used across the workspace.

**Implementation approach**:
- `git-utils/src/mailmap.rs`: Parse `.mailmap` file (4 formats per git docs), build a lookup table
- `Mailmap::lookup(name: &[u8], email: &[u8]) -> (BString, BString)` returns the canonical name/email
- Loaded from `.mailmap` in repo root and `mailmap.file` config
- Used by log/shortlog/blame formatters in `git-cli`

---

## Research Task 8: Config `[include]` and `[includeIf]` Support

**Context**: Config include support already fully exists in `git-config/src/include.rs` with `process_includes()`, `collect_includes()`, `resolve_include_path()`, `evaluate_condition()` for `gitdir:`, `gitdir/i:`, `onbranch:`, and `hasconfig:` conditions. Circular include detection is implemented with `MAX_INCLUDE_DEPTH = 10`. Path expansion supports `~/` tilde.

### Decision: FR-407 is largely already implemented — verify and test

**Rationale**: The implementation is complete. Remaining work is limited to verifying full compatibility with git's include behavior and adding E2E parity tests.

**Implementation approach**:
- Write E2E tests comparing `gitr config --list` vs `git config --list` with include directives
- Verify edge cases: circular include detection, missing include files (should be silently ignored), relative path resolution
- No new code expected unless test gaps are found

---

## Research Task 9: GPG Signing Integration

**Context**: Git delegates GPG signing to the external `gpg` binary (or `gpg.program` config). gitr should do the same.

### Decision: GPG delegation via `std::process::Command` in `git-repository`

**Rationale**: Per the spec assumptions, gitr delegates to the external `gpg` binary rather than implementing crypto. This is a thin integration layer.

**Implementation approach**:
- `git-repository/src/gpg.rs`: `sign_buffer(data: &[u8], key: Option<&str>) -> Result<Vec<u8>>` spawns gpg
- Read `user.signingKey`, `gpg.program` (default: `gpg`), `gpg.format` (default: `openpgp`) from config
- For commits: sign the commit object content, produce signature in `gpgsig` header
- For tags: sign the tag object, append signature
- Verification: `verify_signature(object: &[u8], signature: &[u8]) -> Result<GpgStatus>`

---

## Research Task 10: Credential Helper Protocol

**Context**: Git's credential helper protocol is a simple text-based protocol for storing/retrieving credentials.

### Decision: Module in `git-transport` crate

**Rationale**: Credential helpers are exclusively used by the transport layer (clone, fetch, push) to authenticate against remote servers.

**Implementation approach**:
- `git-transport/src/credential.rs`: `CredentialHelper` struct implementing the protocol
- Protocol: write `protocol=\nhost=\nusername=\npassword=\n\n` to helper's stdin, read response
- Helper resolution: `credential.helper` config, supports `store`, `cache`, `osxkeychain`, arbitrary commands
- Integration: call credential helper before HTTP/SSH authentication in transport layer

---

## Research Task 11: Hook Lifecycle Completeness

**Context**: gitr already has partial hook support (pre-commit, commit-msg in `commit.rs`, push hooks). Phase 3 needs the complete lifecycle.

### Decision: Centralized hook runner in `git-repository`

**Rationale**: Hook execution is needed across many operations (commit, merge, rebase, push, checkout). A centralized runner avoids duplication.

**Implementation approach**:
- `git-repository/src/hooks.rs`: `HookRunner` struct with `run_hook(name: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<HookResult>`
- Discovers hook scripts in `.git/hooks/` directory
- Respects `core.hooksPath` config
- Returns exit code and captured stdout/stderr
- Hooks needed: `pre-commit`, `prepare-commit-msg`, `commit-msg`, `post-commit`, `pre-rebase`, `post-rewrite`, `post-checkout`, `post-merge`, `pre-push`, `pre-auto-gc`, `reference-transaction`
- Server-side hooks (`pre-receive`, `post-receive`, `update`) deferred to server implementation

---

## Research Task 12: New Commands Crate Placement

**Context**: 18 new commands need to be implemented (FR-300 through FR-317).

### Decision: All new commands as modules in `git-cli/src/commands/`

**Rationale**: All commands follow the existing pattern: thin CLI modules that parse args and delegate to library crates. No new crates needed for command implementations — the commands use existing crates (`git-diff`, `git-revwalk`, `git-merge`, `git-repository`, `git-transport`).

**New command modules**:
| Command | Module | Primary library crate |
|---------|--------|-----------------------|
| `apply` | `apply.rs` | `git-merge` (already has `apply` module) |
| `cherry` | `cherry.rs` | `git-revwalk` |
| `count-objects` | `count_objects.rs` | `git-odb` |
| `diff-files` | `diff_files.rs` | `git-diff` |
| `diff-index` | `diff_index.rs` | `git-diff` |
| `diff-tree` | `diff_tree.rs` | `git-diff` |
| `ls-remote` | `ls_remote.rs` | `git-transport` |
| `merge-base` | `merge_base.rs` | `git-revwalk` |
| `merge-file` | `merge_file.rs` | `git-merge` |
| `merge-tree` | `merge_tree.rs` | `git-merge` |
| `name-rev` | `name_rev.rs` | `git-revwalk` + `git-ref` |
| `range-diff` | `range_diff.rs` | `git-revwalk` + `git-diff` |
| `read-tree` | `read_tree.rs` | `git-index` + `git-odb` |
| `fmt-merge-msg` | `fmt_merge_msg.rs` | `git-merge` |
| `stripspace` | `stripspace.rs` | `git-utils` |
| `whatchanged` | `whatchanged.rs` | `git-revwalk` + `git-diff` |
| `maintenance` | `maintenance.rs` | `git-odb` + `git-pack` |
| `sparse-checkout` | `sparse_checkout.rs` | `git-index` |
| `rerere` | `rerere.rs` | `git-merge` |
| `difftool` | `difftool.rs` | `git-diff` (external tool delegation) |
| `request-pull` | `request_pull.rs` | `git-revwalk` + `git-diff` |

---

## Research Task 13: New Crate Assessment

**Context**: The spec introduces new subsystems. Need to decide which warrant new crates vs. modules in existing crates.

### Decision: No new crates — all features as modules in existing crates

**Rationale**: Extensive exploration reveals that infrastructure already exists for color (`git-utils`), pager (`git-utils`), gitattributes (`git-index`), and config includes (`git-config`). No cross-cutting concerns remain that would require a new crate to avoid circular dependencies.

- Color: Already in `git-utils/src/color.rs` — extend with `ColorConfig`
- Pager: Already in `git-utils/src/pager.rs` — integrate into CLI
- Gitattributes: Already in `git-index/src/attributes.rs` — extend with behavioral hooks
- Config includes: Already in `git-config/src/include.rs` — verify and test
- Mailmap: Small utility (~100 LOC), add to `git-utils`
- GPG: Integration layer, add to `git-repository`
- Credential: Transport concern, extend `git-transport`
- Hooks: Repository concern, extend `git-repository`
- Interactive modes: CLI-only concern, add to `git-cli`
- Rerere: Merge concern, add to `git-merge`

**Workspace after phase 3**: 16 crates (unchanged)

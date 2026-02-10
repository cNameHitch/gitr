# Quickstart: Full Git CLI Parity (Phase 3)

**Branch**: `026-git-parity-phase3` | **Date**: 2026-02-09

## Prerequisites

- Rust 1.75+ (`rustup update stable`)
- Git 2.39+ (reference version for parity comparison)
- GPG (`gpg --version`) — only needed for signing tests
- A terminal that supports ANSI colors

## Build & Test

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Run only the new parity tests
cargo test --workspace -- e2e_color_pager e2e_missing_flags e2e_new_commands e2e_behavioral_parity

# Check for lint issues
cargo clippy --workspace

# Build release binary for manual testing
cargo build --release
# Binary at: target/release/gitr
```

## Development Workflow

### 1. Color Output

**Key files**:
- `crates/git-utils/src/color.rs` — ANSI codes, `ColorConfig`, `ColorMode`
- Each command's `run()` function — add `--color` flag handling

**Quick test**:
```bash
# Compare side by side
diff <(git diff --color=always) <(target/release/gitr diff --color=always)
diff <(git log --oneline -5 --color=always) <(target/release/gitr log --oneline -5 --color=always)
```

### 2. Pager Integration

**Key files**:
- `crates/git-cli/src/pager.rs` — `PagerConfig`, `setup_pager()`
- `crates/git-cli/src/main.rs` — pager setup before command dispatch

**Quick test**:
```bash
# Should invoke pager (scrollable output)
target/release/gitr log --all

# Should NOT invoke pager
target/release/gitr -P log --all
echo "no pager" | target/release/gitr log --all  # piped = no pager
```

### 3. Missing Flags

**Key files**: Each command in `crates/git-cli/src/commands/<cmd>.rs`

**Quick test pattern**:
```bash
# Compare help output for flag coverage
diff <(git commit -h 2>&1) <(target/release/gitr commit -h 2>&1)
```

### 4. New Commands

**Key files**: New modules in `crates/git-cli/src/commands/`

**Quick test pattern**:
```bash
# Compare output for new commands
diff <(git merge-base HEAD HEAD~1) <(target/release/gitr merge-base HEAD HEAD~1)
diff <(git diff-tree -r HEAD) <(target/release/gitr diff-tree -r HEAD)
diff <(echo "  hello  " | git stripspace) <(echo "  hello  " | target/release/gitr stripspace)
```

### 5. `.gitattributes`

**Key files**:
- `crates/git-attributes/src/lib.rs` — NEW CRATE
- `crates/git-index/src/attributes.rs` — integration with add/checkout

**Quick test**:
```bash
# Create test repo
mkdir /tmp/attr-test && cd /tmp/attr-test && git init
echo "*.txt text eol=crlf" > .gitattributes
echo "hello" > test.txt
git add .gitattributes test.txt
# Compare gitr add behavior
```

### 6. Interactive Modes

**Key files**:
- `crates/git-cli/src/interactive.rs` — hunk selector
- `crates/git-cli/src/commands/rebase.rs` — interactive rebase

**Quick test** (manual):
```bash
# Create changes, then test interactive staging
echo "change" >> some_file.txt
target/release/gitr add -p
# Should show y/n/q/a/s/e prompt per hunk
```

## Existing Infrastructure (Already Implemented)

These modules already exist and just need integration/extension:

| Module | Location | Status | Phase 3 Work |
|--------|----------|--------|--------------|
| Color | `git-utils/src/color.rs` | `ColorMode`, `Color`, `use_color()`, `colorize()` exist | Add `ColorConfig` struct, integrate `--color` flag into commands |
| Pager | `git-utils/src/pager.rs` | `setup_pager()`, `PagerGuard`, resolution cascade exist | Wire into `main.rs`, add `-p`/`-P` global flags |
| Gitattributes | `git-index/src/attributes.rs` | `AttributeStack`, `AttributeValue`, `get()`, `add_file()` exist | Add eol normalization, diff/merge drivers, clean/smudge filters |
| Config includes | `git-config/src/include.rs` | `process_includes()`, conditions, cycle detection exist | Verify E2E parity, add tests |
| Credential parsing | `git-cli/src/commands/credential.rs` | Protocol parsing exists | Add actual helper invocation via `Command` |
| Hook execution | `git-repository/` (partial) | Pre-commit, commit-msg hooks called | Add remaining hooks (pre-push, post-checkout, etc.) |

## Architecture Reference

### Dependency Graph (relevant subset)

```
git-utils (leaf: color [EXISTS], pager [EXISTS], mailmap [NEW])
    ↑
git-config (include resolution [EXISTS])
    ↑
git-index (attributes [EXISTS — extend], sparse checkout [NEW])
git-diff (colored output [NEW], pickaxe [NEW])
git-merge (rerere [NEW], octopus [NEW])
    ↑
git-repository (hooks [EXTEND], GPG [NEW], editor [EXTEND])
    ↑
git-transport (credential helpers [EXTEND])
    ↑
git-cli (pager integration [NEW], interactive modes [NEW], all commands)
```

### Key Patterns

**Adding a new flag to an existing command**:
1. Add the `#[arg(...)]` field to the command's `Args` struct
2. Handle the flag in the command's `run()` function
3. Add E2E test comparing gitr vs git output

**Adding a new command**:
1. Create `crates/git-cli/src/commands/<cmd>.rs` with `Args` struct and `run()` function
2. Add `pub mod <cmd>;` to `commands/mod.rs`
3. Add variant to `Commands` enum in `commands/mod.rs`
4. Add dispatch arm in `run()` function in `commands/mod.rs`
5. Add E2E parity test

**Adding color to a command**:
1. Import `ColorConfig` and `ColorMode` from `git-utils`
2. Add `--color` arg to command's `Args` struct
3. Resolve effective color mode: `color_config.effective_mode("cmd", args.color)`
4. Wrap output strings in ANSI escapes based on color mode

## Implementation Priority

| Priority | Category | Est. FRs | Description |
|----------|----------|----------|-------------|
| P1-A | Color output | 5 | ANSI color support (FR-010 through FR-014) |
| P1-B | Pager | 4 | Pager integration (FR-020 through FR-023) |
| P1-C | Top-level CLI | 4 | Global flags, help, version (FR-001 through FR-004) |
| P1-D | Core flags | ~40 | Missing flags for commit, status, diff, log, show, branch, switch, checkout, merge, rebase (FR-030 through FR-135) |
| P1-E | Remaining flags | ~25 | cherry-pick, revert, tag, stash, reset, restore, clean, blame, shortlog, reflog, clone, fetch, pull, push, remote, config, describe, format-patch, gc (FR-140 through FR-250) |
| P2-A | New commands | 18 | All missing commands (FR-300 through FR-317) |
| P2-B | Config includes | 1 | [include]/[includeIf] (FR-407) |
| P2-C | Hooks | 1 | Complete hook lifecycle (FR-401) |
| P2-D | .gitattributes | 1 | New crate + integration (FR-400) |
| P2-E | Behavioral | 9 | mailmap, GPG, merge strategies, credential, alternates, editor, conflict style, rename thresholds, shallow ops (FR-402 through FR-411) |
| P3-A | Interactive modes | 1 | add -p, reset -p, stash -p, checkout -p, restore -p (FR-412) |
| P3-B | Interactive rebase | 1 | rebase -i (FR-130) |

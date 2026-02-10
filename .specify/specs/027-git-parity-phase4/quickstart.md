# Quickstart: Git Parity Phase 4

**Feature**: 027-git-parity-phase4 | **Date**: 2026-02-09

## Overview

This phase completes all remaining git behavioral parity gaps across 4 categories: no-op flags, stub subcommands, interactive modes, and incomplete core engines. All changes are within the existing Cargo workspace — no new crates, no new dependencies.

## Prerequisites

- Rust 1.75+ with cargo
- Git 2.39+ (reference implementation for parity testing)
- macOS or Linux (for maintenance start/stop platform-specific code)

## Build & Test

```bash
# Build the workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Run a specific crate's tests
cargo test -p git-revwalk
cargo test -p git-merge
cargo test -p git-cli
```

## Implementation Order (Recommended)

### Phase A: Core Engine Fixes (P1 — Foundation)
These fix correctness bugs in existing code and unblock later work.

1. **Sequencer abort** — fix `abort()` in `git-merge/src/sequencer.rs` to restore HEAD/index
2. **Octopus merge** — implement 3+ head iterative merge in `git-merge/src/strategy/octopus.rs`
3. **Subtree merge** — implement shift detection in `git-merge/src/strategy/subtree.rs`

### Phase B: No-Op Flag Completion (P1 — Walk & Diff)
These add real functionality behind flags that currently do nothing.

4. **Log --follow** — rename tracking during walk in `git-revwalk`
5. **Log --left-right/--cherry-pick/--cherry-mark** — symmetric diff in `git-revwalk`
6. **Log --ancestry-path** — ancestry filtering in `git-revwalk`
7. **Log --source** — ref source tracking in `git-revwalk`
8. **Diff --no-index** — non-repo file comparison in `git-cli/commands/diff.rs`
9. **Diff --check** — whitespace error detection in `git-diff` + `git-cli/commands/diff.rs`
10. **Pull --rebase** — fetch + rebase integration in `git-cli/commands/pull.rs`

### Phase C: Stub Subcommands (P1 — Commands)
These replace "not implemented" messages with real functionality.

11. **Remote get-url** — config lookup in `git-cli/commands/remote.rs`
12. **Remote set-head** — symbolic ref creation in `git-cli/commands/remote.rs`
13. **Remote prune** — stale ref removal in `git-cli/commands/remote.rs`
14. **Remote update** — multi-remote fetch in `git-cli/commands/remote.rs`
15. **Remote set-branches** — refspec config update in `git-cli/commands/remote.rs`
16. **Reflog expire** — time-based entry removal in `git-ref` + `git-cli/commands/reflog.rs`
17. **Reflog delete** — entry removal by index in `git-ref` + `git-cli/commands/reflog.rs`
18. **Maintenance start** — scheduler registration in `git-cli/commands/maintenance.rs`
19. **Maintenance stop** — scheduler unregistration in `git-cli/commands/maintenance.rs`

### Phase D: Interactive Patch Modes (P2 — UX)
These add interactive hunk selection to commands.

20. **Manual hunk edit (e)** — complete the `e` action in `interactive.rs`
21. **add -p** — integrate interactive.rs with add command
22. **reset -p** — integrate interactive.rs with reset command
23. **checkout -p** — integrate interactive.rs with checkout command
24. **restore -p** — integrate interactive.rs with restore command
25. **stash -p** — integrate interactive.rs with stash command
26. **clean -i** — implement file selection menu in clean command

### Phase E: Interactive Rebase (P2 — Complex Feature)
This is the largest single feature.

27. **Rebase -i** — todo list generation, editor integration, action replay, autosquash

### Phase F: E2E Testing
28. **E2E interop tests** — validate output parity for all 28 items

## Key Integration Points

### interactive.rs API
```rust
// Existing API to reuse:
pub struct InteractiveHunkSelector { /* opens /dev/tty */ }
pub fn select_hunks(hunks: &mut [Hunk], prompt: &str) -> Result<()>
pub fn apply_hunks_to_content(content: &[u8], hunks: &[Hunk]) -> Vec<u8>
pub fn reverse_apply_hunks_to_content(content: &[u8], hunks: &[Hunk]) -> Vec<u8>

// New: manual edit action
pub fn edit_hunk_in_editor(hunk: &Hunk, editor: &str) -> Result<Hunk>
```

### Sequencer API
```rust
// Existing API to fix:
pub fn abort(&self, repo: &Repository) -> Result<()>
// Must now: reset HEAD to self.original_head, checkout tree, cleanup

// Extended for interactive rebase:
pub fn replay_interactive(&mut self, repo: &Repository) -> Result<()>
```

### RevWalk API
```rust
// New walk options:
pub struct WalkOptions {
    pub left_right: bool,      // annotate with </>
    pub cherry_pick: bool,     // filter equivalent commits
    pub cherry_mark: bool,     // mark equivalent commits
    pub ancestry_path: bool,   // restrict to ancestry chain
    pub source: bool,          // track originating ref
    pub follow_path: Option<BString>,  // rename-following path
}
```

## Testing Strategy

Each item gets:
1. **Unit test**: in the relevant library crate (e.g., git-revwalk, git-merge)
2. **Integration test**: in `git-cli` crate, testing CLI invocation and output format
3. **E2E interop test**: comparing gitr output against git output on identical repositories

```bash
# Run E2E tests
cargo test -p git-cli --test e2e_interop

# Run specific test
cargo test -p git-cli test_log_follow
```

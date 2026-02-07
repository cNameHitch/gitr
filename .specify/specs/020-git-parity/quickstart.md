# Quickstart: Git Command Parity

**Feature**: 020-git-parity | **Date**: 2026-02-07

## Prerequisites

- Rust 1.75+ with `cargo`
- C git 2.x on PATH
- macOS or Linux

## Build

```bash
cargo build --workspace
```

## Run Tests

```bash
# Run all tests
cargo test --workspace

# Run only parity interop tests
cargo test -p git-cli --test parity_tests

# Run a specific test
cargo test -p git-cli --test parity_tests test_ff_merge_ref_advance

# Run existing interop tests
cargo test -p git-cli --test e2e_workflow_tests
cargo test -p git-cli --test e2e_advanced_tests
cargo test -p git-cli --test e2e_edge_case_tests
```

## Verify Changes Manually

### Date Format
```bash
# Create a test repo
tmp=$(mktemp -d)
cd "$tmp"
GIT_COMMITTER_DATE="2009-02-13T23:31:30+0000" GIT_AUTHOR_DATE="2009-02-13T23:31:30+0000" \
  git init && echo "test" > file.txt && git add . && git commit -m "test commit"

# Compare outputs
git log --format=fuller -1
gitr log --format=fuller -1
# Both should show: "Thu Feb 13 23:31:30 2009 +0000"
```

### Diff Hunk Content
```bash
echo "modified" >> file.txt
git diff
gitr diff
# Both should show @@ hunk headers and +/- content lines
```

### Packfile Reading
```bash
# Create commits, then gc
for i in $(seq 1 12); do echo "$i" > "file$i.txt" && git add . && git commit -m "commit $i"; done
git gc
gitr log --oneline
# Should show all 12 commits even after packing
```

### Merge
```bash
git checkout -b feature
echo "feature" > feature.txt && git add . && git commit -m "feature"
git checkout main
gitr merge feature -m "merge feature"
gitr cat-file -p HEAD  # Should show two parent lines
```

### Stash
```bash
echo "wip" >> file.txt
gitr stash push -m "my stash"
gitr stash push -m "another stash"  # Should not overwrite first
gitr stash list  # Should show both entries
gitr stash pop
```

### Rev-parse Peeling
```bash
gitr rev-parse HEAD^{tree}   # Should output tree OID
gitr rev-parse HEAD^{commit} # Should output commit OID
```

## Implementation Order

1. **P1 — Core fixes** (can be done in parallel):
   - Date format default (DateFormat::Default + FormatOptions default)
   - Diff hunk content (verify/fix blob content reading)
   - Packfile REF_DELTA cross-pack resolution
   - Merge behavioral fixes (exit codes, output format)

2. **P1 — Output format** (depends on date format fix):
   - Log: empty repo exit code, format newlines
   - Show: date format
   - Blame: date+time format
   - Status: detached HEAD OID
   - ls-files: unicode path quoting

3. **P2 — Remote operations** (depends on packfile fix):
   - Clone over file://
   - Push/fetch/pull over file://
   - Remote config verification

4. **P2 — Stash and plumbing**:
   - Stash reflog stack
   - Stash --include-untracked
   - for-each-ref HEAD exclusion
   - rev-parse ^{type} peeling

5. **P3 — Rebase**:
   - Timestamp handling in commit replay

## Lint & CI

```bash
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

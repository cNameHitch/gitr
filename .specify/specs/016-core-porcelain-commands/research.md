# Research: Core Porcelain Commands

## C Source File Mapping

| C File | Rust Module | Command | Complexity |
|--------|-------------|---------|------------|
| builtin/init.c | `init.rs` | git init | Medium |
| builtin/clone.c | `clone.rs` | git clone | Complex |
| builtin/add.c | `add.rs` | git add | Medium |
| builtin/rm.c | `rm.rs` | git rm | Simple |
| builtin/mv.c | `mv.rs` | git mv | Simple |
| builtin/commit.c | `commit.rs` | git commit | Complex |
| builtin/status.c | `status.rs` | git status (part of commit.c in C) | Complex |
| builtin/branch.c | `branch.rs` | git branch | Medium |
| builtin/switch.c | `switch.rs` | git switch | Medium |
| builtin/checkout.c | `checkout.rs` | git checkout | Complex |
| builtin/merge.c | `merge.rs` | git merge | Complex |
| builtin/fetch.c | `fetch.rs` | git fetch | Complex |
| builtin/pull.c | `pull.rs` | git pull | Medium (fetch + merge/rebase) |
| builtin/push.c | `push.rs` | git push | Medium |
| builtin/remote.c | `remote.rs` | git remote | Medium |
| builtin/rebase.c | `rebase.rs` | git rebase | Complex |
| builtin/reset.c | `reset.rs` | git reset | Medium |
| builtin/tag.c | `tag.rs` | git tag | Medium |
| builtin/stash.c | `stash.rs` | git stash | Complex |
| builtin/clean.c | `clean.rs` | git clean | Simple |
| builtin/restore.c | `restore.rs` | git restore (part of checkout.c) | Medium |

## Command Interactions

Most complex interactions:
- `clone` = init + fetch + checkout (orchestrates multiple operations)
- `pull` = fetch + merge (or fetch + rebase)
- `commit` = write-tree + commit-tree + update-ref + reflog
- `checkout` = update-index + update-working-tree + update-HEAD
- `rebase` = cherry-pick sequence via sequencer

## Status Output Formats

### Long format (default)
```
On branch main
Changes to be committed:
  (use "git restore --staged <file>..." to unstage)
        modified:   file.txt

Changes not staged for commit:
        modified:   other.txt

Untracked files:
        new.txt
```

### Short format (--short)
```
M  file.txt
 M other.txt
?? new.txt
```
Two columns: first = index status, second = worktree status.

### Porcelain format (--porcelain=v2)
Machine-readable format with precise status codes.

## git checkout vs git switch/restore

C git split `checkout` into `switch` (branch switching) and `restore` (file restoration) for clarity. Our implementation:
- Implement `switch` and `restore` as the primary implementations
- Implement `checkout` as a wrapper that dispatches to switch/restore based on arguments

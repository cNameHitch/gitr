# Research: History & Inspection Commands

## C Source File Mapping

| C File | Rust Module | Command | Complexity |
|--------|-------------|---------|------------|
| builtin/log.c | `log.rs`, `show.rs`, `rev_list.rs` | git log/show/rev-list | Complex |
| builtin/diff.c | `diff.rs` | git diff | Medium |
| builtin/blame.c | `blame.rs` | git blame | Complex |
| builtin/bisect.c | `bisect.rs` | git bisect | Complex |
| builtin/shortlog.c | `shortlog.rs` | git shortlog | Simple |
| builtin/describe.c | `describe.rs` | git describe | Medium |
| builtin/grep.c | `grep.rs` | git grep | Medium |
| builtin/cherry-pick.c | `cherry_pick.rs` | git cherry-pick | Medium (delegates to merge) |
| builtin/revert.c | `revert.rs` | git revert | Medium |
| builtin/format-patch.c | `format_patch.rs` | git format-patch (part of log.c) | Complex |
| builtin/am.c | `am.rs` | git am | Complex |
| builtin/reflog.c | `reflog.rs` | git reflog | Medium |

## Blame Algorithm

C git's blame algorithm (line-by-line attribution):
1. Start with the current file version, all lines unblamed
2. Walk backwards through history
3. At each commit, diff the file against its parent
4. Lines that changed in this commit → blame this commit
5. Lines unchanged → pass blame to parent
6. With -C: check if lines were copied from other files in the same commit
7. With -C -C: check all files in the commit

The blame result is a list of `(line_range, commit_oid, original_line_number, original_filename)`.

## Bisect State

Files in .git/:
- `BISECT_START`: The original HEAD when bisect started
- `BISECT_LOG`: Log of bisect steps
- `BISECT_NAMES`: Pathspec restriction
- `BISECT_EXPECTED_REV`: Expected next test revision
- `refs/bisect/good-*`: Known good commits
- `refs/bisect/bad`: Known bad commit

## Describe Algorithm

1. From the target commit, walk backwards
2. Find all tags reachable from the commit
3. Select the tag with the fewest commits between it and the target
4. Output: `<tag>-<distance>-g<abbreviated-hash>`
5. If the commit IS a tag: just output the tag name

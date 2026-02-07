# Research: Plumbing Commands

## C Source File Mapping

| C File | Rust Module | Command |
|--------|-------------|---------|
| builtin/cat-file.c | `cat_file.rs` | git cat-file |
| builtin/hash-object.c | `hash_object.rs` | git hash-object |
| builtin/rev-parse.c | `rev_parse.rs` | git rev-parse |
| builtin/update-ref.c | `update_ref.rs` | git update-ref |
| builtin/for-each-ref.c | `for_each_ref.rs` | git for-each-ref |
| builtin/show-ref.c | `show_ref.rs` | git show-ref |
| builtin/symbolic-ref.c | `symbolic_ref.rs` | git symbolic-ref |
| builtin/ls-files.c | `ls_files.rs` | git ls-files |
| builtin/ls-tree.c | `ls_tree.rs` | git ls-tree |
| builtin/update-index.c | `update_index.rs` | git update-index |
| builtin/check-ignore.c | `check_ignore.rs` | git check-ignore |
| builtin/check-attr.c | `check_attr.rs` | git check-attr |
| builtin/mktree.c | `mktree.rs` | git mktree |
| builtin/mktag.c | `mktag.rs` | git mktag |
| builtin/commit-tree.c | `commit_tree.rs` | git commit-tree |
| builtin/verify-pack.c | `verify_pack.rs` | git verify-pack |
| builtin/check-ref-format.c | `check_ref_format.rs` | git check-ref-format |
| builtin/var.c | `var.rs` | git var |

## Command Complexity

- **Simple** (< 100 lines): hash-object, mktag, check-ref-format, var, symbolic-ref, write-tree
- **Medium** (100-300 lines): cat-file, show-ref, ls-tree, update-ref, commit-tree, mktree, verify-pack
- **Complex** (300+ lines): rev-parse, for-each-ref, ls-files, update-index, check-ignore, check-attr

## for-each-ref Format Strings

The `--format` option in for-each-ref supports a mini-language:
- `%(refname)`, `%(refname:short)`, `%(refname:strip=N)`
- `%(objectname)`, `%(objectname:short)`, `%(objecttype)`
- `%(authorname)`, `%(authordate)`, etc.
- `%(if)`, `%(then)`, `%(else)`, `%(end)` conditionals
- `%(align:width,position)`
- `%(color:...)` for ANSI colors

This format language is shared with `git branch --format`, `git tag --format`, and `git log --format`.

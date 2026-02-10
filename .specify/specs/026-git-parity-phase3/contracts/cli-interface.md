# CLI Interface Contract: Full Git CLI Parity (Phase 3)

**Branch**: `026-git-parity-phase3` | **Date**: 2026-02-09

This document defines the public CLI interface changes for phase 3. Since gitr is a CLI tool (not a web API), "contracts" are defined as the command-line interface specifications.

## Top-Level CLI Contract

### Global Flags (additions)

```
gitr [--work-tree=<path>] [--bare] [-p|--paginate] [-P|--no-pager]
     [--no-replace-objects] [--config-env=<name>=<envvar>]
     [--exec-path[=<path>]] [--namespace=<name>]
     [-C <path>] [-c <key>=<value>]
     [--git-dir=<path>]
     <command> [<args>]
```

### Version Output Contract

```
$ gitr --version
gitr version 0.1.1-alpha.10
```

### Help Output Contract (categorized)

```
$ gitr --help
usage: gitr [-C <path>] [-c <name>=<value>] [--git-dir=<path>]
            [--work-tree=<path>] [--namespace=<name>]
            [-p | --paginate | -P | --no-pager]
            <command> [<args>]

These are common gitr commands used in various situations:

start a working area
   clone      Clone a repository into a new directory
   init       Create an empty Git repository

work on the current change
   add        Add file contents to the index
   mv         Move or rename a file
   restore    Restore working tree files
   rm         Remove files from the working tree and index

examine the history and state
   bisect     Use binary search to find faulty commit
   diff       Show changes between commits
   grep       Print lines matching a pattern
   log        Show commit logs
   show       Show various types of objects
   status     Show the working tree status

grow, mark and tweak your common history
   branch     List, create, or delete branches
   commit     Record changes to the repository
   merge      Join two or more development histories
   rebase     Reapply commits on top of another base
   reset      Reset current HEAD to the specified state
   switch     Switch branches
   tag        Create, list, delete or verify a tag

collaborate
   fetch      Download objects and refs from another repository
   pull       Fetch from and integrate with another repository
   push       Update remote refs along with associated objects
```

## Color Contract

### Default Color Scheme

| Element | ANSI Code | Appearance |
|---------|-----------|------------|
| Diff: removed line | `\x1b[31m` | Red |
| Diff: added line | `\x1b[32m` | Green |
| Diff: hunk header | `\x1b[36m` | Cyan |
| Diff: meta info | `\x1b[1m` | Bold |
| Commit hash | `\x1b[33m` | Yellow |
| Branch (current) | `\x1b[32m` | Green |
| Branch (remote) | `\x1b[31m` | Red |
| Tag name | `\x1b[33;1m` | Bold yellow |
| HEAD decoration | `\x1b[36;1m` | Bold cyan |
| Reset | `\x1b[m` | Reset all |

### Color Flag Contract

All color-producing commands accept:
```
--color[=<when>]    when: auto (default), always, never
```

## Pager Contract

### Auto-Paged Commands

These commands auto-invoke pager when stdout is a terminal:
`log`, `diff`, `show`, `blame`, `shortlog`, `grep`, `branch`, `tag`, `help`

### Pager Environment

Before spawning pager, set (if not already set):
```
LESS=FRX
LV=-c
```

## New Commands Contract

### `gitr apply <patch>`
Apply a patch to files and/or to the index. Reads from stdin if no file specified.

### `gitr cherry [-v] <upstream> [<head> [<limit>]]`
Find commits not yet applied to upstream.

### `gitr count-objects [-v|--verbose] [-H|--human-readable]`
Count unpacked number of objects and their disk consumption.

### `gitr diff-files [-p] [-q] [--] [<path>...]`
Compare files in the working tree and the index.

### `gitr diff-index [--cached] [-p] <tree-ish> [--] [<path>...]`
Compare a tree to the working tree or index.

### `gitr diff-tree [-r] [-p] [--name-only] [--name-status] <tree-ish> [<tree-ish>] [--] [<path>...]`
Compare the content and mode of blobs found via two tree objects.

### `gitr ls-remote [--heads] [--tags] [--refs] [-q] <repository> [<patterns>...]`
List references in a remote repository.

### `gitr merge-base [--all] [--octopus] [--is-ancestor] [--fork-point] <commit>...`
Find as good common ancestors as possible for a merge.

### `gitr merge-file [-p|--stdout] [--diff3] [-L <label>] <current> <base> <other>`
Run a three-way file merge.

### `gitr merge-tree [--write-tree] <branch1> <branch2>`
Perform merge without touching index or working tree.

### `gitr name-rev [--tags] [--refs=<pattern>] [--no-undefined] [--always] <commit-ish>...`
Find symbolic names for given revs.

### `gitr range-diff [<options>] <range1> <range2>`
Compare two commit ranges.

### `gitr read-tree [-m] [-u|--reset] [--prefix=<prefix>] <tree-ish> [<tree-ish>...]`
Read tree information into the index.

### `gitr fmt-merge-msg [-m <message>] [--log[=<n>]] [--no-log] [-F <file>]`
Produce a merge commit message.

### `gitr stripspace [-s|--strip-comments] [-c|--comment-lines]`
Remove unnecessary whitespace.

### `gitr whatchanged [<options>] [<revision range>] [[--] <path>...]`
Show logs with difference each commit introduces.

### `gitr maintenance run [--task=<task>] [--auto] [--schedule=<frequency>]`
Run maintenance tasks (gc, commit-graph, prefetch, loose-objects, incremental-repack, pack-refs).

### `gitr sparse-checkout [init|set|add|reapply|disable|list] [<options>]`
Reduce the working tree to a subset of tracked files.

### `gitr rerere [clear|forget <pathspec>|diff|status|gc]`
Reuse recorded resolution of conflicted merges.

### `gitr difftool [--tool=<tool>] [--no-prompt] [<diff-options>] [<commit>] [--] [<path>...]`
Show changes using external diff tool.

### `gitr request-pull [-p] <start> <url> [<end>]`
Generate a summary of pending changes for email submission.

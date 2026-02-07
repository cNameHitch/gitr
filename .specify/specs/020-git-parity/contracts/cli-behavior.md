# CLI Behavior Contracts: Git Command Parity

**Feature**: 020-git-parity | **Date**: 2026-02-07

This document specifies the exact CLI behavior contracts for each command modified in this feature. Each contract defines the input, expected output, and exit code that must be byte-identical to C git.

---

## 1. `gitr merge`

### 1a. Fast-Forward Merge
```
Input:  gitr merge <branch>  (where HEAD is ancestor of <branch>)
Exit:   0
Stdout: "Updating <short-old>..<short-new>\nFast-forward\n <files changed summary>\n"
Effect: HEAD ref advances to <branch> tip. Working tree updated.
Verify: show-ref, ls-tree -r HEAD, cat-file -p HEAD
```

### 1b. Three-Way Clean Merge
```
Input:  gitr merge <branch> -m "<message>"  (non-conflicting changes)
Exit:   0
Stdout: "Merge made by the 'ort' strategy.\n <files changed summary>\n"
Effect: Merge commit created with two parents. Tree includes files from both branches.
Verify: cat-file -p HEAD (two "parent" lines), ls-tree -r HEAD (all files present)
```

### 1c. Three-Way Conflict
```
Input:  gitr merge <branch>  (conflicting changes to same file)
Exit:   1
Stdout: "Auto-merging <file>\nCONFLICT (content): Merge conflict in <file>\nAutomatic merge failed; fix conflicts and then commit the result.\n"
Effect: Conflict markers written to working tree. Index has stage 1/2/3 entries.
Verify: ls-files --stage (stage entries), working tree file (conflict markers)
```

---

## 2. `gitr diff`

### 2a. Unstaged Changes
```
Input:  gitr diff  (modified tracked file, not staged)
Exit:   0 (if no diff) or 1 (if diff exists, with --exit-code)
Stdout: Full unified diff with header + hunk content
Format: "diff --git a/<path> b/<path>\nindex <old>..<new> <mode>\n--- a/<path>\n+++ b/<path>\n@@ -L,C +L,C @@\n<context/add/del lines>\n"
```

### 2b. Cached Changes
```
Input:  gitr diff --cached  (staged modification)
Exit:   same as 2a
Stdout: Same format, comparing index to HEAD
```

### 2c. HEAD Diff
```
Input:  gitr diff HEAD  (all changes vs HEAD)
Exit:   same as 2a
Stdout: Same format, comparing working tree to HEAD
```

---

## 3. `gitr log`

### 3a. Default Format
```
Input:  gitr log
Exit:   0
Date:   "Thu Feb 13 23:31:30 2009 +0000" (C git default format, commit timezone)
Format: "commit <oid>\nAuthor: <name> <<email>>\nDate:   <date>\n\n    <message>\n"
```

### 3b. Custom Format
```
Input:  gitr log --format=%s
Exit:   0
Stdout: Each subject on its own line, newline-separated
```

### 3c. Empty Repository
```
Input:  gitr log  (no commits)
Exit:   128
Stderr: "fatal: your current branch 'main' does not have any commits yet\n"
Stdout: (empty)
```

---

## 4. `gitr show`

### 4a. Show HEAD
```
Input:  gitr show HEAD
Exit:   0
Date:   Same as log default format (C git default, commit timezone)
Stdout: Commit info + diff
```

---

## 5. `gitr blame`

### 5a. Default Format
```
Input:  gitr blame <file>
Exit:   0
Format: "<oid-prefix> (<author> <date+time> <tz> <lineno>) <content>"
OID:    7+ character prefix (auto-abbreviated)
Date:   "YYYY-MM-DD HH:MM:SS <tz>"
Align:  Right-aligned line numbers, padded author field
```

---

## 6. `gitr status`

### 6a. Detached HEAD
```
Input:  gitr status  (detached HEAD)
Exit:   0
Stdout: "HEAD detached at <short-oid>\n..."
```

---

## 7. `gitr ls-files`

### 7a. Unicode Paths
```
Input:  gitr ls-files  (repo with non-ASCII filenames)
Exit:   0
Stdout: Quoted paths with octal escapes: "caf\303\251.txt"
```

### 7b. With -z Flag
```
Input:  gitr ls-files -z  (NUL-terminated)
Exit:   0
Stdout: Raw byte paths separated by NUL, no quoting
```

---

## 8. `gitr for-each-ref`

### 8a. Standard Output
```
Input:  gitr for-each-ref --format="%(refname) %(objectname) %(objecttype)"
Exit:   0
Stdout: Only refs under refs/ — HEAD is NOT included
```

---

## 9. `gitr rev-parse`

### 9a. Peeling to Tree
```
Input:  gitr rev-parse HEAD^{tree}
Exit:   0
Stdout: <tree-oid>\n
```

### 9b. Peeling to Commit
```
Input:  gitr rev-parse v1.0^{commit}
Exit:   0
Stdout: <commit-oid>\n  (tag dereferenced to commit)
```

---

## 10. `gitr clone`

### 10a. Normal Clone
```
Input:  gitr clone file:///path/to/bare repo-clone
Exit:   0
Effect: Working copy with .git/, correct refs, remote config, checked-out files
Verify: log, show-ref, ls-tree, config --get remote.origin.url
```

### 10b. Bare Clone
```
Input:  gitr clone --bare file:///path/to/bare repo-bare
Exit:   0
Effect: Bare repo (HEAD, refs/, objects/, no .git/ wrapper, no working tree)
```

---

## 11. `gitr push`

### 11a. Push to Remote
```
Input:  gitr push origin main  (new commits)
Exit:   0
Effect: Remote refs updated, objects transferred
Verify: C git clone of remote sees pushed commits
```

---

## 12. `gitr fetch`

### 12a. Fetch from Remote
```
Input:  gitr fetch origin
Exit:   0
Effect: Remote-tracking refs updated, objects received
Verify: show-ref shows updated refs/remotes/origin/*
```

---

## 13. `gitr pull`

### 13a. Pull (Fetch + Merge)
```
Input:  gitr pull origin main  (remote ahead by commits)
Exit:   0
Effect: Local branch fast-forwards to remote tip
Verify: log --oneline matches C git
```

---

## 14. `gitr stash`

### 14a. Push and Pop
```
Input:  gitr stash push  →  gitr stash pop
Exit:   0
Effect: Working tree clean after push, modifications restored after pop
Verify: status --porcelain, diff
```

### 14b. Multiple Stashes
```
Input:  gitr stash push (×3)  →  gitr stash list
Exit:   0
Stdout: "stash@{0}: On <branch>: <message>\nstash@{1}: ...\nstash@{2}: ...\n"
```

### 14c. Include Untracked
```
Input:  gitr stash push --include-untracked
Exit:   0
Effect: Untracked files removed from working tree, restored on pop
```

---

## 15. `gitr rebase`

### 15a. Linear Rebase
```
Input:  gitr rebase main  (from feature branch, 2 commits)
Exit:   0
Effect: Feature commits replayed on top of main tip
Verify: log --oneline shows correct order; OIDs match C git when timestamps pinned
```

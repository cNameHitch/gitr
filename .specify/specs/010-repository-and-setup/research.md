# Research: Repository & Setup

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| repository.c/h | ~500 | `lib.rs` | Repository struct |
| setup.c | ~1500 | `discover.rs` | Git dir discovery |
| environment.c | ~400 | `env.rs` | Environment variable handling |
| common-main.c | ~100 | `lib.rs` | Entry point setup |

## .git Directory Structure

```
.git/
├── HEAD              # Symbolic ref to current branch or detached OID
├── config            # Repository-local config
├── description       # Description (for gitweb)
├── hooks/            # Hook scripts
│   ├── pre-commit.sample
│   └── ...
├── info/
│   ├── exclude       # Local gitignore
│   └── refs          # Pack refs supplementary data
├── objects/
│   ├── info/
│   │   └── alternates
│   ├── pack/
│   └── XX/           # Loose objects
├── refs/
│   ├── heads/        # Local branches
│   ├── tags/         # Tags
│   └── remotes/      # Remote tracking branches
├── logs/             # Reflogs
│   ├── HEAD
│   └── refs/
├── packed-refs       # Packed references
├── index             # Staging area
├── COMMIT_EDITMSG    # Last commit message
├── MERGE_HEAD        # OID of merge target (during merge)
├── MERGE_MSG         # Merge commit message template
└── ...
```

## Discovery Algorithm

C git's `setup_git_directory()`:
1. Check `$GIT_DIR` → use directly if set
2. From CWD, walk up the directory tree:
   a. Check if current dir contains `.git/` directory → found
   b. Check if current dir contains `.git` file → read gitdir: redirect
   c. Check if current dir IS a git dir (has HEAD, objects/, refs/) → bare repo
   d. Check against `$GIT_CEILING_DIRECTORIES` → stop if at ceiling
   e. Go to parent directory, repeat
3. If not found → error "not a git repository"

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `GIT_DIR` | Override .git directory location |
| `GIT_WORK_TREE` | Override working tree location |
| `GIT_CEILING_DIRECTORIES` | Colon-separated dirs to stop discovery at |
| `GIT_OBJECT_DIRECTORY` | Override objects/ location |
| `GIT_ALTERNATE_OBJECT_DIRECTORIES` | Extra object dirs (colon-separated) |
| `GIT_COMMON_DIR` | Override common dir (for worktrees) |
| `GIT_INDEX_FILE` | Override index file location |
| `GIT_CONFIG` | Override local config file |

## Worktree Structure

Main repo: normal `.git/` directory
Linked worktree: `.git` is a FILE containing:
```
gitdir: /path/to/main/repo/.git/worktrees/<name>
```

The worktree git dir contains:
```
/path/to/main/repo/.git/worktrees/<name>/
├── HEAD          # Independent HEAD
├── index         # Independent index
├── commondir     # File containing: "../.." (path to main .git)
└── gitdir        # File containing: /path/to/worktree
```

Shared between worktrees via commondir:
- objects/, refs/, packed-refs, config, info/, hooks/

## gitoxide Reference

`gix::Repository` is the equivalent:
- `gix::open()` → discover + open
- `gix::init()` → create new repo
- Lazy subsystem initialization
- Thread-safe via interior mutability patterns

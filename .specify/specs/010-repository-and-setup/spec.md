# Feature Specification: Repository & Setup

**Feature Branch**: `010-repository-and-setup`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity, 006-object-database, 008-reference-system, 009-configuration-system

## User Scenarios & Testing

### User Story 1 - Repository Discovery (Priority: P1)

As a gitr library consumer, I need to find the `.git` directory by walking up from the current directory so that git commands work from any subdirectory.

**Why this priority**: Every git command starts by discovering the repository.

**Independent Test**: Navigate to a deep subdirectory of a repo, run discovery, verify .git dir is found.

**Acceptance Scenarios**:

1. **Given** a directory inside a git repository, **When** discovery runs, **Then** the `.git` directory is found by walking up the directory tree.
2. **Given** a bare repository, **When** discovery runs, **Then** the repository root (which IS the git dir) is identified.
3. **Given** a directory outside any repository, **When** discovery runs, **Then** an appropriate error is returned.
4. **Given** `GIT_DIR` environment variable set, **When** discovery runs, **Then** that path is used directly.
5. **Given** `GIT_WORK_TREE` set, **When** discovery runs, **Then** the working tree is at the specified path.
6. **Given** a `.git` file (not directory) containing `gitdir: /path/to/real/git/dir`, **When** discovered, **Then** the redirection is followed (worktrees, submodules).

---

### User Story 2 - Repository Initialization (Priority: P1)

As a git user, I need to create new repositories so that `git init` works.

**Why this priority**: Creating repos is a fundamental operation. Also needed for testing.

**Independent Test**: Init a new repo, verify structure matches C git, then use C git to interact with it.

**Acceptance Scenarios**:

1. **Given** an empty directory, **When** `git init` is run, **Then** a `.git` directory is created with the standard structure.
2. **Given** `--bare` flag, **When** `git init --bare` is run, **Then** a bare repository is created without a working tree.
3. **Given** a custom template directory, **When** init is run with `--template`, **Then** the template is copied into `.git/`.
4. **Given** an existing repository, **When** `git init` is re-run, **Then** it is a safe no-op (existing data preserved).

---

### User Story 3 - Repository Struct (Priority: P1)

As a gitr library developer, I need a `Repository` struct that ties all subsystems together so that code doesn't need to manage individual components.

**Why this priority**: The Repository struct is the central entry point for all git operations.

**Independent Test**: Open a repository, access ODB, refs, config, index through the Repository struct.

**Acceptance Scenarios**:

1. **Given** a valid git repository path, **When** `Repository::open()` is called, **Then** ODB, refs, config, and index are all initialized.
2. **Given** the Repository struct, **When** `.odb()` is called, **Then** the object database is available.
3. **Given** the Repository struct, **When** `.refs()` is called, **Then** the ref store is available.
4. **Given** the Repository struct, **When** `.config()` is called, **Then** the merged config is available.
5. **Given** the Repository struct, **When** `.index()` is called, **Then** the index is loaded on demand.

---

### User Story 4 - Worktree Support (Priority: P2)

As a git user, I need multiple worktrees linked to the same repository.

**Why this priority**: Worktrees are commonly used for parallel development on multiple branches.

**Acceptance Scenarios**:

1. **Given** a main worktree, **When** `git worktree add` creates a linked worktree, **Then** the new worktree shares the same ODB.
2. **Given** a linked worktree, **When** the Repository is opened, **Then** it correctly identifies its git_dir and the main repo's common_dir.
3. **Given** multiple worktrees, **When** each is opened independently, **Then** they share objects and refs but have independent HEAD, index, and worktree.

---

### User Story 5 - Environment Variables (Priority: P2)

As a gitr library, I need to respect all standard git environment variables for overrides.

**Why this priority**: Scripts and CI systems depend on environment overrides.

**Acceptance Scenarios**:

1. **Given** `GIT_DIR`, **When** opening a repo, **Then** it overrides .git discovery.
2. **Given** `GIT_WORK_TREE`, **When** opening a repo, **Then** it overrides the working tree path.
3. **Given** `GIT_CEILING_DIRECTORIES`, **When** discovering, **Then** discovery stops at the ceiling.
4. **Given** `GIT_OBJECT_DIRECTORY`, **When** opening ODB, **Then** the custom path is used.
5. **Given** `GIT_ALTERNATE_OBJECT_DIRECTORIES`, **When** loading alternates, **Then** extra dirs are included.

### Edge Cases

- Repository with `.git` as a file (gitlink for submodules/worktrees)
- Repository on a network filesystem (NFS, SMB)
- Repository at filesystem root (`/`)
- GIT_CEILING_DIRECTORIES with symlinks
- Bare repository with detached HEAD
- Repository with no commits (unborn branch)
- Missing `.git/HEAD` file (corrupt repo)
- `.git` directory with wrong permissions

## Requirements

### Functional Requirements

- **FR-001**: System MUST discover the git directory by walking up from CWD (or GIT_DIR)
- **FR-002**: System MUST support bare repositories (no working tree)
- **FR-003**: System MUST initialize new repositories with standard `.git/` structure
- **FR-004**: System MUST provide a `Repository` struct exposing all subsystems (ODB, refs, config, index)
- **FR-005**: System MUST support `.git` file redirects (gitdir: path)
- **FR-006**: System MUST respect environment variables: GIT_DIR, GIT_WORK_TREE, GIT_CEILING_DIRECTORIES, GIT_OBJECT_DIRECTORY, GIT_ALTERNATE_OBJECT_DIRECTORIES, GIT_COMMON_DIR
- **FR-007**: System MUST support worktrees with shared commondir
- **FR-008**: System MUST lazy-load subsystems (index only loaded when needed)
- **FR-009**: System MUST identify repository type (normal, bare, linked worktree)
- **FR-010**: System MUST support `init.defaultBranch` config for new repo default branch name

### Key Entities

- **Repository**: Central struct tying all subsystems together
- **GitDir**: Path to the `.git` directory (or bare repo root)
- **WorkTree**: Path to the working directory
- **CommonDir**: Shared directory for worktrees

## Success Criteria

### Measurable Outcomes

- **SC-001**: Repository discovery matches C git's behavior for all test cases (verified against `git rev-parse --git-dir`)
- **SC-002**: Initialized repositories have identical structure to C git `git init`
- **SC-003**: All standard environment variables are respected, matching C git behavior
- **SC-004**: Repository::open() completes in < 10ms for a typical repo
- **SC-005**: Worktree detection correctly identifies linked worktrees

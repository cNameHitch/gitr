# Feature Specification: Plumbing Commands

**Feature Branch**: `015-plumbing-commands`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-018 (all library crates)

## User Scenarios & Testing

### User Story 1 - Object Inspection (Priority: P1)

As a git scripter, I need `cat-file` and `hash-object` so that scripts can inspect and create objects programmatically.

**Why this priority**: These are the most commonly used plumbing commands, essential for scripting and testing.

**Independent Test**: Create objects with hash-object, inspect with cat-file, verify round-trip.

**Acceptance Scenarios**:

1. **Given** `git cat-file -t <oid>`, **When** run, **Then** the object type is printed (blob/tree/commit/tag).
2. **Given** `git cat-file -s <oid>`, **When** run, **Then** the object size in bytes is printed.
3. **Given** `git cat-file -p <oid>`, **When** run, **Then** the pretty-printed object content is shown.
4. **Given** `git cat-file blob <oid>`, **When** run, **Then** raw blob content is written to stdout.
5. **Given** `git cat-file --batch`, **When** OIDs are piped to stdin, **Then** each object is output in batch format.
6. **Given** `echo "hello" | git hash-object --stdin`, **When** run, **Then** the SHA-1 of the blob is printed.
7. **Given** `git hash-object -w file.txt`, **When** run, **Then** the file is stored as a blob and its OID is printed.

---

### User Story 2 - Reference Plumbing (Priority: P1)

As a git scripter, I need `rev-parse`, `update-ref`, `for-each-ref`, `show-ref`, and `symbolic-ref` for scriptable ref operations.

**Why this priority**: Script-level ref operations are essential for CI/CD and tooling.

**Acceptance Scenarios**:

1. **Given** `git rev-parse HEAD`, **When** run, **Then** the full OID of HEAD is printed.
2. **Given** `git rev-parse --git-dir`, **When** run, **Then** the path to .git is printed.
3. **Given** `git update-ref refs/heads/test <oid>`, **When** run, **Then** the ref is created/updated.
4. **Given** `git for-each-ref --format='%(refname) %(objectname:short)'`, **When** run, **Then** all refs are listed with the format applied.
5. **Given** `git show-ref`, **When** run, **Then** all refs are listed with their OIDs.
6. **Given** `git symbolic-ref HEAD`, **When** run, **Then** the target of HEAD is printed.

---

### User Story 3 - Index Plumbing (Priority: P1)

As a git scripter, I need `update-index`, `ls-files`, `ls-tree`, and `check-ignore` for programmatic index and tree inspection.

**Why this priority**: Index and tree inspection are used by build systems and code review tools.

**Acceptance Scenarios**:

1. **Given** `git ls-files --stage`, **When** run, **Then** all index entries are listed with mode, OID, stage, and path.
2. **Given** `git ls-tree HEAD`, **When** run, **Then** the tree entries at HEAD are listed.
3. **Given** `git ls-tree -r HEAD`, **When** run, **Then** all entries recursively are listed.
4. **Given** `git update-index --add file.txt`, **When** run, **Then** the file is added to the index.
5. **Given** `git check-ignore *.o`, **When** run, **Then** ignored patterns are checked and reported.
6. **Given** `git check-attr diff -- file.txt`, **When** run, **Then** the gitattributes for the file are displayed.

---

### User Story 4 - Object Creation Plumbing (Priority: P2)

As a git scripter, I need `mktree`, `mktag`, and `commit-tree` for low-level object construction.

**Why this priority**: Used by advanced scripts, filter-branch, and BFG-like tools.

**Acceptance Scenarios**:

1. **Given** tree entries on stdin, **When** `git mktree` is run, **Then** a tree object is created and its OID is printed.
2. **Given** tag data on stdin, **When** `git mktag` is run, **Then** a tag object is created after validation.
3. **Given** `git commit-tree <tree> -p <parent> -m "msg"`, **When** run, **Then** a commit object is created and its OID is printed.

---

### User Story 5 - Verification Plumbing (Priority: P2)

As a git scripter, I need `verify-pack`, `check-ref-format`, and `var` for validation and inspection.

**Acceptance Scenarios**:

1. **Given** `git verify-pack -v pack.idx`, **When** run, **Then** all pack entries are listed and validated.
2. **Given** `git check-ref-format refs/heads/main`, **When** run, **Then** exit 0 (valid ref name).
3. **Given** `git check-ref-format "invalid..name"`, **When** run, **Then** exit 1 (invalid ref name).
4. **Given** `git var GIT_AUTHOR_IDENT`, **When** run, **Then** the author identity is printed.

### Edge Cases

- `cat-file --batch` with thousands of OIDs (performance)
- `hash-object` with binary content containing null bytes
- `rev-parse` with all revision syntax forms
- `for-each-ref` with complex format strings including `if`/`else`
- `update-ref -d` with stale old-value check
- `ls-files` with unmerged entries (multiple stages)

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement `cat-file` with -t, -s, -p, --batch, --batch-check modes
- **FR-002**: System MUST implement `hash-object` with --stdin, -w, -t flags
- **FR-003**: System MUST implement `rev-parse` with all revision syntax and --git-dir, --show-toplevel, etc.
- **FR-004**: System MUST implement `update-ref` with create, update, delete, and -d --stdin (transaction) mode
- **FR-005**: System MUST implement `for-each-ref` with --format, --sort, --count, --contains
- **FR-006**: System MUST implement `show-ref` with --head, --heads, --tags, --verify
- **FR-007**: System MUST implement `symbolic-ref` for reading and writing symbolic refs
- **FR-008**: System MUST implement `ls-files` with --stage, --cached, --deleted, --modified, --others
- **FR-009**: System MUST implement `ls-tree` with -r, -d, -t, --name-only, --format
- **FR-010**: System MUST implement `update-index` with --add, --remove, --force-remove, --cacheinfo
- **FR-011**: System MUST implement `check-ignore` for testing gitignore patterns
- **FR-012**: System MUST implement `check-attr` for testing gitattributes
- **FR-013**: System MUST implement `mktree`, `mktag`, `commit-tree` for object creation
- **FR-014**: System MUST implement `verify-pack` for pack validation
- **FR-015**: System MUST implement `check-ref-format` for ref name validation
- **FR-016**: System MUST implement `var` for git variable inspection
- **FR-017**: System MUST implement `write-tree` for creating tree from index

### Key Entities

All plumbing commands are thin wrappers around library crate APIs. No new domain entities — they use ObjectDatabase, RefStore, Index, etc. directly.

## Success Criteria

### Measurable Outcomes

- **SC-001**: All plumbing commands produce identical output to C git for a comprehensive test suite
- **SC-002**: `cat-file --batch` processes 10K objects/second minimum
- **SC-003**: `rev-parse` handles all documented revision syntax forms
- **SC-004**: All plumbing commands have matching exit codes to C git (0 for success, 1 for error, 128 for fatal)

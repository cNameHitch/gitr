# Feature Specification: Index / Staging Area

**Feature Branch**: `007-index-staging-area`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity, 003-object-model, 006-object-database

## User Scenarios & Testing

### User Story 1 - Read the Index (Priority: P1)

As a gitr library consumer, I need to read the index file (`.git/index`) to know which files are staged and their metadata.

**Why this priority**: The index is read by almost every command: status, diff, commit, checkout, merge.

**Independent Test**: Stage files with C git, read the index with gitr, verify all entries match.

**Acceptance Scenarios**:

1. **Given** a valid `.git/index` file (v2), **When** parsed, **Then** all cache entries are returned with correct path, OID, mode, and stat data.
2. **Given** a v3 index with extended flags, **When** parsed, **Then** extended flags (intent-to-add, skip-worktree) are preserved.
3. **Given** a v4 index with path compression, **When** parsed, **Then** paths are correctly decompressed.
4. **Given** an index with extensions (TREE, REUC, UNTR), **When** parsed, **Then** extensions are preserved.
5. **Given** a corrupt index (bad checksum), **When** read, **Then** a descriptive error is returned.

---

### User Story 2 - Write the Index (Priority: P1)

As a gitr library, I need to write the index file atomically so that `git add`, `git rm`, and other operations can update the staging area.

**Why this priority**: Staging changes is the core git workflow. Writing the index is essential.

**Independent Test**: Modify the index with gitr, verify with C git `git ls-files --stage`.

**Acceptance Scenarios**:

1. **Given** modified index entries, **When** written, **Then** the index file is created atomically (lock file + rename).
2. **Given** a written index, **When** read by C git, **Then** all entries and extensions are correct.
3. **Given** entries to write, **When** serialized, **Then** they are sorted by path name.
4. **Given** an index with extensions, **When** written, **Then** extensions are preserved in the output.

---

### User Story 3 - Stage and Unstage Files (Priority: P1)

As a git user, I need to add and remove files from the staging area.

**Why this priority**: `git add` and `git rm` are among the most common operations.

**Independent Test**: Add a file, verify it appears in the index. Remove it, verify it's gone.

**Acceptance Scenarios**:

1. **Given** a working tree file, **When** staged (added to index), **Then** the file's content is hashed, stored as a blob, and an index entry is created.
2. **Given** a staged file, **When** removed from index, **Then** the entry is deleted.
3. **Given** a modified file already in the index, **When** re-staged, **Then** the entry is updated with the new OID and stat data.

---

### User Story 4 - Gitignore Processing (Priority: P2)

As a git user, I need `.gitignore` patterns to be respected so that build artifacts and temp files are excluded from staging.

**Why this priority**: Gitignore is expected behavior for `git add` and `git status`.

**Independent Test**: Create `.gitignore` with patterns, attempt to add ignored files, verify they are excluded.

**Acceptance Scenarios**:

1. **Given** a `.gitignore` with `*.o`, **When** adding all files, **Then** `.o` files are skipped.
2. **Given** nested `.gitignore` files, **When** processing ignores, **Then** patterns are scoped to their directory.
3. **Given** a negation pattern `!important.o`, **When** processing, **Then** that file is NOT ignored.
4. **Given** `$GIT_DIR/info/exclude`, **When** loaded, **Then** those patterns also apply.
5. **Given** `core.excludesFile` config, **When** loaded, **Then** the global ignore file is respected.

---

### User Story 5 - Gitattributes Processing (Priority: P2)

As a gitr library, I need to read `.gitattributes` for line ending conversion, diff drivers, and merge strategies.

**Why this priority**: Attributes affect how files are stored and displayed.

**Acceptance Scenarios**:

1. **Given** `.gitattributes` with `*.txt text`, **When** staging a text file, **Then** line endings are normalized.
2. **Given** attribute `binary`, **When** diffing, **Then** binary diff is used.
3. **Given** attribute `merge=ours`, **When** merging, **Then** the ours strategy is used.

---

### User Story 6 - Pathspec Matching (Priority: P2)

As a git user, I need pathspec patterns to filter which files are affected by commands.

**Why this priority**: Pathspecs are used by add, diff, log, and many other commands.

**Acceptance Scenarios**:

1. **Given** pathspec `src/*.rs`, **When** matching index entries, **Then** only Rust files in src/ match.
2. **Given** pathspec `:(exclude)*.test`, **When** matching, **Then** test files are excluded.
3. **Given** pathspec `:(top)README`, **When** matching from a subdirectory, **Then** the pattern is relative to repo root.

---

### User Story 7 - Cache Tree Extension (Priority: P2)

As a gitr library, I need the TREE cache extension for fast tree object creation during commit.

**Why this priority**: Without cache tree, creating the tree hierarchy requires reading all blobs. Cache tree makes commit O(changed files) instead of O(all files).

**Acceptance Scenarios**:

1. **Given** an index with a valid cache tree, **When** committing, **Then** unchanged subtrees reuse cached tree OIDs.
2. **Given** a modified file, **When** the cache tree is invalidated, **Then** only the affected path is recomputed.

### Edge Cases

- Index with 100K+ entries (large monorepo)
- File paths with non-UTF-8 bytes
- Merge conflicts (CE_CONFLICT flag, multiple stage entries for same path)
- Case-insensitive filesystem with case-conflicting paths
- Sparse checkout (skip-worktree flag)
- Split index (shared + personal extensions)
- Index entry with stat data that doesn't match working tree (racily clean)
- Symlinks in the working tree

## Requirements

### Functional Requirements

- **FR-001**: System MUST read index file format v2, v3, and v4
- **FR-002**: System MUST write index file format v2 (default) with atomic lock file write
- **FR-003**: System MUST parse cache entries: path, OID, mode, flags, stat data (ctime, mtime, dev, ino, uid, gid, size)
- **FR-004**: System MUST support merge conflict entries (stages 1, 2, 3 for same path)
- **FR-005**: System MUST parse and write index extensions: TREE, REUC (resolve-undo), UNTR (untracked cache), FSMN (filesystem monitor)
- **FR-006**: System MUST implement gitignore pattern matching (`.gitignore`, `info/exclude`, `core.excludesFile`)
- **FR-007**: System MUST implement gitattributes processing (`.gitattributes`, `info/attributes`)
- **FR-008**: System MUST implement pathspec matching with magic signatures (top, exclude, glob, icase)
- **FR-009**: System MUST maintain sorted order of entries by path
- **FR-010**: System MUST verify index checksum on read
- **FR-011**: System MUST support split index extension for large repos
- **FR-012**: System MUST support sparse index for sparse checkouts

### Key Entities

- **Index**: The complete staging area (parsed `.git/index`)
- **IndexEntry**: A single file entry with path, OID, mode, stat, flags
- **CacheTree**: Cached tree object hierarchy for fast commit
- **IgnoreStack**: Layered gitignore pattern matching
- **AttributeStack**: Layered gitattributes matching
- **Pathspec**: Parsed pathspec pattern for file filtering

## Success Criteria

### Measurable Outcomes

- **SC-001**: Index files written by gitr are byte-for-byte readable by C git
- **SC-002**: Index read/write round-trip preserves all entries and extensions
- **SC-003**: Gitignore matching produces identical results to C git for a test suite of 200+ patterns
- **SC-004**: Index operations on 100K entries complete in < 500ms
- **SC-005**: Pathspec matching is identical to C git for all magic signature types

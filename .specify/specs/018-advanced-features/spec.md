# Feature Specification: Advanced Features

**Feature Branch**: `018-advanced-features`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: All prior specs (001-017)

## User Scenarios & Testing

### User Story 1 - Garbage Collection (Priority: P1)

As a git user, I need `git gc` and `git repack` to optimize repository storage.

**Why this priority**: Repository maintenance is essential for long-term performance.

**Acceptance Scenarios**:

1. **Given** a repository with many loose objects, **When** `git gc` runs, **Then** objects are packed and loose objects are removed.
2. **Given** `git gc --aggressive`, **When** run, **Then** more thorough packing with better delta compression is performed.
3. **Given** unreachable objects, **When** `git gc --prune=now`, **Then** unreachable objects are deleted.
4. **Given** `git repack -a -d`, **When** run, **Then** all objects are repacked into a single pack and old packs are deleted.

---

### User Story 2 - Repository Integrity (Priority: P1)

As a git user, I need `git fsck` to verify repository integrity.

**Acceptance Scenarios**:

1. **Given** `git fsck`, **When** run, **Then** all objects are verified for corruption.
2. **Given** a dangling commit, **When** fsck runs, **Then** it is reported as dangling.
3. **Given** a corrupt object, **When** fsck runs, **Then** the corruption is reported with details.
4. **Given** `git fsck --unreachable`, **When** run, **Then** all unreachable objects are listed.

---

### User Story 3 - Submodules (Priority: P2)

As a git user, I need submodule support for managing nested repositories.

**Acceptance Scenarios**:

1. **Given** `git submodule add <url> path`, **When** run, **Then** the submodule is registered and cloned.
2. **Given** `git submodule update --init`, **When** run, **Then** all submodules are initialized and checked out.
3. **Given** `git submodule status`, **When** run, **Then** the current state of each submodule is shown.
4. **Given** `git clone --recurse-submodules`, **When** run, **Then** submodules are automatically cloned.

---

### User Story 4 - Worktree Management (Priority: P2)

As a git user, I need `git worktree` commands to manage multiple working trees.

**Acceptance Scenarios**:

1. **Given** `git worktree add ../feature feature-branch`, **When** run, **Then** a new worktree is created.
2. **Given** `git worktree list`, **When** run, **Then** all worktrees are listed with their HEAD and branch.
3. **Given** `git worktree remove ../feature`, **When** run, **Then** the worktree is removed.

---

### User Story 5 - Notes and Replace (Priority: P3)

As a git user, I need `git notes` and `git replace` for metadata and history rewriting.

**Acceptance Scenarios**:

1. **Given** `git notes add -m "Note" HEAD`, **When** run, **Then** a note is attached to the commit.
2. **Given** `git notes show HEAD`, **When** run, **Then** the note content is displayed.
3. **Given** `git replace <old> <new>`, **When** run, **Then** the old object is replaced by the new one transparently.

---

### User Story 6 - Archive Generation (Priority: P2)

As a git user, I need `git archive` to export repository contents as tar/zip.

**Acceptance Scenarios**:

1. **Given** `git archive HEAD --format=tar`, **When** run, **Then** a tar archive of the tree at HEAD is produced.
2. **Given** `git archive --prefix=project/ HEAD`, **When** run, **Then** all files are under the prefix directory.
3. **Given** `git archive --format=zip HEAD`, **When** run, **Then** a zip archive is produced.

---

### User Story 7 - Security Features (Priority: P2)

As a git user, I need GPG signing and credential management.

**Acceptance Scenarios**:

1. **Given** `commit.gpgSign=true`, **When** committing, **Then** the commit is GPG-signed.
2. **Given** `git tag -s v1.0`, **When** run, **Then** the tag is GPG-signed.
3. **Given** `git verify-commit HEAD`, **When** run, **Then** the commit signature is verified.
4. **Given** `git credential fill`, **When** run, **Then** credential helpers are consulted.

---

### User Story 8 - Hooks and Fsmonitor (Priority: P2)

As a git user, I need hook support for custom workflow automation.

**Acceptance Scenarios**:

1. **Given** a pre-commit hook, **When** `git commit` runs, **Then** the hook is executed and can abort the commit.
2. **Given** a post-receive hook, **When** a push is received, **Then** the hook is executed.
3. **Given** `core.fsmonitor` configured, **When** running status, **Then** the fsmonitor daemon is consulted for changed files.

### Edge Cases

- gc running while another git operation is in progress
- fsck on a repository with millions of objects (performance)
- Submodule with circular references
- Archive of a tree with symlinks
- GPG key not found or expired
- Hook script with incorrect permissions
- Worktree on different filesystem than main repo
- Replace object creating a cycle

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement `gc` with --aggressive, --prune, --auto
- **FR-002**: System MUST implement `repack` with -a, -d, -f, --write-bitmap-hashcache
- **FR-003**: System MUST implement `prune` for removing unreachable objects
- **FR-004**: System MUST implement `fsck` with --unreachable, --dangling, --root, --strict
- **FR-005**: System MUST implement `pack-objects` and `index-pack` for low-level pack operations
- **FR-006**: System MUST implement `submodule` with add, init, update, status, foreach, sync, deinit
- **FR-007**: System MUST implement `worktree` with add, list, remove, lock, unlock, move, prune
- **FR-008**: System MUST implement `notes` with add, show, list, remove, merge
- **FR-009**: System MUST implement `replace` with --graft, -d, -l
- **FR-010**: System MUST implement `archive` with --format=tar/zip, --prefix, --output
- **FR-011**: System MUST implement GPG signing for commits and tags
- **FR-012**: System MUST implement `verify-commit` and `verify-tag` for signature verification
- **FR-013**: System MUST implement `credential` helper protocol (fill, approve, reject)
- **FR-014**: System MUST execute hooks at all standard hook points (pre-commit, commit-msg, post-commit, pre-push, etc.)
- **FR-015**: System MUST support `core.fsmonitor` for filesystem monitoring integration
- **FR-016**: System MUST implement `fast-import` for bulk data import
- **FR-017**: System MUST implement `bundle` create and unbundle for offline transfer
- **FR-018**: System MUST implement `daemon` for anonymous git:// serving (optional)

### Key Entities

- **GcConfig**: Configuration for garbage collection (auto threshold, prune expiry)
- **SubmoduleConfig**: Parsed .gitmodules and submodule state
- **HookRunner**: Executes hook scripts at the right points

## Success Criteria

### Measurable Outcomes

- **SC-001**: `git gc` produces equivalent results to C git (same pack structure)
- **SC-002**: `git fsck` detects all corruption types that C git detects
- **SC-003**: Submodules work identically to C git for all operations
- **SC-004**: Archives are byte-identical to C git's output
- **SC-005**: All standard hooks are executed at the correct points
- **SC-006**: GPG signing/verification interoperates with C git

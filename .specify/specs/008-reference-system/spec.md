# Feature Specification: Reference System

**Feature Branch**: `008-reference-system`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity

## User Scenarios & Testing

### User Story 1 - Resolve References (Priority: P1)

As a gitr library consumer, I need to resolve branch, tag, and symbolic references to their target OIDs so that commands like `git log main` and `git checkout feature` work.

**Why this priority**: Ref resolution is the entry point for almost every git operation.

**Independent Test**: Create branches and tags with C git, resolve them with gitr, verify OIDs match.

**Acceptance Scenarios**:

1. **Given** a branch ref `refs/heads/main`, **When** resolved, **Then** the commit OID it points to is returned.
2. **Given** the symbolic ref `HEAD` pointing to `refs/heads/main`, **When** resolved, **Then** the commit OID of main is returned.
3. **Given** `HEAD` in detached state, **When** resolved, **Then** the direct OID is returned.
4. **Given** a nonexistent ref, **When** resolved, **Then** `None` is returned.
5. **Given** a symbolic ref chain (A → B → C → OID), **When** resolved, **Then** the final OID is returned.

---

### User Story 2 - Update References (Priority: P1)

As a gitr library, I need to create, update, and delete references atomically so that branch and tag operations are safe.

**Why this priority**: Every commit, branch, merge, and tag operation updates refs. Must be atomic.

**Independent Test**: Update a ref with gitr, verify with C git, then update with C git and verify with gitr.

**Acceptance Scenarios**:

1. **Given** a new branch name, **When** created, **Then** a ref file or reftable entry is written atomically.
2. **Given** an existing ref, **When** updated with the correct old value, **Then** the update succeeds atomically.
3. **Given** an existing ref, **When** updated with an incorrect old value (CAS failure), **Then** the update is rejected.
4. **Given** a ref to delete, **When** deleted, **Then** the ref is removed and no longer resolvable.
5. **Given** multiple refs to update, **When** updated as a transaction, **Then** all succeed or all fail (atomicity).

---

### User Story 3 - Enumerate References (Priority: P1)

As a gitr library, I need to iterate over references with prefix filtering so that `git branch`, `git tag`, and `for-each-ref` work.

**Why this priority**: Many commands list refs. Enumeration is a core operation.

**Independent Test**: Create several branches and tags, enumerate with prefix filter, verify all expected refs are returned.

**Acceptance Scenarios**:

1. **Given** refs/heads/ prefix, **When** iterating, **Then** all local branches are returned sorted.
2. **Given** refs/tags/ prefix, **When** iterating, **Then** all tags are returned sorted.
3. **Given** refs/remotes/origin/ prefix, **When** iterating, **Then** all remote-tracking branches for origin are returned.
4. **Given** no prefix filter, **When** iterating, **Then** all refs are returned sorted.

---

### User Story 4 - Reflog (Priority: P2)

As a git user, I need reflog support so that recent ref changes are recorded and recoverable via `git reflog`.

**Why this priority**: Reflogs are essential for recovering from mistakes (force push, reset, etc.).

**Independent Test**: Make several commits, verify reflog entries are created and match C git's reflog.

**Acceptance Scenarios**:

1. **Given** a ref update, **When** reflog is enabled, **Then** a reflog entry is appended with old OID, new OID, identity, timestamp, and message.
2. **Given** a branch with reflog, **When** `@{N}` is used, **Then** the Nth previous value is returned.
3. **Given** a reflog, **When** entries are read, **Then** they match C git's `git reflog show`.
4. **Given** reflog expiry, **When** expired entries are pruned, **Then** old entries are removed per configured retention.

---

### User Story 5 - Pluggable Backends (Priority: P2)

As a gitr library designer, I need the ref storage to be trait-based so that files-backend, packed-refs, and reftable can be used interchangeably.

**Why this priority**: Git supports multiple ref backends. The architecture must accommodate this.

**Independent Test**: Run the same ref operations against both files-backend and a mock backend, verify identical behavior.

**Acceptance Scenarios**:

1. **Given** a repository with files backend, **When** refs are read/written, **Then** the files backend is used.
2. **Given** a repository with reftable backend, **When** refs are read/written, **Then** the reftable backend is used.
3. **Given** any backend, **When** the same operations are performed, **Then** the results are identical.

---

### User Story 6 - Packed Refs (Priority: P1)

As a gitr library, I need to read and write the packed-refs file for efficient ref storage.

**Why this priority**: Repositories with many tags use packed-refs. The files backend depends on it.

**Acceptance Scenarios**:

1. **Given** a packed-refs file, **When** refs are looked up, **Then** packed refs are found.
2. **Given** both a loose ref and a packed ref for the same name, **When** resolved, **Then** the loose ref takes precedence.
3. **Given** a ref to pack, **When** packed, **Then** it's added to packed-refs and the loose file is deleted.

### Edge Cases

- Symbolic ref pointing to nonexistent target (dangling symref)
- Ref name containing special characters (refs are restricted but some edge cases exist)
- Invalid ref names (rejected per git-check-ref-format rules)
- Concurrent ref updates from multiple processes
- Packed-refs lock contention during gc
- HEAD pointing to an unborn branch (new repo, no commits yet)
- Very long ref names (path length limits)
- Ref starting with refs/heads/HEAD (valid but confusing)

## Requirements

### Functional Requirements

- **FR-001**: System MUST resolve direct refs (name → OID) and symbolic refs (name → name → ... → OID)
- **FR-002**: System MUST support atomic ref updates with compare-and-swap (old_value check)
- **FR-003**: System MUST support ref transactions (batch atomic updates)
- **FR-004**: System MUST enumerate refs with optional prefix filter, sorted lexicographically
- **FR-005**: System MUST maintain reflogs for ref updates when configured
- **FR-006**: System MUST validate ref names per git-check-ref-format rules
- **FR-007**: System MUST support the files backend (loose refs + packed-refs)
- **FR-008**: System MUST read and write the packed-refs file format
- **FR-009**: System MUST support special refs: HEAD, MERGE_HEAD, CHERRY_PICK_HEAD, REVERT_HEAD, BISECT_HEAD, ORIG_HEAD, FETCH_HEAD
- **FR-010**: System MUST use lock files for atomic ref file updates
- **FR-011**: System MUST define a `RefStore` trait for pluggable backends
- **FR-012**: System MUST support peeled refs in packed-refs (^OID notation for tag targets)

### Key Entities

- **Reference**: A named pointer to an OID or another ref
- **SymbolicRef**: A ref that points to another ref name
- **RefTransaction**: Atomic batch of ref updates
- **ReflogEntry**: Record of a ref value change
- **RefStore**: Trait for ref storage backends

## Success Criteria

### Measurable Outcomes

- **SC-001**: All ref operations produce results identical to C git
- **SC-002**: Ref transactions are truly atomic (verified with concurrent stress test)
- **SC-003**: Reflog entries match C git's format byte-for-byte
- **SC-004**: Ref enumeration returns all refs in correct sorted order
- **SC-005**: Packed-refs performance: lookup < 1μs with binary search

# Feature Specification: Object Database

**Feature Branch**: `006-object-database`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity, 003-object-model, 004-loose-object-storage, 005-packfile-system

## User Scenarios & Testing

### User Story 1 - Unified Object Access (Priority: P1)

As a gitr library consumer, I need a single interface to read objects regardless of whether they are stored loose or packed, so that callers don't need to know about storage details.

**Why this priority**: This is the primary abstraction that all higher-level code uses to access objects.

**Independent Test**: Create objects (some loose, some packed), read them all through the unified ODB.

**Acceptance Scenarios**:

1. **Given** a loose object, **When** read through the ODB, **Then** the correct object is returned.
2. **Given** a packed object, **When** read through the ODB, **Then** the delta is resolved and the correct object is returned.
3. **Given** an OID, **When** the object exists in both loose and packed storage, **Then** the loose version is preferred (it may be newer).
4. **Given** an OID that doesn't exist anywhere, **When** read, **Then** `None` is returned.
5. **Given** multiple packfiles, **When** reading an object, **Then** all packs are searched.

---

### User Story 2 - Object Writing (Priority: P1)

As a gitr library, I need to write objects through the ODB so that new objects are created in the correct storage location.

**Why this priority**: Every operation that creates content (add, commit, merge) writes through the ODB.

**Independent Test**: Write objects through the ODB, verify they're readable by both gitr and C git.

**Acceptance Scenarios**:

1. **Given** a new object, **When** written through ODB, **Then** it is created as a loose object.
2. **Given** a new object, **When** written, **Then** the returned OID matches the object's hash.
3. **Given** an object that already exists, **When** written again, **Then** the write is idempotent.

---

### User Story 3 - Alternates (Priority: P2)

As a git user, I need alternates support so that repositories can share object storage (e.g., for forks or worktrees).

**Why this priority**: Used by `git clone --reference`, worktrees, and shared repository setups.

**Independent Test**: Set up an alternates file pointing to another repo's objects/, verify objects from both repos are accessible.

**Acceptance Scenarios**:

1. **Given** `.git/objects/info/alternates` with a path, **When** reading objects, **Then** the alternate's objects are also searched.
2. **Given** nested alternates (A → B → C), **When** searching, **Then** all levels are searched.
3. **Given** a circular alternates chain, **When** detected, **Then** an error is returned.

---

### User Story 4 - Object Existence Checks (Priority: P1)

As a gitr library, I need fast object existence checks for operations like fetch negotiation and connectivity checks.

**Why this priority**: Many operations only need to know if an object exists, not read its content.

**Independent Test**: Check existence of 10K OIDs (mix of existing and non-existing) and measure performance.

**Acceptance Scenarios**:

1. **Given** an OID of a loose object, **When** checked for existence, **Then** true is returned without reading the content.
2. **Given** an OID of a packed object, **When** checked, **Then** pack indexes are consulted without decompression.
3. **Given** an OID that doesn't exist, **When** checked, **Then** false is returned after checking all sources.

---

### User Story 5 - Header-Only Reads (Priority: P2)

As a gitr library, I need to read just the object type and size for operations that don't need full content.

**Why this priority**: Operations like `ls-tree` or type checking benefit from avoiding full decompression.

**Acceptance Scenarios**:

1. **Given** an OID, **When** reading header only, **Then** type and size are returned without decompressing content.

### Edge Cases

- Object in a corrupt packfile (fall back to other sources)
- Alternates path that doesn't exist (skip with warning)
- Race condition: object being written while being read
- Object in multiple packs (any valid copy suffices)
- MIDX present alongside individual pack indexes
- Empty repository (no objects at all)

## Requirements

### Functional Requirements

- **FR-001**: System MUST provide a unified `ObjectDatabase` interface over loose + packed storage
- **FR-002**: System MUST search loose objects first, then packfiles, then alternates
- **FR-003**: System MUST support writing new objects (always as loose initially)
- **FR-004**: System MUST support `.git/objects/info/alternates` for shared object storage
- **FR-005**: System MUST support fast existence checks without reading content
- **FR-006**: System MUST support header-only reads (type + size) for efficiency
- **FR-007**: System MUST handle multiple packfiles (search all, MIDX preferred if available)
- **FR-008**: System MUST provide streaming read for large objects
- **FR-009**: System MUST provide OID prefix resolution (short hex → full OID, error on ambiguity)
- **FR-010**: System MUST be thread-safe for concurrent reads (multiple threads reading objects)

### Key Entities

- **ObjectDatabase**: Unified interface to all object storage
- **ObjectInfo**: Lightweight type+size info without content
- **OdbBackend**: Trait for pluggable object storage backends

## Success Criteria

### Measurable Outcomes

- **SC-001**: All objects in a repository are accessible through the unified ODB
- **SC-002**: Existence checks are < 1μs for packed objects (index-only, no decompression)
- **SC-003**: Alternates chains resolve correctly up to arbitrary depth
- **SC-004**: Thread-safe reads verified with concurrent stress test (10 threads, 10K objects each)
- **SC-005**: OID prefix resolution matches C git behavior for all prefix lengths

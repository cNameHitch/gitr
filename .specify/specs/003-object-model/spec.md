# Feature Specification: Object Model

**Feature Branch**: `003-object-model`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity

## User Scenarios & Testing

### User Story 1 - Parse Git Objects (Priority: P1)

As a gitr library consumer, I need to parse the four git object types (blob, tree, commit, tag) from their raw byte representation so that object data can be inspected and manipulated.

**Why this priority**: Every git operation reads objects. Parsing is the most fundamental operation.

**Independent Test**: Read raw objects produced by C git, parse them, and verify all fields are extracted correctly.

**Acceptance Scenarios**:

1. **Given** a raw blob object (header + content), **When** parsed, **Then** the blob's content bytes are accessible.
2. **Given** a raw tree object, **When** parsed, **Then** all entries (mode, name, OID) are returned in order.
3. **Given** a raw commit object, **When** parsed, **Then** tree OID, parent OIDs, author, committer, and message are all correctly extracted.
4. **Given** a raw tag object, **When** parsed, **Then** target OID, target type, tag name, tagger, and message are extracted.
5. **Given** an object with an unknown type in the header, **When** parsed, **Then** a descriptive error is returned.

---

### User Story 2 - Serialize Git Objects (Priority: P1)

As a gitr library, I need to serialize git objects to their canonical byte representation so that objects can be stored and their OIDs computed.

**Why this priority**: Object storage and hash computation require canonical serialization.

**Independent Test**: Serialize an object, then parse it back and verify all fields match.

**Acceptance Scenarios**:

1. **Given** a Commit struct, **When** serialized, **Then** the output matches the byte-for-byte format C git produces.
2. **Given** a Tree with entries, **When** serialized, **Then** entries are sorted by git's tree sorting rules (directory names have trailing '/').
3. **Given** a Tag struct, **When** serialized, **Then** the output includes all required fields in the correct order.
4. **Given** any serialized object, **When** hashed, **Then** the OID matches C git's `git hash-object -t <type>`.

---

### User Story 3 - Object Type System (Priority: P1)

As a gitr developer, I need a type-safe enum for git object types so that type errors are caught at compile time.

**Why this priority**: Type safety prevents bugs where wrong object types are passed to functions expecting a specific type.

**Independent Test**: Attempt to extract tree entries from a blob and verify it fails at compile time (or returns a type error).

**Acceptance Scenarios**:

1. **Given** an Object enum, **When** pattern-matched, **Then** all four variants (Blob, Tree, Commit, Tag) are covered.
2. **Given** an object type string ("blob", "tree", "commit", "tag"), **When** parsed, **Then** the correct ObjectType enum variant is returned.
3. **Given** an ObjectType, **When** converted to string, **Then** it produces the canonical lowercase name.

---

### User Story 4 - Object Name Resolution (Priority: P2)

As a git user, I need to resolve object names (SHA prefixes, HEAD, refs, rev^, rev~N) to full OIDs so that the revision syntax works.

**Why this priority**: Users rarely type full SHA-1 hashes. Name resolution is essential for usability.

**Independent Test**: Resolve "HEAD", "HEAD~3", "abc1234" (prefix), and "v1.0^{commit}" against a test repository.

**Acceptance Scenarios**:

1. **Given** a full 40-character hex string, **When** resolved, **Then** the corresponding ObjectId is returned.
2. **Given** a short hex prefix (minimum 4 characters), **When** resolved, **Then** the unique matching OID is returned, or an ambiguity error.
3. **Given** "HEAD^", **When** resolved, **Then** the first parent of HEAD is returned.
4. **Given** "HEAD~3", **When** resolved, **Then** the third ancestor following first parents is returned.
5. **Given** "v1.0^{commit}", **When** resolved, **Then** the commit that the tag points to is returned (peeling).

---

### User Story 5 - Object Caching (Priority: P3)

As a gitr library, I need parsed object caching so that repeatedly accessed objects (e.g., root tree of HEAD) don't require re-parsing.

**Why this priority**: Performance optimization. Important for operations that traverse commit history.

**Independent Test**: Parse the same object twice and verify the second access uses the cache (no I/O).

**Acceptance Scenarios**:

1. **Given** an object parsed once, **When** requested again by OID, **Then** the cached version is returned without re-reading storage.
2. **Given** a cache with a size limit, **When** the limit is exceeded, **Then** least-recently-used entries are evicted.
3. **Given** a multi-threaded context, **When** objects are cached concurrently, **Then** no data races occur.

### Edge Cases

- Tree entry with mode 160000 (gitlink/submodule)
- Commit with zero parents (root commit)
- Commit with multiple parents (merge commit, octopus merge)
- Tag pointing to another tag (nested tags)
- Object with extra trailing newline in message
- Tree with entries that sort differently in C locale vs natural order
- Commit with non-UTF-8 encoding header
- Blob containing null bytes
- Empty tree (no entries)
- Commit message with no trailing newline

## Requirements

### Functional Requirements

- **FR-001**: System MUST define four object types: Blob, Tree, Commit, Tag as a Rust enum
- **FR-002**: System MUST parse object headers in format `"<type> <size>\0"` from raw bytes
- **FR-003**: System MUST parse tree entries as `<mode> <name>\0<20-byte-oid>` sequences
- **FR-004**: System MUST parse commits extracting: tree OID, parent OIDs (0+), author signature, committer signature, optional encoding, message body
- **FR-005**: System MUST parse tags extracting: target OID, target type, tag name, optional tagger signature, message body
- **FR-006**: System MUST serialize all object types to their canonical byte format, byte-identical to C git
- **FR-007**: System MUST sort tree entries using git's tree entry comparison (entries for trees sort as if name has trailing '/')
- **FR-008**: System MUST support object name resolution for: full hex, short hex prefix, HEAD, refs, rev^N, rev~N, rev^{type}
- **FR-009**: System MUST provide FileMode enum: Regular(644), Executable(755), Symlink(120000), Gitlink(160000), Tree(40000)
- **FR-010**: System MUST provide an LRU object cache with configurable size limit

### Key Entities

- **Object**: Enum of Blob, Tree, Commit, Tag
- **ObjectType**: Enum identifying object type without data
- **TreeEntry**: A single entry in a tree object (mode, name, OID)
- **Commit**: Parsed commit with tree, parents, author, committer, message
- **Tag**: Parsed annotated tag
- **FileMode**: Tree entry permission mode

## Success Criteria

### Measurable Outcomes

- **SC-001**: All object types parse correctly for a corpus of 10K+ real objects from the git.git repository
- **SC-002**: Serialization round-trip (parse → serialize → parse) preserves all fields exactly
- **SC-003**: Serialized objects hash to the same OID as C git's `git hash-object`
- **SC-004**: Tree sorting matches C git's sort order for all edge cases (names with special characters, directory vs file)
- **SC-005**: Object name resolution matches C git's `git rev-parse` for a test suite of 50+ revision expressions

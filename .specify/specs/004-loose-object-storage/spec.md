# Feature Specification: Loose Object Storage

**Feature Branch**: `004-loose-object-storage`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity, 003-object-model

## User Scenarios & Testing

### User Story 1 - Read Loose Objects (Priority: P1)

As a gitr library consumer, I need to read individual loose objects from `.git/objects/xx/xxxx...` so that recently created or unpacked objects can be accessed.

**Why this priority**: Reading loose objects is essential for any repository operation. New objects start as loose.

**Independent Test**: Create an object with C git, read it with gitr, verify content matches.

**Acceptance Scenarios**:

1. **Given** a loose object file at `.git/objects/ab/cdef1234...`, **When** read, **Then** the zlib-decompressed content matches the original object.
2. **Given** a valid OID, **When** the loose object exists, **Then** the object type and content are returned.
3. **Given** a valid OID, **When** the loose object does not exist, **Then** a "not found" result is returned (not an error).
4. **Given** a corrupt loose object (bad zlib data), **When** read, **Then** a descriptive error is returned.
5. **Given** a loose object, **When** only the header is needed, **Then** type and size can be read without decompressing the full content.

---

### User Story 2 - Write Loose Objects (Priority: P1)

As a gitr library, I need to write objects as loose files so that `git add`, `git commit`, and similar operations create new objects.

**Why this priority**: Object creation is as fundamental as reading. Every new blob, tree, commit starts as a loose object.

**Independent Test**: Write an object with gitr, read it with C git (`git cat-file`), verify content matches.

**Acceptance Scenarios**:

1. **Given** a blob with content, **When** written as a loose object, **Then** the file is created at the correct path and is valid zlib-compressed data.
2. **Given** an object to write, **When** the object already exists (same OID), **Then** the write is a no-op (idempotent).
3. **Given** an object to write, **When** writing, **Then** the file is written atomically (temp file + rename) to prevent corruption.
4. **Given** an object to write, **When** the OID is computed, **Then** it matches C git's `git hash-object` for identical content.

---

### User Story 3 - Enumerate Loose Objects (Priority: P2)

As a gitr library, I need to enumerate all loose objects for garbage collection and fsck operations.

**Why this priority**: Needed for gc, fsck, and repack operations.

**Independent Test**: Create several loose objects, enumerate them, verify all are found.

**Acceptance Scenarios**:

1. **Given** a repository with loose objects, **When** enumerated, **Then** all loose object OIDs are returned.
2. **Given** a repository with subdirectories 00-ff in objects/, **When** enumerated, **Then** objects from all fan-out directories are found.
3. **Given** a non-object file in the objects directory, **When** enumerating, **Then** it is skipped with a warning.

---

### User Story 4 - Streaming Large Objects (Priority: P2)

As a gitr library, I need to read large loose objects in a streaming fashion so that memory usage stays bounded for files larger than available RAM.

**Why this priority**: Repositories can contain very large binary files. Streaming prevents OOM.

**Independent Test**: Write a 1GB loose object, stream-read it, verify content without loading it all into memory.

**Acceptance Scenarios**:

1. **Given** a large loose object (>100MB), **When** read in streaming mode, **Then** peak memory usage stays below 10MB.
2. **Given** a streaming reader, **When** bytes are consumed, **Then** they are decompressed on demand.
3. **Given** a streaming reader, **When** only the first N bytes are needed, **Then** the rest is not decompressed.

### Edge Cases

- Loose object file with incorrect permissions (read-only filesystem)
- Loose object with size mismatch (header says X bytes, content is Y bytes)
- Empty blob (zero-length content, still has header)
- Object directory doesn't exist yet (first object in empty repo)
- Filesystem without atomic rename (FAT32)
- Loose object path collision on case-insensitive filesystem
- Object hash verification failure (content doesn't match filename OID)

## Requirements

### Functional Requirements

- **FR-001**: System MUST read loose objects from `.git/objects/XX/YYYY...` (first 2 hex chars as directory, rest as filename)
- **FR-002**: System MUST decompress loose objects using zlib (RFC 1950)
- **FR-003**: System MUST verify that the decompressed content matches the expected format: `"<type> <size>\0<content>"`
- **FR-004**: System MUST write loose objects atomically (write to temp file, then rename)
- **FR-005**: System MUST compress written objects with zlib at the configured compression level
- **FR-006**: System MUST skip writing if an object with the same OID already exists
- **FR-007**: System MUST support reading just the header (type + size) without full decompression
- **FR-008**: System MUST enumerate all loose objects by walking the objects/ directory
- **FR-009**: System MUST support streaming reads for large objects
- **FR-010**: System MUST verify object integrity (hash content on read, compare with expected OID)
- **FR-011**: System MUST create fan-out directories (00-ff) as needed when writing
- **FR-012**: System MUST respect `core.compression` and `core.looseCompression` config for compression level

### Key Entities

- **LooseObjectStore**: Interface to the loose object directory
- **LooseObject**: A single loose object with type, size, and content
- **ObjectStream**: Streaming reader for large loose objects

## Success Criteria

### Measurable Outcomes

- **SC-001**: All loose objects written by gitr are readable by C git and vice versa
- **SC-002**: Object hash verification catches all tampered objects (100% detection rate)
- **SC-003**: Streaming read of 1GB object uses < 10MB peak memory
- **SC-004**: Write performance within 10% of C git for single objects
- **SC-005**: Atomic writes prevent corruption under concurrent access (verified with stress test)

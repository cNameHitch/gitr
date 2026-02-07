# Feature Specification: Hash & Object Identity

**Feature Branch**: `002-hash-and-object-identity`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities

## User Scenarios & Testing

### User Story 1 - Object ID Representation (Priority: P1)

As a gitr library consumer, I need a type-safe Object ID (OID) type that represents SHA-1 (20 bytes) or SHA-256 (32 bytes) hashes so that all object references are correct by construction.

**Why this priority**: Every git object is identified by its hash. This type is used everywhere.

**Independent Test**: Create OIDs from hex strings, compare them, use as hash map keys, and verify Display/FromStr round-trip.

**Acceptance Scenarios**:

1. **Given** a 40-character hex string, **When** parsed as an OID, **Then** a valid SHA-1 ObjectId is created.
2. **Given** a 64-character hex string, **When** parsed as an OID, **Then** a valid SHA-256 ObjectId is created.
3. **Given** two identical hex strings, **When** parsed into OIDs, **Then** they compare as equal.
4. **Given** an OID, **When** displayed, **Then** the lowercase hex representation is produced.
5. **Given** an invalid hex string (odd length, non-hex chars), **When** parsed, **Then** an error is returned.

---

### User Story 2 - Hash Computation (Priority: P1)

As a gitr library, I need to compute SHA-1 and SHA-256 hashes of arbitrary data so that object IDs can be generated from content.

**Why this priority**: Object storage, verification, and indexing all require hash computation.

**Independent Test**: Hash known test vectors and verify output matches expected digests.

**Acceptance Scenarios**:

1. **Given** the empty string, **When** hashed with SHA-1, **Then** the result matches the known SHA-1 of empty string.
2. **Given** a git object header + content, **When** hashed, **Then** the result matches `git hash-object` output.
3. **Given** data fed in multiple chunks, **When** finalized, **Then** the hash matches single-chunk computation.
4. **Given** SHA-256 mode, **When** hashing the same content, **Then** the correct SHA-256 digest is produced.

---

### User Story 3 - Hex Encoding/Decoding (Priority: P1)

As a gitr library, I need fast hex encoding and decoding of OIDs for display, parsing, and loose object paths.

**Why this priority**: Hex conversion is used in object file paths, protocol exchange, and user display.

**Independent Test**: Encode/decode round-trip for all possible byte values.

**Acceptance Scenarios**:

1. **Given** a 20-byte array, **When** hex-encoded, **Then** a 40-character lowercase hex string is produced.
2. **Given** a hex string with uppercase letters, **When** decoded, **Then** it is accepted (case-insensitive decode).
3. **Given** a partial hex prefix, **When** used for lookup, **Then** it can match OIDs that start with that prefix.

---

### User Story 4 - OID Collections (Priority: P2)

As a gitr library, I need specialized collections for OIDs (sorted array, hash map, hash set) for efficient object lookups.

**Why this priority**: Pack indexes, object databases, and reachability checks need efficient OID collections.

**Independent Test**: Insert 10K OIDs, verify lookup performance, test sorted array binary search.

**Acceptance Scenarios**:

1. **Given** an OID array, **When** sorted, **Then** binary search finds any contained OID in O(log n).
2. **Given** an OID set, **When** checked for membership, **Then** lookup is O(1) average.
3. **Given** an OID map with values, **When** looked up by OID, **Then** the associated value is returned.
4. **Given** a sorted OID array and a hex prefix, **When** searched, **Then** all matching OIDs are returned.

---

### User Story 5 - Hash Algorithm Pluggability (Priority: P2)

As a gitr library, I need the hash algorithm to be pluggable so that SHA-256 repositories work alongside SHA-1 repositories.

**Why this priority**: Git supports SHA-256 as an alternative hash. The architecture must support both.

**Independent Test**: Create a repository context with SHA-256, hash objects, and verify all operations use the correct algorithm.

**Acceptance Scenarios**:

1. **Given** a SHA-1 repository, **When** objects are hashed, **Then** SHA-1 is used and OIDs are 20 bytes.
2. **Given** a SHA-256 repository, **When** objects are hashed, **Then** SHA-256 is used and OIDs are 32 bytes.
3. **Given** a hash algorithm choice, **When** creating a hasher, **Then** the correct algorithm is selected at runtime.

### Edge Cases

- OID of all zeros (null OID — used as sentinel)
- OID with all 'f' bytes (maximum value)
- Ambiguous short hex prefix that matches multiple objects
- Hash of empty content vs hash of empty git object (they differ due to header)
- Extremely large content hashing (>4GB — streaming hash required)
- SHA-1 collision detection (sha1dc)

## Requirements

### Functional Requirements

- **FR-001**: System MUST represent object IDs as fixed-size byte arrays (20 bytes for SHA-1, 32 bytes for SHA-256)
- **FR-002**: System MUST support creating OIDs from hex strings, byte arrays, and hash computation
- **FR-003**: System MUST provide hex encoding (to lowercase string) and decoding (case-insensitive)
- **FR-004**: System MUST provide a streaming hash interface (update with chunks, finalize to OID)
- **FR-005**: System MUST support SHA-1 (default) and SHA-256 hash algorithms
- **FR-006**: System MUST detect SHA-1 collisions using the sha1dc (SHA-1 DC) approach or equivalent
- **FR-007**: System MUST provide a null OID constant (all zeros) for each hash algorithm
- **FR-008**: System MUST provide OID prefix matching for abbreviated object names
- **FR-009**: System MUST provide OidArray (sorted, binary-searchable), OidMap, and OidSet collections
- **FR-010**: System MUST implement Display, FromStr, Hash, Eq, Ord, Serialize for ObjectId
- **FR-011**: System MUST provide fan-out table support for pack index lookups (first byte → range)

### Key Entities

- **ObjectId**: A hash digest identifying a git object (20 or 32 bytes)
- **HashAlgorithm**: Enum of supported algorithms (SHA-1, SHA-256)
- **Hasher**: Streaming hash computation context
- **OidArray**: Sorted collection of OIDs with binary search
- **OidMap<V>**: Hash map keyed by OID
- **OidSet**: Hash set of OIDs

## Success Criteria

### Measurable Outcomes

- **SC-001**: ObjectId hex round-trip is correct for all 2^160 possible SHA-1 values (verified via property tests)
- **SC-002**: Hash computation matches C git's `git hash-object` for a test corpus of 1000+ objects
- **SC-003**: OID collection operations meet performance targets: lookup < 100ns for OidSet, < 1μs for OidArray binary search
- **SC-004**: SHA-1 collision detection correctly identifies the known SHAttered collision
- **SC-005**: All operations work correctly for both SHA-1 and SHA-256 modes

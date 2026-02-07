# Feature Specification: Packfile System

**Feature Branch**: `005-packfile-system`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity, 003-object-model

## User Scenarios & Testing

### User Story 1 - Read Objects from Packfiles (Priority: P1)

As a gitr library, I need to read objects from packfiles so that repositories with packed objects (the majority of objects after gc) can be accessed.

**Why this priority**: Most objects in a real repository live in packfiles. This is essential for reading any non-trivial repo.

**Independent Test**: Run `git gc` on a test repo, then read objects from the resulting packfile with gitr.

**Acceptance Scenarios**:

1. **Given** a pack index file (.idx), **When** looking up an OID, **Then** the offset of the object in the .pack file is returned.
2. **Given** a base object in a packfile, **When** read, **Then** the decompressed content matches the original object.
3. **Given** a deltified object (OFS_DELTA or REF_DELTA), **When** read, **Then** the delta is applied to the base to reconstruct the full object.
4. **Given** a chain of deltas (A → B → C), **When** reading C, **Then** all deltas are resolved to produce the correct final object.
5. **Given** a packfile and its index, **When** iterating all objects, **Then** every object is accessible and valid.

---

### User Story 2 - Pack Index Lookup (Priority: P1)

As a gitr library, I need fast OID-to-offset lookup using pack index files so that object reads are efficient.

**Why this priority**: Without index lookup, finding an object in a pack requires linear scan. The index makes it O(log n).

**Independent Test**: Look up 1000 known OIDs in a pack index and verify all return correct offsets.

**Acceptance Scenarios**:

1. **Given** a v2 pack index, **When** looking up an OID, **Then** the correct offset is returned using fan-out + binary search.
2. **Given** a large packfile (>2GB), **When** using a v2 index with 64-bit offset table, **Then** offsets beyond 2GB are handled correctly.
3. **Given** an OID not in the pack, **When** looked up, **Then** None is returned.
4. **Given** an OID prefix, **When** searched, **Then** all matching OIDs in the pack are returned.

---

### User Story 3 - Delta Encoding/Decoding (Priority: P1)

As a gitr library, I need to apply and create delta instructions so that deltified objects in packs can be read and new packs can be created.

**Why this priority**: Deltas are what make packfiles compact. Most objects in a pack are stored as deltas.

**Independent Test**: Apply delta instructions to a known base and verify the result matches the expected output.

**Acceptance Scenarios**:

1. **Given** a base object and a delta instruction stream, **When** the delta is applied, **Then** the result matches the expected target object byte-for-byte.
2. **Given** copy instructions in a delta, **When** applied, **Then** bytes are copied from the correct offset and length in the base.
3. **Given** insert instructions in a delta, **When** applied, **Then** the literal bytes are inserted into the output.
4. **Given** a source and target object, **When** a delta is computed, **Then** applying it to the source produces the target.

---

### User Story 4 - Pack Generation (Priority: P2)

As a gitr library, I need to create packfiles for push operations and garbage collection.

**Why this priority**: Required for `git push`, `git gc`, and `git repack`.

**Independent Test**: Generate a pack from a set of objects, then verify C git can read all objects from it.

**Acceptance Scenarios**:

1. **Given** a set of objects, **When** packed, **Then** a valid .pack and .idx file are created.
2. **Given** similar objects, **When** packed with delta compression, **Then** the pack is significantly smaller than loose storage.
3. **Given** a generated pack, **When** verified with `git verify-pack`, **Then** no errors are reported.
4. **Given** a set of objects to push and a list of OIDs the remote already has, **When** a thin pack is generated, **Then** delta bases reference objects not in the pack (only those known to exist on the remote).
5. **Given** a thin pack, **When** received by C git's `receive-pack`, **Then** it is accepted and indexed correctly.
6. **Given** an empty set of objects to send (remote is up to date), **When** pack generation is requested, **Then** no pack is created and the operation succeeds gracefully.

---

### User Story 5 - Multi-Pack Index (Priority: P3)

As a gitr library, I need to support multi-pack indexes (MIDX) for repositories with many packfiles.

**Why this priority**: Large repositories (monorepos) use MIDX for performance. Not needed for basic functionality.

**Independent Test**: Create a MIDX from multiple packs, verify object lookup works across all packs.

**Acceptance Scenarios**:

1. **Given** multiple packfiles and a MIDX, **When** looking up an OID, **Then** the correct pack and offset are returned.
2. **Given** a MIDX, **When** iterating all objects, **Then** every object across all packs is returned.

---

### User Story 6 - Bitmap Index (Priority: P3)

As a gitr library, I need to support pack bitmap indexes for fast reachability queries.

**Why this priority**: Bitmaps dramatically speed up `git fetch` and `git clone` for large repos.

**Acceptance Scenarios**:

1. **Given** a bitmap index, **When** querying reachability from a commit, **Then** all reachable objects are identified without walking the graph.

### Edge Cases

- Packfile larger than 4GB (64-bit offsets required)
- Delta chain depth exceeding configured maximum
- Thin pack during push (base objects not included — remote is expected to have them)
- Thin pack during fetch (base objects not included — client is expected to have them)
- Corrupt delta instructions (source offset out of bounds)
- Pack with zero objects (degenerate case)
- Object that appears in multiple packs (prefer most recent pack)
- Pack checksum verification failure

## Requirements

### Functional Requirements

- **FR-001**: System MUST read pack format v2 (the standard) and v3 files
- **FR-002**: System MUST read pack index v2 format (fan-out table, sorted OIDs, CRC32, offsets)
- **FR-003**: System MUST resolve OFS_DELTA objects (offset-based delta)
- **FR-004**: System MUST resolve REF_DELTA objects (OID-based delta)
- **FR-005**: System MUST handle delta chains of arbitrary depth
- **FR-006**: System MUST apply delta instructions: copy-from-base and insert-literal
- **FR-007**: System MUST compute deltas between similar objects for pack creation
- **FR-008**: System MUST generate valid .pack and .idx files
- **FR-015**: System MUST support thin pack generation where delta bases reference objects not included in the pack (used by push and fetch to reduce transfer size)
- **FR-016**: System MUST accept a set of "known remote OIDs" to determine which objects can serve as external delta bases in thin packs
- **FR-009**: System MUST support 64-bit offsets for packs > 2GB
- **FR-010**: System MUST support memory-mapped packfile access for performance
- **FR-011**: System MUST support multi-pack index (MIDX) format for multi-pack lookup
- **FR-012**: System MUST support bitmap index for fast reachability queries
- **FR-013**: System MUST support pack reverse index (offset → OID mapping)
- **FR-014**: System MUST verify pack checksums (trailing SHA-1/SHA-256 of pack content)

### Key Entities

- **PackFile**: A single .pack file with its .idx index
- **PackIndex**: The .idx file providing OID → offset mapping
- **DeltaInstruction**: Copy or insert operation in a delta stream
- **MultiPackIndex**: MIDX file spanning multiple packs
- **BitmapIndex**: Reachability bitmap for fast traversal

## Success Criteria

### Measurable Outcomes

- **SC-001**: All objects in a `git gc`-packed repository are readable by gitr
- **SC-002**: Delta application produces byte-identical results to C git
- **SC-003**: Pack files generated by gitr pass `git verify-pack -v`
- **SC-004**: Pack index lookup is O(log n) via fan-out + binary search
- **SC-005**: Memory-mapped pack access allows reading from packs larger than available RAM
- **SC-006**: Delta compression ratio within 5% of C git's `git repack -a -d`

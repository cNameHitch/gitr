# Feature Specification: Transport Protocol

**Feature Branch**: `014-transport-protocol`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities, 002-hash-and-object-identity, 005-packfile-system, 006-object-database, 008-reference-system, 010-repository-and-setup

## User Scenarios & Testing

### User Story 1 - Fetch Objects (Priority: P1)

As a git user, I need to fetch objects from remote repositories so that `git fetch` and `git pull` work.

**Why this priority**: Fetching is essential for distributed workflow.

**Independent Test**: Push commits to a test server, fetch them with gitr, verify all objects arrive correctly.

**Acceptance Scenarios**:

1. **Given** a remote with new commits, **When** fetching, **Then** all reachable objects are downloaded and refs are updated.
2. **Given** protocol v2, **When** fetching, **Then** capability negotiation uses the v2 protocol.
3. **Given** a partial clone filter, **When** fetching with `--filter=blob:none`, **Then** blobs are excluded from the pack.
4. **Given** no new commits, **When** fetching, **Then** the operation completes quickly without downloading data.

---

### User Story 2 - Push Objects (Priority: P1)

As a git user, I need to push objects to remote repositories so that `git push` works.

**Why this priority**: Pushing is essential for sharing work.

**Independent Test**: Create local commits, push to a C git `receive-pack` server, verify refs are updated and objects are present.

**Acceptance Scenarios**:

1. **Given** local commits not on remote, **When** pushing, **Then** a thin pack is generated (using remote's advertised refs as "haves"), sent via the transport, and remote refs are updated.
2. **Given** a non-fast-forward push, **When** pushing without `--force`, **Then** the push is rejected with a clear error.
3. **Given** `--force`, **When** pushing, **Then** the remote ref is updated regardless.
4. **Given** push with delete (`git push --delete`), **When** pushing, **Then** the remote ref is removed.
5. **Given** `--force-with-lease`, **When** pushing, **Then** the push succeeds only if the remote ref matches the expected old value (compare-and-swap).
6. **Given** `--atomic`, **When** pushing multiple refs, **Then** either all ref updates succeed or none are applied.
7. **Given** a push with `--set-upstream`, **When** pushing, **Then** the upstream tracking branch is configured for the pushed branch.
8. **Given** the remote is up to date, **When** pushing, **Then** "Everything up-to-date" is displayed and no pack is sent.

**Push Protocol Flow (send-pack ↔ receive-pack)**:

1. Client connects to remote's `receive-pack` service
2. Server advertises its refs and capabilities (including `report-status`, `delete-refs`, `atomic`, `push-options`, `ofs-delta`)
3. Client compares local refs to remote's advertised refs to determine what to push
4. Client sends ref update commands: `<old-oid> <new-oid> <refname>` for each ref to update (old-oid = `0{40}` for create, new-oid = `0{40}` for delete)
5. Client sends flush packet to end ref update list
6. Client generates a thin pack of objects reachable from new OIDs but not from the remote's advertised OIDs, and streams it to the server
7. Server unpacks/indexes the pack and processes ref updates
8. Server sends status report (`report-status` capability): `unpack ok\n` followed by `ok <refname>\n` or `ng <refname> <reason>\n` for each ref
9. Client parses the status report and reports success/failure per ref

---

### User Story 3 - SSH Transport (Priority: P1)

As a git user, I need SSH transport so that I can push/fetch over SSH.

**Why this priority**: SSH is the most common authenticated transport.

**Acceptance Scenarios**:

1. **Given** an SSH URL (`git@github.com:user/repo.git`), **When** connecting, **Then** the SSH transport is used.
2. **Given** a custom SSH command (`GIT_SSH_COMMAND`), **When** connecting, **Then** the custom command is used.
3. **Given** SSH key authentication, **When** connecting, **Then** the key is used for authentication.

---

### User Story 4 - HTTP(S) Smart Transport (Priority: P1)

As a git user, I need HTTP/HTTPS transport so that I can push/fetch over HTTP.

**Why this priority**: HTTPS is the most common transport for public repositories.

**Acceptance Scenarios**:

1. **Given** an HTTPS URL, **When** connecting, **Then** the smart HTTP protocol is used.
2. **Given** authentication required, **When** connecting, **Then** credential helpers are consulted.
3. **Given** HTTP redirects, **When** following, **Then** the redirect is followed safely.

---

### User Story 5 - Pkt-Line Protocol (Priority: P1)

As a gitr library, I need the pkt-line framing protocol for all git wire communication.

**Why this priority**: Pkt-line is the foundation of the git wire protocol.

**Acceptance Scenarios**:

1. **Given** data to send, **When** encoded as pkt-line, **Then** each line is prefixed with a 4-hex-digit length.
2. **Given** a flush packet (`0000`), **When** read, **Then** it signals the end of a section.
3. **Given** pkt-line data, **When** decoded, **Then** the length prefix is removed and data content is returned.

---

### User Story 6 - Protocol v2 (Priority: P2)

As a gitr library, I need protocol v2 support for efficient capability negotiation and server-side filtering.

**Why this priority**: Protocol v2 is more efficient and supports features like partial clone.

**Acceptance Scenarios**:

1. **Given** a v2-capable server, **When** connecting, **Then** protocol v2 is negotiated.
2. **Given** v2, **When** listing refs, **Then** only requested ref prefixes are returned (server-side filtering).
3. **Given** v2 fetch, **When** fetching, **Then** server-side filtering and incremental negotiation work.

### Edge Cases

- Server timeout during transfer
- Interrupted transfer (resume not supported in git protocol)
- Authentication failure with retry
- Very large pack (>4GB) transfer
- Shallow fetch and deepen
- Bundle file transport (offline transfer)
- Sideband multiplexing (progress on band 2, errors on band 3)
- DNS resolution failure
- TLS certificate verification failure

## Requirements

### Functional Requirements

- **FR-001**: System MUST implement the git wire protocol (pkt-line framing)
- **FR-002**: System MUST support protocol v0/v1 and v2
- **FR-003**: System MUST implement fetch-pack (client-side fetch negotiation)
- **FR-004**: System MUST implement send-pack (client-side push) including ref update commands, thin pack generation, and status report parsing
- **FR-015**: System MUST support push capabilities: `report-status`, `report-status-v2`, `delete-refs`, `atomic`, `ofs-delta`, `push-options`
- **FR-016**: System MUST support `--force-with-lease` by comparing remote's advertised ref OID against the expected value before sending updates
- **FR-017**: System MUST support atomic push (`--atomic` capability) — all ref updates succeed or all are rejected
- **FR-005**: System MUST support SSH transport (spawn ssh process)
- **FR-006**: System MUST support HTTP/HTTPS smart transport
- **FR-007**: System MUST support local transport (direct file access)
- **FR-008**: System MUST implement capability negotiation
- **FR-009**: System MUST implement sideband multiplexing for progress and error reporting
- **FR-010**: System MUST support credential helpers for authentication
- **FR-011**: System MUST support remote configuration (remote.*.url, remote.*.fetch)
- **FR-012**: System MUST support bundle files for offline transfer
- **FR-013**: System MUST support shallow fetch (--depth, --deepen, --unshallow)
- **FR-014**: System MUST validate received pack data (checksum, object integrity)

### Key Entities

- **Transport**: Trait for different transport mechanisms (SSH, HTTP, local)
- **PktLineReader/Writer**: Pkt-line protocol framing
- **FetchNegotiation**: Have/want exchange for fetches
- **RemoteConfig**: Parsed remote configuration
- **CredentialHelper**: Authentication credential provider

## Success Criteria

### Measurable Outcomes

- **SC-001**: Fetch from C git server and push to C git server both work correctly
- **SC-002**: Protocol v2 negotiation works with GitHub, GitLab, and Bitbucket
- **SC-003**: SSH transport works with OpenSSH and PuTTY
- **SC-004**: HTTP transport handles authentication, redirects, and proxies
- **SC-005**: Transfer throughput within 10% of C git for large repositories
- **SC-006**: Sideband progress display matches C git output

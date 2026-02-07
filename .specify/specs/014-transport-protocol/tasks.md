# Tasks: Transport Protocol

**Input**: Design documents from `specs/014-transport-protocol/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Phase 1: Setup

- [X] T001 Create `crates/git-transport/Cargo.toml` with deps: git-utils, thiserror, reqwest (optional)
- [X] T002 Create `crates/git-protocol/Cargo.toml` with deps: git-utils, git-hash, git-pack, git-odb, git-ref, git-repository, bstr, thiserror
- [X] T003 [P] Create `crates/git-transport/src/lib.rs` with Transport trait, GitUrl, Scheme, TransportError
- [X] T004 [P] Create `crates/git-protocol/src/lib.rs` with ProtocolError types

**Checkpoint**: Both crates compile

---

## Phase 2: User Story 5 - Pkt-Line (Priority: P1)

**Goal**: Wire protocol framing

- [X] T005 [US5] Implement PktLineReader in `crates/git-protocol/src/pktline.rs` — read_line, read_until_flush
- [X] T006 [US5] Implement PktLineWriter — write_line, write_flush, write_delimiter
- [X] T007 [US5] Implement sideband demux in `crates/git-protocol/src/sideband.rs`
- [X] T008 [US5] Add pkt-line tests in `crates/git-protocol/tests/pktline_tests.rs`

**Checkpoint**: Pkt-line encoding/decoding correct for all packet types

---

## Phase 3: URL Parsing and Transport Setup

- [X] T009 Implement GitUrl parsing in `crates/git-transport/src/url.rs` — all schemes including SCP-like SSH
- [X] T010 Implement `connect` function dispatching by scheme

**Checkpoint**: URLs parse correctly, transport dispatch works

---

## Phase 4: User Story 3 - SSH Transport (Priority: P1)

- [X] T011 [US3] Implement SSH transport in `crates/git-transport/src/ssh.rs` — spawn ssh process, connect stdin/stdout
- [X] T012 [US3] Implement GIT_SSH_COMMAND and ssh.command config support
- [X] T013 [US3] Add SSH transport tests

**Checkpoint**: SSH transport connects and exchanges data

---

## Phase 5: User Story 4 - HTTP Transport (Priority: P1)

- [X] T014 [US4] Implement HTTP transport in `crates/git-transport/src/http.rs` — smart HTTP protocol
- [X] T015 [US4] Implement credential helper integration in `crates/git-transport/src/credential.rs`
- [X] T016 [US4] Add HTTP transport tests

**Checkpoint**: HTTPS transport works with authentication

---

## Phase 6: User Story 1 - Fetch (Priority: P1)

**Goal**: Complete fetch implementation

- [X] T017 [US1] Implement capability parsing in `crates/git-protocol/src/capability.rs`
- [X] T018 [US1] Implement v1 fetch negotiation in `crates/git-protocol/src/v1.rs` — want/have/ACK exchange
- [X] T019 [US1] Implement fetch function in `crates/git-protocol/src/fetch.rs` — negotiate, receive pack, index pack
- [X] T020 [US1] Implement remote config parsing in `crates/git-protocol/src/remote.rs` — RefSpec parsing
- [X] T021 [US1] Add fetch integration tests in `crates/git-protocol/tests/fetch_tests.rs`

**Checkpoint**: Fetch from C git server works

---

## Phase 7: User Story 2 - Push (Priority: P1)

**Goal**: Complete send-pack implementation for pushing to remote repositories

- [X] T022 [US2] Implement ref update command generation in `crates/git-protocol/src/push.rs` — compare local refs against remote's advertised refs, emit `<old-oid> <new-oid> <refname>` lines
- [X] T022a [US2] Implement push capability negotiation in `crates/git-protocol/src/push.rs` — negotiate `report-status`, `delete-refs`, `atomic`, `ofs-delta`, `push-options`
- [X] T022b [US2] Implement `compute_push_objects` — walk reachable objects from local OIDs, exclude objects reachable from remote's advertised OIDs
- [X] T022c [US2] Integrate with `generate_pack` (spec 005) for thin pack streaming — pipe pack data through transport writer with sideband if supported
- [X] T022d [US2] Implement status report parsing — parse `unpack ok/ng` and per-ref `ok <refname>` / `ng <refname> <reason>` lines
- [X] T022e [US2] Implement `--force-with-lease` support — compare expected OID against remote's advertised ref OID before sending update
- [X] T022f [US2] Implement atomic push — send `atomic` capability, handle all-or-nothing ref update semantics
- [X] T022g [US2] Implement `--push-option` support — send push-option lines between flush and pack data when server advertises `push-options` capability
- [X] T023 [US2] Add push integration tests in `crates/git-protocol/tests/push_tests.rs` — test fast-forward push, force push, delete, force-with-lease, atomic, rejection handling, and empty push (up-to-date)

**Checkpoint**: Push to C git server works for all ref update scenarios

---

## Phase 8: User Story 6 - Protocol v2 (Priority: P2)

- [X] T024 [US6] Implement v2 handshake and capability negotiation in `crates/git-protocol/src/v2.rs`
- [X] T025 [US6] Implement v2 ls-refs command
- [X] T026 [US6] Implement v2 fetch command with server-side filtering
- [X] T027 [US6] Add v2 tests

**Checkpoint**: Protocol v2 works with GitHub/GitLab

---

## Phase 9: Bundles and Polish

- [X] T028 Implement bundle file reading/writing in `crates/git-protocol/src/bundle.rs`
- [X] T029 Implement local transport in `crates/git-transport/src/local.rs`
- [X] T030 [P] Run `cargo clippy` on both crates and fix warnings
- [X] T031 Run `cargo test` on both crates — all tests pass

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 (pkt-line first, everything uses it)
- Phase 3 depends on Phase 1 (URL parsing for connect)
- Phases 4 and 5 depend on Phase 3 (transport implementations)
- Phase 6 depends on Phases 2+4+5 (fetch uses pkt-line + transport)
- Phase 7 depends on Phase 6 (push reuses fetch infrastructure)
- Phase 8 depends on Phase 6 (v2 extends v1 patterns)
- T011, T014 can run in parallel (different transport implementations)

### Cross-Spec Dependencies

- Spec 016 (porcelain) depends on: fetch, push for clone/fetch/pull/push commands
- Spec 018 (advanced) depends on: bundle for bundle command

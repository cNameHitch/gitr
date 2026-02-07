# Implementation Plan: Transport Protocol

**Branch**: `014-transport-protocol` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/014-transport-protocol/spec.md`

## Summary

Implement `git-transport` and `git-protocol` crates for network communication. `git-transport` handles the physical transport (SSH, HTTP, local), while `git-protocol` handles the wire protocol (pkt-line, capability negotiation, fetch/push exchange). Together they enable git fetch, push, clone, and ls-remote operations.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-pack`, `git-odb`, `git-ref`, `tokio` (optional, for HTTP), `reqwest` (HTTP), `thiserror`
**Storage**: N/A (network communication)
**Testing**: `cargo test`, integration tests against a local git daemon and test HTTP server
**Target Platform**: Linux, macOS, Windows
**Project Type**: Two library crates
**Performance Goals**: Transfer throughput > 50 MB/s (limited by network, not CPU)
**Constraints**: Must interoperate with C git servers. Protocol v2 preferred.
**Scale/Scope**: ~10 C files (~8K lines) → ~5K lines Rust across two crates

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | TLS verification, pack validation after receive |
| C-Compatibility | ✅ Pass | Interop tested against C git daemon/http-backend |
| Modular Crates | ✅ Pass | `git-transport` (physical) + `git-protocol` (wire) |
| Trait-Based | ✅ Pass | Transport trait for SSH/HTTP/local |
| Test-Driven | ✅ Pass | Integration tests with real git servers |

## Project Structure

### Source Code

```text
crates/git-transport/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Transport trait, URL parsing
│   ├── ssh.rs          # SSH transport (subprocess)
│   ├── http.rs         # HTTP/HTTPS transport
│   ├── local.rs        # Local file transport
│   ├── url.rs          # Git URL parsing
│   └── credential.rs   # Credential helper interface

crates/git-protocol/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Public API
│   ├── pktline.rs      # Pkt-line reader/writer
│   ├── capability.rs   # Capability negotiation
│   ├── v1.rs           # Protocol v1
│   ├── v2.rs           # Protocol v2
│   ├── fetch.rs        # Fetch negotiation (have/want)
│   ├── push.rs         # Push protocol
│   ├── sideband.rs     # Sideband multiplexing
│   ├── remote.rs       # Remote configuration
│   └── bundle.rs       # Bundle file format
├── tests/
│   ├── pktline_tests.rs
│   ├── fetch_tests.rs
│   └── push_tests.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| SSH transport | Subprocess (spawn ssh) | Simplest, uses user's SSH config and agent |
| HTTP transport | `reqwest` (async-capable) | Well-maintained, handles TLS, auth, proxies |
| Async I/O | Optional via feature flag | HTTP benefits from async; SSH/local don't need it |
| Credential helpers | Call git-credential as subprocess | Reuses existing credential helper ecosystem |
| Protocol default | Prefer v2, fall back to v1 | v2 is more efficient and widely supported |
| URL parsing | Custom parser matching C git | C git URL syntax has quirks not covered by url crate |

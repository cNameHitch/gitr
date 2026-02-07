# Implementation Plan: Hash & Object Identity

**Branch**: `002-hash-and-object-identity` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/002-hash-and-object-identity/spec.md`

## Summary

Implement the `git-hash` crate providing the core ObjectId type, hash computation, hex encoding/decoding, and OID collections. This crate is the second layer of the dependency graph — almost every other crate depends on it for object identification.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `sha1` 0.10, `sha2` 0.10 (RustCrypto), `hex` 0.4, `git-utils` (internal)
**Storage**: N/A (pure computation)
**Testing**: `cargo test`, property-based tests with `proptest`, benchmarks with `criterion`
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: SHA-1 hash > 500 MB/s, hex encode < 50ns, OID comparison < 5ns
**Constraints**: No `unsafe`. SHA-1 collision detection required.
**Scale/Scope**: ~6 C files replaced, ~2K lines of C → ~1K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | Pass | Pure computation, no unsafe needed |
| C-Compatibility | Pass | Hash output is algorithm-defined, identical by definition |
| Modular Crates | Pass | Single `git-hash` crate, depends only on `git-utils` |
| Trait-Based | Pass | HashAlgorithm trait for pluggable algorithms |
| Test-Driven | Pass | Property tests for hex round-trip, known test vectors |

## Project Structure

### Source Code

```text
crates/git-hash/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Public API, re-exports
│   ├── oid.rs          # ObjectId type, Display, FromStr, Eq, Ord, Hash
│   ├── hex.rs          # Hex encoding/decoding
│   ├── hasher.rs       # Streaming hash computation
│   ├── algorithm.rs    # HashAlgorithm enum and trait
│   ├── collections/
│   │   ├── mod.rs
│   │   ├── oid_array.rs   # Sorted OID array with binary search
│   │   ├── oid_map.rs     # HashMap<ObjectId, V>
│   │   └── oid_set.rs     # HashSet<ObjectId>
│   └── fanout.rs       # Fan-out table for pack index
├── tests/
│   ├── hash_vectors.rs    # Known hash test vectors
│   ├── hex_roundtrip.rs   # Property-based hex tests
│   └── collection_tests.rs
└── benches/
    └── hash_bench.rs      # Hash throughput benchmarks
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Hash crates | `sha1` + `sha2` from RustCrypto | Pure Rust, audited, widely used |
| OID representation | Enum with SHA-1 and SHA-256 variants | Runtime algorithm selection without generics pollution |
| Hex encoding | Custom implementation | Performance-critical path, avoid allocation where possible |
| Collision detection | Feature-gated sha1-checked | Optional for performance; always on for received objects |
| OID hashing | Use raw bytes directly | ObjectId bytes are already a hash — use identity or truncated hash |

## Complexity Tracking

No constitution violations anticipated.

# Implementation Plan: Packfile System

**Branch**: `005-packfile-system` | **Date**: 2026-02-05 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/005-packfile-system/spec.md`

## Summary

Implement the `git-pack` crate providing packfile reading, writing, delta encoding/decoding, pack index lookup, multi-pack index, bitmap index, and reverse index. Packfiles are git's primary storage optimization — they store objects efficiently using delta compression and are the wire format for network transfer.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition)
**Primary Dependencies**: `git-utils`, `git-hash`, `git-object`, `flate2`, `memmap2`, `crc32fast`, `thiserror`
**Storage**: File system — `.git/objects/pack/`
**Testing**: `cargo test`, `git verify-pack` validation
**Target Platform**: Linux, macOS, Windows
**Project Type**: Library crate
**Performance Goals**: Index lookup < 1μs, delta apply < 100μs for typical deltas, pack read throughput > 100 MB/s
**Constraints**: Must handle packs > 4GB. Memory-mapped access for performance.
**Scale/Scope**: ~8 C files, ~8K lines of C → ~5K lines of Rust

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| Safety-First | ✅ Pass | Bounds-checked delta application, hash verification |
| C-Compatibility | ✅ Pass | Pack format is precisely specified, verified with git verify-pack |
| Modular Crates | ✅ Pass | `git-pack` depends on utils/hash/object only |
| Trait-Based | ✅ Pass | PackBackend trait for pluggable pack implementations |
| Test-Driven | ✅ Pass | Verify against C git packs, known delta test vectors |

## Project Structure

### Source Code

```text
crates/git-pack/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── pack.rs             # PackFile: reading .pack files
│   ├── index.rs            # PackIndex: reading .idx files (v2)
│   ├── delta/
│   │   ├── mod.rs
│   │   ├── apply.rs        # Apply delta instructions
│   │   └── compute.rs      # Compute deltas between objects
│   ├── entry.rs            # Pack entry types (object header parsing)
│   ├── write.rs            # Pack generation (create .pack + .idx)
│   ├── midx.rs             # Multi-pack index (MIDX)
│   ├── bitmap.rs           # Bitmap index
│   ├── revindex.rs         # Reverse index (offset → OID)
│   └── verify.rs           # Pack checksum and integrity verification
├── tests/
│   ├── read_real_packs.rs  # Read C git-generated packs
│   ├── delta_vectors.rs    # Known delta instruction test vectors
│   ├── roundtrip.rs        # Write pack → read back → verify
│   └── midx_tests.rs
└── benches/
    ├── pack_read_bench.rs
    └── delta_bench.rs
```

## Architecture Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Pack access | `memmap2` for memory-mapped I/O | Packs can be multi-GB; mmap avoids loading into memory |
| Delta application | Bounds-checked copy operations | Safety: delta offsets must not read past base object end |
| Pack index | In-memory after mmap | Index files are small enough to map entirely |
| Delta computation | Based on C git's diff-delta algorithm | Must produce compatible deltas |
| CRC32 | `crc32fast` crate | Hardware-accelerated CRC32 for pack index validation |
| Large pack support | 64-bit offsets when pack > 2GB | Pack format v2 supports this in the index |

# Research: Hash & Object Identity

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| hash.c/h | ~300 | `hasher.rs`, `algorithm.rs` | Hash algorithm abstraction |
| hex.c | ~200 | `hex.rs` | Hex encoding/decoding |
| hex-ll.c | ~100 | `hex.rs` | Low-level hex utilities |
| hash-lookup.c | ~150 | `collections/oid_array.rs` | Binary search with fan-out |
| oid-array.c/h | ~200 | `collections/oid_array.rs` | Sorted OID array |
| oidmap.c/h | ~100 | `collections/oid_map.rs` | Hashmap keyed by OID |
| oidset.c/h | ~80 | `collections/oid_set.rs` | Hash set of OIDs |
| block-sha1/ | ~500 | Replaced by `sha1` crate | Block SHA-1 implementation |
| sha1dc/ | ~2000 | Replaced by `sha1` crate feature | SHA-1 collision detection |
| sha256/ | ~300 | Replaced by `sha2` crate | SHA-256 implementation |

## gitoxide (gix) Reference

- `gix-hash`: ObjectId type with `Kind` enum (Sha1). Good reference for API design.
- Uses `const` generics consideration but settled on enum approach.
- Stores raw bytes in a fixed-size array, max size for largest hash.

## Key Design Choices

### ObjectId Representation Options

1. **Enum approach** (chosen): `enum ObjectId { Sha1([u8; 20]), Sha256([u8; 32]) }`
   - Pro: Clear, safe, self-describing
   - Con: Match on every operation

2. **Fixed max-size array**: `struct ObjectId { bytes: [u8; 32], len: u8 }`
   - Pro: Single type, no matching
   - Con: Wastes space for SHA-1, easy to create invalid states

3. **Const generics**: `struct ObjectId<const N: usize>([u8; N])`
   - Pro: Zero-cost, type-safe
   - Con: Generics propagate to every using type

Decision: Enum approach balances safety and ergonomics. The match overhead is negligible compared to I/O costs.

### SHA-1 Collision Detection

C git uses sha1dc (SHA-1 with collision detection) to mitigate the SHAttered attack. The RustCrypto `sha1` crate does not include collision detection by default. Options:
- Use `sha1-checked` crate (Rust port of sha1dc)
- Feature-gate collision detection
- Always use collision detection for objects received from network

Decision: Always use collision detection for objects received from untrusted sources. Optional for local operations where performance matters.

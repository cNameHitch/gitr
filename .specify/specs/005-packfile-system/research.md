# Research: Packfile System

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| packfile.c/h | ~2000 | `pack.rs`, `entry.rs` | Pack reading, entry parsing |
| pack.h | ~200 | `lib.rs` | Pack format constants |
| pack-bitmap.c/h | ~1500 | `bitmap.rs` | EWAH bitmap index |
| pack-revindex.c/h | ~500 | `revindex.rs` | Reverse index |
| midx.c/h | ~1500 | `midx.rs` | Multi-pack index |
| diff-delta.c | ~500 | `delta/compute.rs` | Delta creation |
| patch-delta.c | ~200 | `delta/apply.rs` | Delta application |
| csum-file.c | ~300 | `write.rs` | Checksummed file writing |

## Pack Format (v2)

### .pack file structure
```
Header:  PACK (4 bytes) | version (4 bytes, network order) | num_objects (4 bytes)
Objects: [packed_object_1] [packed_object_2] ...
Trailer: SHA-1 of all preceding content (20 bytes)
```

### Packed object header (variable length)
```
Bits: [MSB][type:3][size:4] [MSB][size:7] [MSB][size:7] ...
```
- MSB = 1 means more size bytes follow
- type: 1=commit, 2=tree, 3=blob, 4=tag, 6=OFS_DELTA, 7=REF_DELTA
- size: uncompressed size (variable-length encoding)

### OFS_DELTA
After the header: variable-length negative offset to the base object in the same pack, then zlib-compressed delta instructions.

### REF_DELTA
After the header: 20-byte OID of the base object, then zlib-compressed delta instructions.

### Delta instruction format
```
Source and target size (variable-length integers)
Instructions:
  Copy: [1SSSOOOO] [offset bytes] [size bytes]
    - S bits: which size bytes present (1,2,3)
    - O bits: which offset bytes present (1,2,3,4)
  Insert: [0NNNNNNN] [N literal bytes]
    - N = 1-127: number of literal bytes to insert
    - N = 0: reserved (invalid)
```

## Pack Index (v2) Format

```
Header:  \377tOc (4 bytes magic) | version (4 bytes = 2)
Fanout:  256 × 4-byte network-order counts (cumulative)
OIDs:    N × 20-byte sorted OIDs
CRC32:   N × 4-byte CRC32 of packed object data
Offsets: N × 4-byte offsets (high bit = 1 means use 64-bit table)
64-bit:  M × 8-byte offsets (only for large packs)
Trailer: SHA-1 of pack file | SHA-1 of index content
```

## Multi-Pack Index (MIDX) Format
- Magic: MIDX
- Chunk-based format (like commit-graph)
- Chunks: pack names, OID fan-out, OID lookup, object offsets
- Optional: bitmap, reverse index

## gitoxide Reference

`gix-pack` is comprehensive:
- `gix_pack::data::File` for .pack reading
- `gix_pack::index::File` for .idx reading
- `gix_pack::multi_index::File` for MIDX
- Delta application in `gix_pack::data::entry`
- Highly optimized with parallel indexing

## Key Challenges

1. **Delta chains**: Objects can reference deltas-of-deltas, creating chains. Must handle arbitrary depth without stack overflow (use iteration, not recursion).

2. **Memory mapping**: Packs can exceed available RAM. Memory-mapped I/O with the OS paging system is essential.

3. **Thread safety**: Multiple threads may read from the same pack simultaneously. Mmap provides this naturally, but internal caches need synchronization.

4. **Thin packs**: Network protocol sends "thin" packs where base objects aren't included (they're on the receiver's side). Must handle missing bases gracefully.

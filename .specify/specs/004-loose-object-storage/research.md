# Research: Loose Object Storage

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| object-file.c | ~2500 | `read.rs`, `write.rs` | Only loose-object portions; rest is in spec 006 |
| loose.c/h | ~400 | `read.rs`, `enumerate.rs` | Loose object backend |

## Loose Object Format

### Path Layout
```
.git/objects/
├── 00/
│   └── 1234567890abcdef... (object with OID starting "00")
├── 01/
│   └── ...
├── ...
├── ff/
│   └── ...
├── info/
│   └── alternates       (spec 006)
└── pack/
    └── *.pack, *.idx   (spec 005)
```

### File Format
Each loose object is a zlib-compressed stream of:
```
<type> <size>\0<content>
```
- `type`: ASCII "blob", "tree", "commit", or "tag"
- `size`: ASCII decimal of content length
- `\0`: null byte separator
- `content`: raw object bytes

### Compression
- Default zlib compression level (Z_DEFAULT_COMPRESSION = 6)
- Configurable via `core.compression` (applies to everything) or `core.looseCompression` (overrides for loose only)
- Level 0 = no compression, 1 = fastest, 9 = best compression

### Write Protocol
1. Generate content: `header + content`
2. Compute hash of uncompressed data
3. Check if object already exists → skip if so
4. Create temp file in `.git/objects/` with random suffix
5. zlib-compress and write data to temp file
6. Set permissions (0444 on Unix)
7. Create fan-out directory if needed (`.git/objects/XX/`)
8. Rename temp file to final path (atomic on POSIX)
9. If rename fails because target exists (race condition), delete temp

## gitoxide Reference

`gix-odb` handles loose objects as part of the larger ODB:
- `gix_odb::store::loose::Store` — loose object read/write
- Supports streaming via `gix_features::zlib` wrapper
- Header-only reads via partial decompression

## Key Implementation Notes

1. **Temp file placement**: Must be in the same directory (or at least same filesystem) as the target to ensure `rename()` is atomic.

2. **Object existence check**: Before writing, check if `.git/objects/XX/YYY...` exists. This is a common fast path since objects are content-addressed (duplicates have the same OID).

3. **Header-only read**: For operations that only need type and size, decompress just enough bytes to read past the `\0`. This saves significant I/O for large objects.

4. **File permissions**: C git creates loose objects as read-only (0444) to signal they should not be modified. The `shared_repository` config affects group permissions.

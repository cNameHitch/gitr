# Research: Index / Staging Area

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| read-cache.c | ~3000 | `read.rs`, `write.rs`, `lib.rs` | Index read/write, core operations |
| cache-tree.c/h | ~500 | `extensions/tree.rs` | Cache tree extension |
| split-index.c/h | ~400 | `read.rs` (partial) | Split index support |
| sparse-index.c/h | ~300 | Future | Sparse index |
| resolve-undo.c/h | ~200 | `extensions/resolve_undo.rs` | REUC extension |
| name-hash.c | ~300 | `lib.rs` | Name-based lookup table |
| preload-index.c | ~200 | `lib.rs` | Parallel stat |
| entry.c/h | ~300 | `entry.rs` | Entry manipulation |
| unpack-trees.c/h | ~2000 | `lib.rs` (partial) | Tree → index operations |
| dir.c/h | ~3000 | `dir.rs`, `ignore.rs` | Directory scanning, gitignore |
| attr.c/h | ~1500 | `attributes.rs` | Gitattributes |
| pathspec.c/h | ~800 | `pathspec.rs` | Pathspec matching |

## Index File Format

### Header (12 bytes)
```
DIRC (4 bytes magic)
Version (4 bytes, network order): 2, 3, or 4
Number of entries (4 bytes, network order)
```

### Cache Entry (v2)
```
32-bit ctime seconds
32-bit ctime nanoseconds
32-bit mtime seconds
32-bit mtime nanoseconds
32-bit dev
32-bit ino
32-bit mode
32-bit uid
32-bit gid
32-bit file size
160-bit (20-byte) SHA-1
16-bit flags:
  - 1 bit: assume-valid
  - 1 bit: extended (must be 0 in v2)
  - 2 bits: merge stage (0-3)
  - 12 bits: name length (max 0xFFF, 0xFFF means use strlen)
[v3+ only] 16-bit extended flags:
  - 1 bit: intent-to-add
  - 1 bit: skip-worktree
  - 14 bits: reserved
Path name (variable length, null-terminated)
Padding: 1-8 null bytes to align to 8-byte boundary (v2/v3), none for v4
```

### v4 Path Compression
v4 uses prefix compression: each entry stores the number of bytes to remove from the previous entry's path, then the new suffix. This significantly reduces index size for repos with deep directory trees.

### Extensions
```
4-byte extension signature (e.g., "TREE", "REUC", "UNTR")
32-bit size
extension data
```
Known extensions:
- **TREE**: Cached tree OIDs for fast commit
- **REUC**: Resolve-undo data (original versions before conflict resolution)
- **UNTR**: Untracked file cache
- **FSMN**: Filesystem monitor data
- **EOIE**: End of index entry (for parallel loading)
- **IEOT**: Index entry offset table (for parallel loading)
- **sdir**: Sparse directory entries

### Trailer
```
20-byte SHA-1 checksum of all preceding index content
```

## Gitignore Rules

Patterns are loaded from (low to high priority):
1. `core.excludesFile` (global, e.g., `~/.config/git/ignore`)
2. `.gitignore` at repo root
3. `.gitignore` in subdirectories (scoped to that directory)
4. `.git/info/exclude` (repo-local, not committed)

Pattern syntax:
- `#` comment lines
- `!` negation (re-include previously excluded)
- `/` at start: anchored to directory containing the .gitignore
- `/` at end: match only directories
- `*` matches anything except `/`
- `**` matches across directories
- `?` matches any single character except `/`
- `[...]` character class

## gitoxide Reference

`gix-index` provides comprehensive index support:
- `gix_index::File` for reading/writing
- `gix_index::Entry` for cache entries
- Extensions preserved as raw bytes + parsed where known
- Good reference for v4 path decompression

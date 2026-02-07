# Research: Object Database

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| odb/ directory | ~500 | `backend.rs` | Object database backend infrastructure |
| object-file.c (high-level) | ~1000 | `lib.rs`, `search.rs` | Unified read/write, search across backends |
| sha1-file.c (portions) | ~500 | `alternates.rs` | Alternates parsing |

## Search Order

C git searches for objects in this order:
1. Cached objects (in-memory parse cache)
2. Loose objects in `.git/objects/`
3. Packed objects (newest pack first, or via MIDX)
4. Alternates (recursively, same order)

The loose-first ordering is important: during operations like `git repack`, an object might temporarily exist in both loose and packed form. The loose version is considered authoritative because it's being actively written.

## Alternates

The file `.git/objects/info/alternates` contains one path per line:
```
/path/to/other/repo/.git/objects
/another/repo/.git/objects
```

Each alternate is itself an object store that may have its own alternates (chain). Circular chains must be detected.

### Usage patterns:
- `git clone --reference <other>`: Sets up alternates to avoid copying objects
- `git worktree add`: Worktrees share objects via alternates
- GitHub fork network: Forks share objects on server side

## gitoxide Reference

`gix_odb::Store` is the equivalent:
- General store managing loose and pack storage
- `gix_odb::store::Handle` for thread-safe access
- Dynamic pack refresh (detects new packs added by concurrent processes)
- Slot-based design for pack management

Key insight from gix: packs can appear or disappear at any time (gc, repack). The ODB must periodically refresh its list of available packs.

## Thread Safety Considerations

Multiple threads may:
- Read objects concurrently (common in diff, merge)
- One thread writes while others read (less common but must work)
- Packs may be added/removed by external git processes

Solution:
- Reads are lock-free for memory-mapped packs (OS handles concurrency)
- Pack list protected by RwLock (write lock only when refreshing)
- Loose reads are inherently safe (files are immutable once written)

# Research: Object Model

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| object.c/h | ~600 | `lib.rs` | Object type enum, parsed object pool |
| blob.c/h | ~100 | `blob.rs` | Minimal — blob is just bytes |
| tree.c/h | ~500 | `tree.rs` | Tree parsing, entry iteration, sorting |
| commit.c/h | ~800 | `commit.rs` | Commit parsing, parent traversal |
| tag.c/h | ~400 | `tag.rs` | Tag parsing, peeling |
| alloc.c/h | ~200 | Not needed | Rust handles allocation |
| object-name.c/h | ~1800 | `name.rs` | Rev-parse logic — complex! |

## Git Object Format

### Object Header
```
<type> <size-in-decimal>\0<content>
```
Where type is "blob", "tree", "commit", or "tag".

### Tree Entry Format (binary)
```
<mode-ascii> <name>\0<20-byte-oid>
```
- Mode is ASCII decimal (e.g., "100644", "40000")
- Name is the filename (bytes, no '/' separators)
- OID is raw binary, not hex

### Tree Sorting Rules
Git sorts tree entries as if directories have a trailing '/'. So a tree entry for directory "foo" (mode 40000) sorts as "foo/", while a file "foo.c" sorts as "foo.c". This means "foo" (dir) sorts before "foo.c" but after "foo-bar".

### Commit Format
```
tree <hex-oid>\n
parent <hex-oid>\n          (0 or more)
author <name> <email> <timestamp> <tz>\n
committer <name> <email> <timestamp> <tz>\n
encoding <encoding>\n       (optional)
\n
<message>
```

### Tag Format
```
object <hex-oid>\n
type <type-name>\n
tag <tag-name>\n
tagger <name> <email> <timestamp> <tz>\n
\n
<message>
```

## gitoxide Reference

`gix-object` crate provides:
- `Object` enum (Blob, Tree, Commit, Tag) with `data` and `kind` fields
- Separate `*Ref` types for borrowed/zero-copy parsing
- `TreeRef`, `CommitRef` for zero-copy iteration
- Immutable and mutable variants

Key differences from our approach:
- gix uses separate owned vs borrowed types (e.g., `Tree` vs `TreeRef`)
- We'll start with owned types for simplicity, add borrowed variants if needed for performance

## Object Name Resolution (rev-parse)

The C implementation in `object-name.c` is ~1800 lines handling:
- Full hex SHA-1/SHA-256
- Abbreviated hex (minimum 4 chars, configurable)
- Ref names (branches, tags, remote refs)
- `HEAD`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`, etc.
- `@{upstream}`, `@{push}`
- `^` (parent), `~` (ancestor)
- `^{type}` (peel to type)
- `^{/regex}` (search commit messages)
- `:/regex` (search from any ref)
- `@{date}` (reflog by date)
- `@{N}` (reflog by index)

This is one of the most complex single features. Implementation will be incremental.

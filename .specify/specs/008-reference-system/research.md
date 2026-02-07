# Research: Reference System

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| refs.c/h | ~1500 | `lib.rs`, `store.rs` | Ref API, backend dispatch |
| refs/files-backend.c | ~2500 | `files/` | Files backend implementation |
| refs/packed-backend.c | ~1000 | `files/packed.rs` | Packed-refs reading/writing |
| refs/reftable-backend.c | ~1500 | Deferred to spec 018 | Reftable backend |
| reflog.c | ~500 | `reflog.rs` | Reflog operations |

## Reference Types

### Direct ref (loose file)
A file at `.git/refs/heads/main` containing a hex OID + newline:
```
abc123def456...789\n
```

### Symbolic ref
A file containing `ref: <target-ref-name>`:
```
ref: refs/heads/main\n
```
HEAD is typically a symbolic ref pointing to the current branch.

### Packed refs
The `.git/packed-refs` file:
```
# pack-refs with: peeled fully-peeled sorted
abc123... refs/heads/main
def456... refs/tags/v1.0
^789abc... (peeled value of the tag above)
```

## Ref Name Rules (git-check-ref-format)

Valid ref names must:
- Not contain: space, ~, ^, :, ?, *, [, \
- Not start with . or end with /
- Not contain //
- Not end with .
- Not end with .lock
- Not be @{...}
- Not contain @{
- Not be exactly "@"
- Not contain a \0

## Ref Transactions (C API)

C git's ref transaction API:
1. `ref_transaction_begin()` — create transaction
2. `ref_transaction_update()` — add an update to the transaction
3. `ref_transaction_delete()` — add a delete to the transaction
4. `ref_transaction_commit()` — atomically apply all updates
5. `ref_transaction_abort()` — cancel

Each update includes: ref name, new value, old value (for CAS), flags, reflog message.

The commit phase:
1. Lock all refs being updated
2. Verify all old values match (CAS check)
3. Write new values
4. Rename lock files (atomic)
5. Update reflogs

If any step fails, all locks are released (rollback).

## Reflog Format

Each line in `.git/logs/refs/heads/main`:
```
<old-oid> <new-oid> <name> <email> <timestamp> <tz>\t<message>\n
```
Example:
```
0000...0000 abc1...def2 Alice <alice@example.com> 1234567890 +0000\tcheckout: moving from old to new\n
```

## gitoxide Reference

`gix-ref` provides:
- `gix_ref::file::Store` for files backend
- `gix_ref::packed::Buffer` for parsed packed-refs
- `gix_ref::transaction::Change` for ref transactions
- Comprehensive reflog support
- Good reference for the trait-based architecture

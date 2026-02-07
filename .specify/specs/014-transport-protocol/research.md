# Research: Transport Protocol

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| transport.c/h | ~800 | `git-transport/lib.rs` | Transport abstraction |
| connect.c/h | ~500 | `git-transport/ssh.rs`, `local.rs` | Connection setup |
| pkt-line.c/h | ~400 | `git-protocol/pktline.rs` | Pkt-line framing |
| protocol.c/h | ~200 | `git-protocol/capability.rs` | Version negotiation |
| fetch-pack.c/h | ~1500 | `git-protocol/fetch.rs` | Fetch client |
| upload-pack.c | ~1000 | Server-side (deferred) | Upload pack server |
| send-pack.c | ~500 | `git-protocol/push.rs` | Push client |
| receive-pack.c | ~800 | Server-side (deferred) | Receive pack server |
| http.c/h | ~1000 | `git-transport/http.rs` | HTTP transport |
| remote.c/h | ~800 | `git-protocol/remote.rs` | Remote configuration |
| remote-curl.c | ~1000 | `git-transport/http.rs` | HTTP/HTTPS helper |
| sideband.c/h | ~200 | `git-protocol/sideband.rs` | Sideband demux |
| bundle.c/h | ~500 | `git-protocol/bundle.rs` | Bundle format |

## Pkt-Line Format

Each packet: `<4-hex-digit-length><data>`
- Length includes the 4 bytes of the length field itself
- Special packets: `0000` (flush), `0001` (delimiter, v2), `0002` (response-end, v2)
- Maximum data per packet: 65516 bytes (65520 - 4)

Example:
```
001e# service=git-upload-pack\n
0000
00a0ref1 HEAD\0capabilities...\n
003fref2 refs/heads/main\n
0000
```

## Protocol v2

Initiated by `version 2\n` in initial exchange.

Commands:
- `ls-refs`: List references (server-side ref filtering)
- `fetch`: Fetch objects (replaces entire v1 exchange)

Capabilities:
- `object-format`: Hash algorithm
- `fetch`: Fetch support with sub-capabilities
- `server-option`: Server-specific options
- `partial-clone`: Partial clone/fetch support

## Fetch Negotiation (v1)

1. Client sends `want <oid> <capabilities>` for each desired ref
2. Client sends `have <oid>` for objects it already has
3. Server responds with `ACK <oid>` for common objects
4. When done, server sends pack data

## Fetch Negotiation (v2)

1. Client sends `command=fetch` with capabilities
2. Client sends `want <oid>` lines
3. Client sends `have <oid>` lines
4. Server responds with `acknowledgments` section
5. Server sends `packfile` section with pack data

## Push Protocol (send-pack ↔ receive-pack)

### v1 Push Flow

1. Client connects to `receive-pack` service on remote
2. Server advertises refs: `<oid> <refname>\0<capabilities>\n` (first line), `<oid> <refname>\n` (subsequent)
3. Server sends flush packet `0000`
4. Client sends ref update commands:
   ```
   <old-oid> <new-oid> <refname>\n    # update
   <zero-oid> <new-oid> <refname>\n   # create
   <old-oid> <zero-oid> <refname>\n   # delete
   ```
5. Client sends flush packet `0000`
6. If push-options negotiated, client sends option lines, then flush
7. Client generates and streams thin pack data (objects reachable from new OIDs but not from remote's advertised OIDs)
8. Server receives pack, runs `index-pack --fix-thin` to resolve thin pack deltas, then updates refs
9. Server sends status report (if `report-status` capability):
   ```
   unpack ok\n          # or: unpack <error>\n
   ok <refname>\n       # per-ref success
   ng <refname> <msg>\n # per-ref failure
   ```

### Push Capabilities

| Capability | Description |
|------------|-------------|
| `report-status` | Server sends unpack + per-ref status |
| `report-status-v2` | Extended status with option support |
| `delete-refs` | Server allows ref deletion |
| `atomic` | All ref updates succeed or all fail |
| `ofs-delta` | Pack may use OFS_DELTA (offset-based) |
| `push-options` | Client may send push-option strings |
| `no-thin` | Server doesn't want thin packs |
| `side-band` / `side-band-64k` | Multiplexed pack + progress |

### Thin Pack for Push

During push, the client generates a "thin" pack:
- Objects reachable from the new OIDs being pushed
- Minus objects reachable from the remote's advertised OIDs
- Delta bases may reference objects not in the pack (the remote already has them)
- The remote's `receive-pack` calls `index-pack --fix-thin` to complete the pack by fetching missing delta bases from its own ODB

### Force-with-Lease

Client-side check before sending ref updates:
- Client records the expected remote OID for each ref
- Before sending the update, client verifies the remote's advertised OID matches the expected value
- If mismatch, the ref update is not sent and the push fails for that ref
- This is entirely client-side — the server just sees a normal ref update

### v2 Push

Protocol v2 does not (as of C git 2.44) define a `push` command. Push in v2 falls back to v0/v1 `receive-pack` semantics. The v2 handshake is used only for capability discovery; the actual push exchange uses the v1 protocol.

## Git URL Syntax

```
ssh://[user@]host[:port]/path
git://host[:port]/path
http[s]://[user@]host[:port]/path
/local/path
file:///local/path
user@host:path (SCP-like SSH syntax)
```

The SCP-like syntax (`user@host:path`) is special — no scheme, colon separates host from path. Must not be confused with Windows drive letters (`C:\path`).

## gitoxide Reference

`gix-transport`:
- Trait-based transport abstraction
- SSH via process, HTTP via `reqwest`
- Async support throughout

`gix-protocol`:
- Full v1 and v2 support
- Comprehensive capability handling

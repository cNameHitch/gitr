# Research: Advanced Features

## C Source File Mapping

| C File | Rust Location | Feature |
|--------|--------------|---------|
| builtin/gc.c | `src/commands/gc.rs` | Garbage collection |
| builtin/repack.c | `src/commands/repack.rs` | Repacking |
| builtin/prune.c | `src/commands/prune.rs` | Pruning unreachable objects |
| builtin/fsck.c | `src/commands/fsck.rs` | Integrity checking |
| builtin/pack-objects.c | `src/commands/pack_objects.rs` | Low-level pack creation |
| builtin/index-pack.c | `src/commands/index_pack.rs` | Pack indexing |
| builtin/submodule--helper.c | `crates/git-submodule/` | Submodule operations |
| builtin/worktree.c | `src/commands/worktree.rs` | Worktree management |
| builtin/notes.c | `src/commands/notes.rs` | Notes |
| builtin/replace.c | `src/commands/replace.rs` | Object replacement |
| builtin/archive.c | `crates/git-archive/` | Archive generation |
| builtin/fast-import.c | `src/commands/fast_import.rs` | Bulk import |
| bundle.c/h | `src/commands/bundle.rs` | Bundle files |
| builtin/daemon.c | `src/commands/daemon.rs` | Git daemon |
| credential.c | `src/commands/credential.rs` | Credential helpers |
| gpg-interface.c | Library code | GPG signing/verification |
| hook.c | `crates/git-utils/` or `git-repository/` | Hook execution |
| fsmonitor.c | Library code | Filesystem monitoring |

## GC Algorithm

`git gc` performs:
1. Pack all loose objects: `repack -d -l`
2. Pack all refs: `pack-refs --all --prune`
3. Remove old reflog entries: `reflog expire`
4. Prune unreachable objects: `prune --expire`
5. Remove stale temp files: `prune-packed`
6. Update server info: `update-server-info` (optional)

Auto GC triggers when:
- Loose objects > gc.auto (default 6700)
- Packs > gc.autoPackLimit (default 50)

## Fsck Checks

Object validation:
- **Blob**: Any content is valid
- **Tree**: Entries sorted correctly, valid modes, no duplicate names, no null bytes in names
- **Commit**: Has tree, valid parent OIDs, valid author/committer signatures, timestamp not in future (warning)
- **Tag**: Valid target OID, valid target type, has tag name, valid tagger signature

Connectivity check:
- All objects referenced by commits/trees/tags exist
- All refs point to valid objects
- No cycles in commit graph

## Submodule Structure

`.gitmodules` file (tracked):
```ini
[submodule "libs/foo"]
    path = libs/foo
    url = https://github.com/user/foo.git
    branch = main
```

`.git/modules/<name>/` — the submodule's git directory
`.git/config` — submodule.*.url (possibly overridden)

Submodule state:
- Gitlink entry in tree (mode 160000, OID = submodule's HEAD)
- `.gitmodules` tracks metadata
- `.git/modules/` stores submodule git data

## Hook Points

| Hook | Trigger | Can Abort |
|------|---------|-----------|
| pre-commit | Before commit | Yes |
| prepare-commit-msg | Before editor opens | Yes |
| commit-msg | After message entered | Yes |
| post-commit | After commit created | No |
| pre-rebase | Before rebase | Yes |
| post-checkout | After checkout/switch | No |
| post-merge | After merge | No |
| pre-push | Before push | Yes |
| pre-receive | Server: before accepting push | Yes |
| update | Server: per-ref update | Yes |
| post-receive | Server: after accepting push | No |
| post-update | Server: after refs updated | No |
| fsmonitor-watchman | File system change query | N/A |

# Research: Foundation Utilities

## C Source File Mapping

| C File | Lines | Rust Module | Notes |
|--------|-------|-------------|-------|
| strbuf.c/h | ~1200 | `bstring.rs` | Replaced by `bstr` crate; extension traits only |
| hashmap.c/h | ~400 | `collections/hashmap.rs` | Rust HashMap with BuildHasher suffices |
| string-list.c/h | ~300 | `collections/string_list.rs` | Sorted Vec<(BString, Option<T>)> |
| prio-queue.c/h | ~150 | `collections/prio_queue.rs` | BinaryHeap wrapper |
| path.c/h | ~1500 | `path.rs` | Complex: git_path_dirname, normalize, etc. |
| wildmatch.c/h | ~300 | `wildmatch.rs` | Must port exactly for gitignore compat |
| date.c | ~1200 | `date.rs` | Complex: approxidate, relative dates |
| quote.c/h | ~400 | `bstring.rs` | Shell quoting, C quoting |
| parse-options.c/h | ~800 | Replaced by `clap` | Complete replacement |
| run-command.c/h | ~600 | `subprocess.rs` | Subprocess management |
| tempfile.c/h | ~300 | `tempfile.rs` | RAII temp files |
| lockfile.c/h | ~250 | `lockfile.rs` | RAII lock files |
| wrapper.c | ~500 | Various | xmalloc etc. → Rust memory safety |
| usage.c/h | ~200 | `error.rs` | die/warning → Result/error types |
| color.c | ~300 | `color.rs` | ANSI color management |
| progress.c | ~400 | `progress.rs` | Progress bar display |
| editor.c | ~150 | `subprocess.rs` | Editor launching |
| sigchain.c | ~100 | Not needed | Rust's Drop handles cleanup |
| thread-utils.c | ~200 | Not needed | Rust's std::thread and crossbeam suffice |
| utf8.c/h | ~400 | `bstring.rs` | UTF-8 utilities, mostly in bstr |
| mem-pool.c/h | ~200 | Not needed | Rust's allocator handles this |
| strmap.c/h | ~100 | `collections/hashmap.rs` | HashMap<BString, V> |

## Existing Rust Ecosystem

### gitoxide (gix) Reference

The `gix` project (https://github.com/GitoxideLabs/gitoxide) has already implemented many of these utilities:
- `gix-path`: Path manipulation (good reference but different API design)
- `gix-date`: Date parsing (comprehensive, could study their approach)
- `gix-glob`: Glob matching (may have compatibility differences)
- `gix-lock`: Lock file protocol
- `gix-tempfile`: Signal-safe temp files

**Assessment**: gix is a valid reference but has different design goals (idiomatic Rust-first vs C-compatibility-first). We should study their implementations but prioritize byte-identical compatibility with C git.

### Key Crates

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `bstr` | 1.x | Byte string handling | Essential — no alternative |
| `clap` | 4.x | CLI argument parsing | Industry standard |
| `thiserror` | 2.x | Error type derivation | Zero-cost error types |
| `crossbeam` | 0.8 | Threading utilities | Channels, scoped threads |
| `chrono` | 0.4 | Date/time formatting | For output formatting only |
| `proptest` | 1.x | Property-based testing | Fuzzing date/wildmatch |
| `criterion` | 0.5 | Benchmarking | Performance tracking |

## Key Porting Challenges

1. **strbuf → bstr**: C's strbuf is mutable, growable. Rust's BString is the equivalent. Key difference: C code often mutates in place; Rust prefers returning new values.

2. **wildmatch**: Must be a direct port of wildmatch.c. The algorithm handles `**` (match across path separators), character classes, and case folding. The C git test suite has comprehensive test vectors.

3. **date.c**: One of the most complex files. `approxidate()` parses incredibly flexible date formats ("last tuesday", "2 weeks ago", "noon yesterday"). Must match C git's heuristics exactly.

4. **Lock files**: C git uses signal handlers to clean up lock files on crash. Rust needs both `Drop` and signal handler (`ctrlc` crate or similar) for robustness.

5. **parse-options.c → clap**: The C option parsing supports git-specific conventions (--no-X negation, command-specific options, subcommand aliasing). clap handles most of this natively.

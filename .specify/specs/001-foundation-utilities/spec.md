# Feature Specification: Foundation Utilities

**Feature Branch**: `001-foundation-utilities`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: None (root of dependency graph)

## User Scenarios & Testing

### User Story 1 - Byte String Manipulation (Priority: P1)

As a gitr library consumer, I need a robust byte string type that handles git's non-UTF-8 paths and content so that all git operations work correctly regardless of encoding.

**Why this priority**: Nearly every subsystem depends on byte-aware string handling. Git paths, commit messages, and file content are byte sequences, not guaranteed UTF-8.

**Independent Test**: Create a `BStr`-based path, manipulate it (join, split, compare), and verify round-trip through file system operations preserves bytes.

**Acceptance Scenarios**:

1. **Given** a path containing non-UTF-8 bytes, **When** the path is stored and retrieved, **Then** the bytes are preserved exactly.
2. **Given** two byte strings, **When** they are concatenated with a path separator, **Then** the result matches git's path joining behavior.
3. **Given** a byte string with mixed encodings, **When** displayed to the user, **Then** invalid UTF-8 sequences are replaced with the Unicode replacement character.

---

### User Story 2 - Error Handling Framework (Priority: P1)

As a gitr developer, I need a consistent error handling framework so that all library crates report errors uniformly with full context chains.

**Why this priority**: Error handling is foundational — every function that can fail needs this infrastructure before implementation.

**Independent Test**: Trigger errors at multiple layers (I/O, parse, logic) and verify error messages include context chain, source location, and actionable information.

**Acceptance Scenarios**:

1. **Given** a low-level I/O error, **When** it propagates through the call stack, **Then** each layer adds context (e.g., "reading packfile: opening index: file not found: /path").
2. **Given** a library error, **When** caught by the CLI, **Then** it can be formatted as either user-friendly text or structured JSON.
3. **Given** an error in library code, **When** it occurs, **Then** no panic is triggered — all errors use `Result<T, E>`.

---

### User Story 3 - CLI Argument Parsing (Priority: P1)

As a git user, I need the Rust git CLI to accept the same arguments as C git so that existing scripts and muscle memory work unchanged.

**Why this priority**: The CLI is the primary user interface. Argument compatibility is essential for drop-in replacement.

**Independent Test**: Run `gitr <command> --help` and verify the argument structure matches C git for the same command.

**Acceptance Scenarios**:

1. **Given** a git command with standard options, **When** parsed by the Rust CLI, **Then** the same arguments are accepted with identical semantics.
2. **Given** `--` separator, **When** parsing arguments, **Then** everything after `--` is treated as pathspecs, matching C git behavior.
3. **Given** combined short flags (e.g., `-am`), **When** parsed, **Then** they are correctly split into individual flags.

---

### User Story 4 - Path Manipulation (Priority: P1)

As a gitr library consumer, I need cross-platform path utilities that handle git's path conventions (forward slashes in tree entries, case sensitivity rules).

**Why this priority**: Path handling is used by every subsystem that touches the working tree, index, or tree objects.

**Independent Test**: Create paths on the current platform and verify normalization, joining, and comparison match git's conventions.

**Acceptance Scenarios**:

1. **Given** a Windows-style path with backslashes, **When** normalized for git storage, **Then** it uses forward slashes.
2. **Given** a path with `.` and `..` components, **When** cleaned, **Then** they are resolved correctly.
3. **Given** two paths, **When** compared with case-sensitivity matching the file system, **Then** the comparison result is correct.

---

### User Story 5 - Subprocess Execution (Priority: P2)

As a gitr command, I need to spawn and manage subprocesses (hooks, external diff tools, credential helpers) with proper stdio piping.

**Why this priority**: Many git commands invoke external programs. This is needed before implementing hooks and helpers.

**Independent Test**: Spawn a subprocess, pipe stdin/stdout/stderr, and verify exit codes are captured correctly.

**Acceptance Scenarios**:

1. **Given** a hook script, **When** executed by gitr, **Then** stdin/stdout/stderr are connected correctly and exit code determines success/failure.
2. **Given** a subprocess that times out, **When** the timeout expires, **Then** the process is killed and an appropriate error is returned.
3. **Given** a subprocess in a pipeline, **When** stdout of one feeds stdin of another, **Then** data flows correctly without deadlock.

---

### User Story 6 - Lock Files and Atomic Writes (Priority: P2)

As a gitr library, I need atomic file operations using lock files so that concurrent git processes don't corrupt repository state.

**Why this priority**: Any write to refs, index, or config needs atomic file operations to prevent corruption.

**Independent Test**: Acquire a lock, write content, commit the lock, and verify the file is updated atomically. Verify that a second locker fails gracefully.

**Acceptance Scenarios**:

1. **Given** a file to update, **When** a lock is acquired, **Then** a `.lock` file is created and the original file is unchanged.
2. **Given** a held lock, **When** another process tries to lock the same file, **Then** it receives an error indicating the file is locked.
3. **Given** a lock that is committed, **When** the commit completes, **Then** the `.lock` file is atomically renamed to the target file.
4. **Given** a lock that is dropped without commit, **When** the `Drop` impl runs, **Then** the `.lock` file is removed and the original file is unchanged.

---

### User Story 7 - Progress Display and Color Output (Priority: P3)

As a git user, I need progress bars, colored output, and pager support so that the Rust CLI provides the same terminal experience as C git.

**Why this priority**: Important for user experience but not blocking any library functionality.

**Independent Test**: Run a long operation and verify progress updates appear on stderr. Verify color output respects `--color=auto/always/never` and `NO_COLOR`.

**Acceptance Scenarios**:

1. **Given** a long operation (e.g., counting objects), **When** progress is enabled, **Then** a progress bar updates on stderr showing count and throughput.
2. **Given** `--color=never`, **When** output is generated, **Then** no ANSI escape codes are present.
3. **Given** stdout piped to a file, **When** `--color=auto`, **Then** color is disabled automatically.
4. **Given** a large output, **When** stdout is a terminal, **Then** output is piped through the configured pager.

---

### User Story 8 - Date Parsing (Priority: P2)

As a gitr library, I need to parse git's flexible date formats so that commit timestamps, reflog entries, and date-based filters work correctly.

**Why this priority**: Commits and reflogs contain timestamps. Many commands accept date-based arguments.

**Independent Test**: Parse various date formats ("2 weeks ago", "2025-01-15", "@1234567890", "yesterday") and verify they produce correct Unix timestamps.

**Acceptance Scenarios**:

1. **Given** an ISO 8601 date string, **When** parsed, **Then** the correct Unix timestamp and timezone offset are returned.
2. **Given** a relative date ("3 days ago"), **When** parsed, **Then** the correct absolute timestamp is computed.
3. **Given** a raw timestamp ("@1234567890 +0000"), **When** parsed, **Then** the exact values are preserved.
4. **Given** a date format matching `git log --date=FORMAT`, **When** formatted, **Then** output matches C git exactly.

---

### User Story 9 - Glob/Wildmatch Pattern Matching (Priority: P2)

As a gitr library, I need glob pattern matching compatible with git's wildmatch algorithm for gitignore, pathspec, and refspec patterns.

**Why this priority**: Used by gitignore, pathspec matching, and ref patterns.

**Independent Test**: Match patterns against paths and verify results match C git's wildmatch function.

**Acceptance Scenarios**:

1. **Given** pattern `*.c` and path `foo.c`, **When** matched, **Then** result is true.
2. **Given** pattern `**/foo`, **When** matched against `a/b/foo`, **Then** result is true.
3. **Given** pattern `foo/**/bar`, **When** matched against `foo/x/y/bar`, **Then** result is true.
4. **Given** pattern with character class `[a-z]`, **When** matched, **Then** behavior matches C git's wildmatch.

---

### User Story 10 - Hashmap and Collections (Priority: P1)

As a gitr developer, I need efficient hashmap and collection types that match git's internal data structure semantics.

**Why this priority**: Git's internal hashmap, string-list, and priority queue are used pervasively.

**Independent Test**: Create collections, insert/lookup/iterate, and verify O(1) lookup and correct ordering behavior.

**Acceptance Scenarios**:

1. **Given** a hashmap with custom hash function, **When** items are inserted and looked up, **Then** the correct values are returned with O(1) average complexity.
2. **Given** a sorted string list, **When** items are inserted and searched, **Then** binary search finds items correctly.
3. **Given** a priority queue, **When** items are pushed and popped, **Then** they come out in priority order.

### Edge Cases

- Path containing null bytes (git rejects these)
- Empty string arguments to CLI
- Lock file left behind by crashed process (stale lock detection)
- Terminal width of 0 (piped output)
- Date overflow (year 2038+, year 9999)
- Extremely long paths (>4096 bytes)
- Non-monotonic system clock for relative date calculations

## Requirements

### Functional Requirements

- **FR-001**: System MUST provide a byte string type wrapping `bstr::BString`/`bstr::BStr` for all git paths and content
- **FR-002**: System MUST provide a hashmap type with configurable hash function for internal use
- **FR-003**: System MUST provide a sorted string list with optional associated data payload
- **FR-004**: System MUST provide a priority queue supporting min-heap and max-heap operations
- **FR-005**: System MUST provide cross-platform path manipulation (normalize, join, relativize, clean)
- **FR-006**: System MUST provide wildmatch-compatible glob pattern matching
- **FR-007**: System MUST provide CLI argument parsing via `clap` that accepts C git's argument conventions
- **FR-008**: System MUST provide subprocess spawning with full stdio control (pipe, inherit, null)
- **FR-009**: System MUST provide lock file acquisition, commit, and rollback with RAII semantics
- **FR-010**: System MUST provide atomic file writes (write to temp, rename to target)
- **FR-011**: System MUST provide progress bar display on stderr with throughput calculation
- **FR-012**: System MUST provide colored output respecting `--color`, `NO_COLOR`, and terminal detection
- **FR-013**: System MUST provide pager integration (respect `GIT_PAGER`, `core.pager`, `PAGER`)
- **FR-014**: System MUST parse all git date formats (ISO 8601, relative, raw timestamp, RFC 2822)
- **FR-015**: System MUST format dates in all git output formats (relative, local, iso, rfc, short, raw, human, unix)
- **FR-016**: System MUST provide `Result<T, E>`-based error types with `thiserror` — no panics in library code
- **FR-017**: System MUST provide tempfile creation with automatic cleanup on drop

### Key Entities

- **BytePath**: A git path represented as a byte sequence, with methods for joining, splitting, normalizing
- **LockFile**: RAII guard for `.lock` file creation, writing, committing, and cleanup
- **Progress**: Terminal progress display with count, throughput, and percentage
- **DateValue**: Parsed date with Unix timestamp and timezone offset
- **WildmatchPattern**: Compiled glob pattern for efficient repeated matching

## Success Criteria

### Measurable Outcomes

- **SC-001**: All C git date format strings are parsed correctly (validated against a corpus of 100+ real date strings)
- **SC-002**: Wildmatch produces identical results to C git for the entire wildmatch test suite
- **SC-003**: Lock file operations are safe under concurrent access (verified with multi-threaded stress test)
- **SC-004**: CLI argument parsing accepts all options documented in C git man pages for implemented commands
- **SC-005**: Zero panics in library code under any input (verified with fuzzing)
- **SC-006**: All path operations produce correct results on Linux, macOS, and Windows

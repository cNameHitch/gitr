# Feature Specification: Configuration System

**Feature Branch**: `009-configuration-system`
**Created**: 2026-02-05
**Status**: Draft
**Depends On**: 001-foundation-utilities

## User Scenarios & Testing

### User Story 1 - Read Configuration Values (Priority: P1)

As a gitr library consumer, I need to read git configuration values from the standard config file hierarchy so that user preferences and repository settings are respected.

**Why this priority**: Almost every git operation reads configuration. This is blocking for most features.

**Independent Test**: Set a config value with C git, read it with gitr, verify they match.

**Acceptance Scenarios**:

1. **Given** a `.gitconfig` file with `[user] name = Alice`, **When** `config.get_string("user.name")` is called, **Then** "Alice" is returned.
2. **Given** overlapping keys in system, global, and local config, **When** a key is read, **Then** the most-specific scope wins (local > global > system).
3. **Given** a multi-valued key (e.g., `remote.origin.fetch`), **When** read, **Then** all values are returned in order.
4. **Given** a boolean key written as "yes"/"on"/"true"/"1", **When** read as bool, **Then** `true` is returned.
5. **Given** a key that doesn't exist, **When** read, **Then** `None` is returned (not an error).

---

### User Story 2 - Write Configuration Values (Priority: P1)

As a git user, I need to write configuration values to the appropriate config file so that `git config` commands work.

**Why this priority**: Required for `git config` command and repository initialization.

**Independent Test**: Write a config value with gitr, read it back with C git, verify they match.

**Acceptance Scenarios**:

1. **Given** a key-value pair, **When** written to local config, **Then** `.git/config` is updated and C git reads the same value.
2. **Given** a key-value pair for global scope, **When** written, **Then** `~/.gitconfig` is updated.
3. **Given** a section that doesn't exist, **When** a key is written, **Then** the section is created.
4. **Given** an existing key, **When** updated, **Then** the old value is replaced in-place.
5. **Given** a key to remove, **When** deleted, **Then** the key is removed and the file remains valid.

---

### User Story 3 - Parse Config File Format (Priority: P1)

As a gitr library, I need to parse git's INI-like config format correctly, including all its quirks and extensions.

**Why this priority**: Config parsing is the foundation of the config system.

**Independent Test**: Parse a config file with all syntactic features and verify every value is extracted.

**Acceptance Scenarios**:

1. **Given** a section header `[section "subsection"]`, **When** parsed, **Then** keys are correctly scoped to `section.subsection.key`.
2. **Given** a continued value with `\` at end of line, **When** parsed, **Then** lines are joined.
3. **Given** comments starting with `#` or `;`, **When** parsed, **Then** they are ignored.
4. **Given** quoted string values, **When** parsed, **Then** escape sequences (`\n`, `\t`, `\\`, `\"`) are processed.
5. **Given** a key with no `=` sign, **When** parsed as boolean, **Then** it is treated as `true`.

---

### User Story 4 - Typed Value Access (Priority: P2)

As a gitr developer, I need typed accessors for config values (bool, int, path, color) so that values are correctly interpreted.

**Why this priority**: Many config values have specific type semantics (e.g., size suffixes for integers).

**Independent Test**: Read config values with various types and verify interpretation matches C git.

**Acceptance Scenarios**:

1. **Given** a value "10k", **When** read as integer, **Then** 10240 is returned (k = 1024).
2. **Given** a value "10m", **When** read as integer, **Then** 10485760 is returned.
3. **Given** a value "~/path", **When** read as path, **Then** `~` is expanded to the home directory.
4. **Given** a color spec "red bold", **When** read as color, **Then** the correct ANSI codes are produced.

---

### User Story 5 - Include and Conditional Includes (Priority: P2)

As a git user, I need `include.path` and `includeIf` directives so that config can be modular and context-dependent.

**Why this priority**: Many users rely on conditional includes for work/personal config separation.

**Independent Test**: Create a config with `includeIf.gitdir:~/work/.path` and verify it's only loaded in matching repos.

**Acceptance Scenarios**:

1. **Given** `include.path = extra.config`, **When** config is loaded, **Then** values from extra.config are included.
2. **Given** `includeIf.gitdir:~/work/.path = work.config`, **When** in a repo under ~/work, **Then** work.config is included.
3. **Given** `includeIf.onbranch:main.path = main.config`, **When** on the main branch, **Then** main.config is included.
4. **Given** a circular include, **When** detected, **Then** an error is returned (not infinite recursion).

---

### User Story 6 - Environment Variable Overrides (Priority: P2)

As a gitr library, I need to respect environment variable overrides (GIT_CONFIG_*, GIT_AUTHOR_*, etc.) so that scripts and CI systems work.

**Why this priority**: CI/CD and automation heavily depend on environment overrides.

**Independent Test**: Set GIT_CONFIG_COUNT/KEY/VALUE env vars and verify they override file config.

**Acceptance Scenarios**:

1. **Given** `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=user.name`, `GIT_CONFIG_VALUE_0=Bot`, **When** config is read, **Then** user.name returns "Bot".
2. **Given** `GIT_CONFIG_NOSYSTEM=1`, **When** config loads, **Then** system config is skipped.
3. **Given** `GIT_CONFIG_GLOBAL=/tmp/custom.config`, **When** config loads, **Then** it's used instead of ~/.gitconfig.

---

### User Story 7 - Push and Remote Configuration Keys (Priority: P2)

As a gitr library, I need to correctly interpret push-related and remote-related configuration keys so that `git push` uses the right defaults and remote settings.

**Why this priority**: Push behavior depends heavily on configuration (which branch to push, which remote, refspecs).

**Independent Test**: Set push.default and remote configuration with C git, verify gitr interprets them identically.

**Acceptance Scenarios**:

1. **Given** `push.default = simple`, **When** pushing without refspec, **Then** the current branch is pushed to its upstream tracking branch (only if names match).
2. **Given** `push.default = current`, **When** pushing without refspec, **Then** the current branch is pushed to a branch of the same name on the remote.
3. **Given** `push.default = upstream`, **When** pushing without refspec, **Then** the current branch is pushed to its upstream tracking branch (even if names differ).
4. **Given** `push.default = matching`, **When** pushing without refspec, **Then** all branches with matching names on the remote are pushed.
5. **Given** `push.default = nothing`, **When** pushing without refspec, **Then** the push is refused.
6. **Given** `remote.origin.push = refs/heads/main:refs/heads/prod`, **When** pushing to origin, **Then** the explicit push refspec is used.
7. **Given** `push.followTags = true`, **When** pushing, **Then** annotated tags pointing to pushed commits are also pushed.
8. **Given** `push.autoSetupRemote = true`, **When** pushing a new branch, **Then** `--set-upstream` behavior is automatic.
9. **Given** `remote.origin.url` and `remote.origin.pushUrl`, **When** pushing to origin, **Then** the `pushUrl` is used for push (URL is used for fetch).
10. **Given** `url.<base>.pushInsteadOf = <prefix>`, **When** pushing, **Then** the URL prefix is rewritten for push operations only.

### Edge Cases

- Config file with BOM (byte order mark)
- Section names with unusual characters
- Very long values (> 4KB on a single line)
- Config file with mixed line endings (CR, LF, CRLF)
- Keys with dots in subsection names (e.g., `[url "https://github.com/"] insteadOf`)
- Empty config file
- Config file that is a symlink
- Concurrent writes to the same config file (lock file needed)

## Requirements

### Functional Requirements

- **FR-001**: System MUST read config from all standard scopes: system (/etc/gitconfig), global (~/.gitconfig), local (.git/config), worktree (.git/config.worktree), command-line (-c key=value)
- **FR-002**: System MUST implement scope precedence: command > worktree > local > global > system
- **FR-003**: System MUST parse git's INI-like config format with sections, subsections, comments, continuations, quoting
- **FR-004**: System MUST support typed access: string, bool, int (with k/m/g suffixes), path (~/ expansion), color
- **FR-005**: System MUST support multi-valued keys (return all values)
- **FR-006**: System MUST write config files preserving existing formatting and comments
- **FR-007**: System MUST support `include.path` and `includeIf` directives (gitdir, onbranch, hasconfig:remote.*.url)
- **FR-008**: System MUST support environment variable overrides (GIT_CONFIG_COUNT, GIT_CONFIG_NOSYSTEM, etc.)
- **FR-009**: System MUST use lock files for atomic config writes
- **FR-010**: System MUST support deleting keys and sections
- **FR-011**: System MUST handle case-insensitive section and key names (but case-sensitive subsections)
- **FR-012**: System MUST support `push.default` with values: `nothing`, `current`, `upstream`, `tracking` (deprecated alias), `simple` (default), `matching`
- **FR-013**: System MUST support `push.followTags` (boolean, default false)
- **FR-014**: System MUST support `push.autoSetupRemote` (boolean, default false)
- **FR-015**: System MUST support `remote.<name>.pushUrl` for push-specific URLs
- **FR-016**: System MUST support `remote.<name>.push` refspecs for push
- **FR-017**: System MUST support `url.<base>.insteadOf` and `url.<base>.pushInsteadOf` for URL rewriting
- **FR-018**: System MUST support `branch.<name>.remote` and `branch.<name>.merge` for upstream tracking

### Key Entities

- **ConfigFile**: Parsed representation of a single config file
- **ConfigSet**: Merged view of all config scopes
- **ConfigScope**: Enum of config file scopes (System, Global, Local, Worktree, Command)
- **ConfigEntry**: A single key-value pair with scope and file origin

## Success Criteria

### Measurable Outcomes

- **SC-001**: Config values written by gitr are readable by C git and vice versa
- **SC-002**: All config type conversions match C git behavior (verified with test suite)
- **SC-003**: Include/includeIf directives work identically to C git (verified against C git test suite t1300-config.sh)
- **SC-004**: Config write operations preserve file formatting (comments, whitespace, ordering)
- **SC-005**: Concurrent config access is safe (lock file prevents corruption)

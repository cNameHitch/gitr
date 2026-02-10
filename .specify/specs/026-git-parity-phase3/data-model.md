# Data Model: Full Git CLI Parity (Phase 3)

**Branch**: `026-git-parity-phase3` | **Date**: 2026-02-09

## Entity Definitions

### 1. ColorMode (EXISTS)

**Location**: `git-utils/src/color.rs` — **already implemented**

Represents the three-state color output mode matching git's `--color` flag.

```rust
// Already exists in git-utils/src/color.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}
```

**Resolution cascade** (highest priority first):
1. Command-line `--color=<mode>` flag
2. Per-command config `color.<cmd>` (e.g., `color.diff`)
3. Global config `color.ui`
4. Default: `Auto`

**Validation**: In `Auto` mode, check `std::io::stdout().is_terminal()` and `$TERM != "dumb"`.

---

### 2. ColorSlot

**Location**: `git-utils/src/color.rs`

Defines all color slots used by git with their ANSI escape codes.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
    // Diff colors
    DiffOldNormal,      // red
    DiffNewNormal,      // green
    DiffOldMoved,       // bold magenta
    DiffNewMoved,       // bold cyan
    DiffContext,        // (no color)
    DiffMetaInfo,       // bold
    DiffFragInfo,       // cyan
    DiffFuncInfo,       // (no color)
    DiffOldPlain,       // red
    DiffNewPlain,       // green
    DiffWhitespace,     // red background

    // Status colors
    StatusHeader,       // (no color)
    StatusAdded,        // green
    StatusChanged,      // red
    StatusUntracked,    // red
    StatusBranch,       // (no color)
    StatusNoBranch,     // red

    // Branch colors
    BranchCurrent,      // green
    BranchLocal,        // (no color)
    BranchRemote,       // red
    BranchUpstream,     // blue
    BranchPlain,        // (no color)

    // Log/show colors
    DecorateHead,       // bold cyan
    DecorateBranch,     // bold green
    DecorateRemote,     // bold red
    DecorateTag,        // bold yellow
    DecorateStash,      // bold magenta
    DecorateGrafted,    // bold blue

    // Grep colors
    GrepFilename,       // magenta
    GrepLineNumber,     // green
    GrepSeparator,      // cyan
    GrepMatch,          // bold red

    // General
    Reset,              // \x1b[m
}
```

**Fields**:
- Each slot maps to a default ANSI escape sequence
- Custom colors configurable via `color.<area>.<slot>` config keys
- Color values: `normal`, `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`, `bold`, `dim`, `ul`, `blink`, `reverse`, `strike`, or `#RRGGBB`

---

### 3. ColorConfig

**Location**: `git-utils/src/color.rs`

Manages color settings for all command areas.

```rust
pub struct ColorConfig {
    /// Global color.ui setting
    pub ui: ColorMode,
    /// Per-command overrides (color.diff, color.status, etc.)
    pub commands: HashMap<String, ColorMode>,
    /// Custom slot colors (color.diff.old, color.status.added, etc.)
    pub slots: HashMap<(String, String), AnsiColor>,
}
```

**Methods**:
- `from_config(config: &GitConfig) -> Self` — load from config
- `effective_mode(&self, command: &str, cli_flag: Option<ColorMode>) -> ColorMode` — resolve cascade
- `get_color(&self, slot: ColorSlot) -> &[u8]` — return ANSI bytes for slot

---

### 4. PagerConfig (EXISTS — extend)

**Location**: `git-utils/src/pager.rs` — **core already implemented** (`setup_pager()`, `PagerGuard`, resolution cascade). Needs CLI integration.

Manages pager selection and invocation.

```rust
pub struct PagerConfig {
    /// Resolved pager command
    pub pager: Option<String>,
    /// Whether pager should be used
    pub enabled: bool,
}
```

**Resolution cascade** (highest priority first):
1. `-P`/`--no-pager` flag → disabled
2. `-p`/`--paginate` flag → enabled
3. `$GIT_PAGER` environment variable
4. `core.pager` config
5. Per-command `pager.<cmd>` config override
6. `$PAGER` environment variable
7. Default: `less`

**Commands that auto-page**: `log`, `diff`, `show`, `blame`, `shortlog`, `grep`, `branch`, `tag`, `help`

**Environment setup**: Set `LESS=FRX` and `LV=-c` if not already present before spawning pager.

---

### 5. Gitattributes (Entry) (EXISTS — extend)

**Location**: `git-index/src/attributes.rs` — **parsing and lookup already implemented** (`AttributeStack`, `AttributeValue`, `AttributeRule`, `add_file()`, `get()`, `get_all()`). Needs behavioral extensions (eol, drivers, filters).

Represents a single attribute rule from a `.gitattributes` file.

```rust
#[derive(Debug, Clone)]
pub struct AttributeEntry {
    /// Glob pattern (e.g., "*.txt", "src/**/*.rs")
    pub pattern: BString,
    /// Whether pattern is negated (prefix !)
    pub negated: bool,
    /// Whether pattern applies to directories only (trailing /)
    pub directory_only: bool,
    /// Attribute assignments
    pub attributes: Vec<AttributeAssignment>,
}

#[derive(Debug, Clone)]
pub struct AttributeAssignment {
    pub name: BString,
    pub state: AttributeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeState {
    /// Attribute is set (e.g., `text`)
    Set,
    /// Attribute is unset (e.g., `-text`)
    Unset,
    /// Attribute has a value (e.g., `eol=crlf`)
    Value(BString),
    /// Attribute is unspecified (e.g., `!text`)
    Unspecified,
}
```

**Built-in attribute behaviors**:
| Attribute | Effect |
|-----------|--------|
| `text` | Enable line-ending normalization |
| `text=auto` | Auto-detect text/binary |
| `eol=crlf` | Checkout with CRLF line endings |
| `eol=lf` | Checkout with LF line endings |
| `binary` | Shorthand for `-text -diff -merge` |
| `diff=<driver>` | Use named diff driver |
| `merge=<driver>` | Use named merge driver |
| `filter=<name>` | Apply clean/smudge filter |

**File load order** (later overrides earlier):
1. `$GIT_DIR/info/attributes`
2. `.gitattributes` in working tree (repo root + subdirectories)
3. `core.attributesFile` config (default: `~/.config/git/attributes`)

---

### 6. AttributeStack (EXISTS — extend)

**Location**: `git-index/src/attributes.rs` — **base implementation exists**. Extend with convenience methods.

Manages the hierarchical attribute lookup.

```rust
pub struct AttributeStack {
    /// Global attributes (from core.attributesFile)
    global: Vec<AttributeEntry>,
    /// Per-directory attribute files, keyed by directory path
    directories: HashMap<PathBuf, Vec<AttributeEntry>>,
    /// Info attributes ($GIT_DIR/info/attributes)
    info: Vec<AttributeEntry>,
}
```

**Methods**:
- `new(repo_root: &Path, config: &GitConfig) -> Result<Self>` — load all attribute files
- `attributes_for(&self, path: &Path) -> Vec<(BString, AttributeState)>` — resolve attributes for a path
- `is_binary(&self, path: &Path) -> bool` — check if path is binary
- `eol_for(&self, path: &Path) -> Option<Eol>` — get line-ending config for path
- `diff_driver(&self, path: &Path) -> Option<BString>` — get diff driver name
- `filter_for(&self, path: &Path) -> Option<(BString, BString)>` — get clean/smudge filter commands

---

### 7. Mailmap

**Location**: `git-utils/src/mailmap.rs`

Maps old author/committer identities to canonical forms.

```rust
pub struct Mailmap {
    entries: Vec<MailmapEntry>,
}

struct MailmapEntry {
    /// Canonical name (output)
    canonical_name: Option<BString>,
    /// Canonical email (output)
    canonical_email: BString,
    /// Match on this name (input pattern, if specified)
    match_name: Option<BString>,
    /// Match on this email (input pattern)
    match_email: BString,
}
```

**Mailmap file format** (4 forms):
1. `Canonical Name <canonical@email>` — match by email only
2. `<canonical@email> <match@email>` — map email to email
3. `Canonical Name <canonical@email> <match@email>` — map email, override name
4. `Canonical Name <canonical@email> Match Name <match@email>` — match both name+email

**Methods**:
- `from_file(path: &Path) -> Result<Self>` — parse `.mailmap` file
- `from_config(config: &GitConfig) -> Result<Option<Self>>` — load from `mailmap.file` config
- `lookup(&self, name: &[u8], email: &[u8]) -> (BString, BString)` — return canonical name/email

---

### 8. HookType

**Location**: `git-repository/src/hooks.rs`

Represents a git hook event with its expected arguments and behavior.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    PreCommit,
    PrepareCommitMsg,
    CommitMsg,
    PostCommit,
    PreRebase,
    PostRewrite,
    PostCheckout,
    PostMerge,
    PrePush,
    PreAutoGc,
    ReferenceTransaction,
}
```

**Hook behavior table**:
| Hook | Args | Stdin | Can abort |
|------|------|-------|-----------|
| `pre-commit` | (none) | (none) | Yes (exit non-zero) |
| `prepare-commit-msg` | `<msg-file> [<source> [<sha>]]` | (none) | Yes |
| `commit-msg` | `<msg-file>` | (none) | Yes |
| `post-commit` | (none) | (none) | No |
| `pre-rebase` | `<upstream> [<branch>]` | (none) | Yes |
| `post-rewrite` | `<command>` | `<old-sha> <new-sha>\n` per line | No |
| `post-checkout` | `<old-ref> <new-ref> <branch-flag>` | (none) | No |
| `post-merge` | `<squash-flag>` | (none) | No |
| `pre-push` | `<remote-name> <remote-url>` | ref update lines | Yes |
| `pre-auto-gc` | (none) | (none) | Yes |

---

### 9. HookRunner

**Location**: `git-repository/src/hooks.rs`

Executes hook scripts.

```rust
pub struct HookRunner {
    hooks_path: PathBuf,
}

pub struct HookResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
```

**Methods**:
- `new(repo: &Repository) -> Self` — resolve hooks path from `core.hooksPath` config or `.git/hooks/`
- `hook_exists(&self, hook: HookType) -> bool` — check if hook script exists and is executable
- `run(&self, hook: HookType, args: &[&str], stdin: Option<&[u8]>) -> Result<HookResult>` — execute hook
- `run_or_ok(&self, hook: HookType, args: &[&str], stdin: Option<&[u8]>) -> Result<HookResult>` — execute if exists, return success if not

---

### 10. GpgSigner

**Location**: `git-repository/src/gpg.rs`

Delegates GPG signing to external binary.

```rust
pub struct GpgSigner {
    /// Path to gpg binary (from gpg.program config, default "gpg")
    program: String,
    /// GPG format (openpgp or x509)
    format: GpgFormat,
    /// Signing key (from user.signingKey config)
    key: Option<String>,
}

pub enum GpgFormat {
    OpenPGP,
    X509,
}

pub struct GpgSignature {
    pub signature: Vec<u8>,
}
```

**Methods**:
- `from_config(config: &GitConfig) -> Self` — load config
- `sign(&self, data: &[u8]) -> Result<GpgSignature>` — sign data
- `verify(&self, data: &[u8], signature: &[u8]) -> Result<GpgVerifyResult>` — verify signature

---

### 11. CredentialHelper

**Location**: `git-transport/src/credential.rs`

Implements git's credential helper protocol.

```rust
pub struct CredentialHelper {
    /// Helper commands from credential.helper config
    helpers: Vec<String>,
}

pub struct Credential {
    pub protocol: String,
    pub host: String,
    pub path: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}
```

**Protocol**:
- `fill`: Send partial credential to helper's stdin, read back filled credential
- `approve`: Notify helper that credential was accepted
- `reject`: Notify helper that credential was rejected

**Wire format** (to/from helper stdin/stdout):
```
protocol=https
host=github.com
username=user
password=token

```
(blank line terminates)

**Methods**:
- `from_config(config: &GitConfig) -> Self` — load helper chain
- `fill(&self, credential: &mut Credential) -> Result<()>` — try each helper until one fills
- `approve(&self, credential: &Credential) -> Result<()>` — notify helpers of success
- `reject(&self, credential: &Credential) -> Result<()>` — notify helpers of failure

---

### 12. InteractiveHunkSelector

**Location**: `git-cli/src/interactive.rs`

Manages interactive hunk selection for `-p`/`--patch` modes.

```rust
pub struct InteractiveHunkSelector {
    /// TTY file handle for user input (opened from /dev/tty)
    tty: File,
}

pub enum HunkAction {
    /// Accept this hunk
    Yes,
    /// Reject this hunk
    No,
    /// Quit (reject remaining)
    Quit,
    /// Accept all remaining hunks
    All,
    /// Done (skip remaining)
    Done,
    /// Split this hunk into smaller hunks
    Split,
    /// Manually edit this hunk
    Edit,
}
```

**User prompt format** (matching git):
```
@@ -10,7 +10,8 @@ function_name
 context line
-removed line
+added line
 context line
(1/3) Stage this hunk [y,n,q,a,d,s,e,?]?
```

**Methods**:
- `new() -> Result<Self>` — open `/dev/tty`
- `select_hunks(&self, hunks: &[DiffHunk], prompt: &str) -> Result<Vec<bool>>` — present each hunk, return selection mask
- `split_hunk(&self, hunk: &DiffHunk) -> Result<Vec<DiffHunk>>` — split a hunk into sub-hunks

---

### 13. EditorConfig

**Location**: `git-repository/src/editor.rs` (enhancement to existing)

Resolves the editor for interactive operations.

```rust
pub struct EditorConfig {
    pub command: String,
}
```

**Resolution cascade** (highest priority first):
1. `$GIT_EDITOR`
2. `core.editor` config
3. `$VISUAL`
4. `$EDITOR`
5. Default: `vi`

**Methods**:
- `from_config(config: &GitConfig) -> Self` — resolve editor
- `edit_file(&self, path: &Path) -> Result<()>` — open editor, wait for exit

---

### 14. ConfigInclude (EXISTS)

**Location**: `git-config/src/include.rs` — **fully implemented** (`process_includes()`, `evaluate_condition()`, cycle detection, `MAX_INCLUDE_DEPTH=10`). Only needs E2E test verification.

Handles config file inclusion and conditional includes.

```rust
pub struct IncludeResolver {
    /// Stack of included file paths for cycle detection
    include_stack: Vec<PathBuf>,
    /// Maximum include depth
    max_depth: usize,
}

pub enum IncludeCondition {
    /// [includeIf "gitdir:<pattern>"]
    Gitdir(BString),
    /// [includeIf "gitdir/i:<pattern>"]
    GitdirCaseInsensitive(BString),
    /// [includeIf "onbranch:<pattern>"]
    OnBranch(BString),
    /// [includeIf "hasconfig:remote.*.url:<pattern>"]
    HasConfig(BString),
}
```

**Validation**:
- Max include depth: 10 (matching git)
- Cycle detection: error if same file path appears in include stack
- Path resolution: `~` expands to home dir, relative paths resolve from including file's directory

**Methods**:
- `resolve(&mut self, section: &str, path: &str, repo: &Path) -> Result<Option<PathBuf>>` — resolve include path
- `should_include(&self, condition: &IncludeCondition, repo: &Repository) -> bool` — evaluate condition

---

### 15. MergeStrategyType (Extension)

**Location**: `git-merge/src/lib.rs` (existing, adding Octopus variant)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategyType {
    Ort,        // existing
    Recursive,  // existing
    Ours,       // existing
    Subtree,    // existing
    Octopus,    // NEW: merge 3+ branches simultaneously
}
```

**Octopus strategy behavior**:
- Accepts 3+ merge heads
- Fails if any merge produces conflicts (refuses to proceed)
- Produces a single merge commit with N+1 parents
- Default strategy when merging 3+ branches

---

### 16. RerereDatabase

**Location**: `git-merge/src/rerere.rs`

Reuses recorded conflict resolutions.

```rust
pub struct RerereDatabase {
    /// Path to .git/rr-cache/
    cache_dir: PathBuf,
}

pub struct RerereEntry {
    /// SHA-1 hash of the conflict (computed from normalized conflict markers)
    conflict_id: String,
    /// Recorded resolution
    resolution: Option<Vec<u8>>,
}
```

**Methods**:
- `new(git_dir: &Path) -> Self` — open/create `.git/rr-cache/`
- `record(&self, path: &Path, conflict_content: &[u8]) -> Result<String>` — save conflict, return ID
- `resolve(&self, path: &Path, conflict_content: &[u8]) -> Result<Option<Vec<u8>>>` — look up recorded resolution
- `forget(&self, conflict_id: &str) -> Result<()>` — delete recorded resolution
- `gc(&self, cutoff: Duration) -> Result<usize>` — remove old entries

---

## Relationships

```
ColorConfig ──uses──▶ ColorSlot (enum of all color slots)
ColorConfig ──reads──▶ GitConfig (color.* settings)
PagerConfig ──reads──▶ GitConfig (core.pager, pager.*)
AttributeStack ──contains──▶ AttributeEntry ──contains──▶ AttributeAssignment
AttributeStack ──reads──▶ GitConfig (core.attributesFile)
Mailmap ──contains──▶ MailmapEntry
Mailmap ──reads──▶ GitConfig (mailmap.file)
HookRunner ──dispatches──▶ HookType
HookRunner ──reads──▶ GitConfig (core.hooksPath)
GpgSigner ──reads──▶ GitConfig (gpg.program, user.signingKey, gpg.format)
CredentialHelper ──reads──▶ GitConfig (credential.helper)
ConfigInclude ──evaluates──▶ IncludeCondition
InteractiveHunkSelector ──operates on──▶ DiffHunk (from git-diff)
RerereDatabase ──stores──▶ RerereEntry
MergeOptions ──selects──▶ MergeStrategyType
```

## State Transitions

### Color Mode Resolution

```
CLI --color flag → Per-command config → color.ui → Default(Auto)
                                                        │
                                             ┌──────────┤
                                             ▼          ▼
                                     is_terminal()   !is_terminal()
                                        │                  │
                                        ▼                  ▼
                                   Output color       No color
```

### Pager Lifecycle

```
Command starts → Check auto-page list → Check terminal
                        │                     │
                        ▼                     ▼
                  [in auto list]     [stdout is terminal]
                        │                     │
                        └──────┬──────────────┘
                               ▼
                        Resolve pager binary
                               │
                        Set LESS=FRX, LV=-c
                               │
                        Spawn pager process
                               │
                        Pipe stdout to pager stdin
                               │
                        [command output writes to pipe]
                               │
                        Close pipe → pager exits
```

### Credential Helper Flow

```
Transport needs auth → CredentialHelper.fill()
                              │
              ┌───────────────┤
              ▼               ▼
        [helper fills]   [no helper fills]
              │               │
              ▼               ▼
        Try auth          Prompt user
              │               │
       ┌──────┴───────┐      │
       ▼               ▼     ▼
   [success]       [failure]
       │               │
       ▼               ▼
   .approve()      .reject()
```

### Interactive Hunk Selection

```
Generate diff hunks → Present hunk N of M
                            │
          ┌─────────────────┼────────────────────┐
          ▼                 ▼                     ▼
    [y] Accept       [n] Reject              [s] Split
          │                 │                     │
          ▼                 ▼                     ▼
     Next hunk        Next hunk          Sub-hunks generated
                                               │
                                          Present sub-hunk
                                               │
          ┌────────────────────────────────────┘
          ▼
    [q] Quit → reject remaining
    [a] All → accept remaining
    [d] Done → skip remaining
```

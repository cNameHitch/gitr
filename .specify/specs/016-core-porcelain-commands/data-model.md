# Data Model: Core Porcelain Commands

Porcelain commands are wrappers around library APIs. Key argument structures:

```rust
/// git status output
pub struct StatusResult {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged: Vec<StatusEntry>,
    pub unstaged: Vec<StatusEntry>,
    pub untracked: Vec<BString>,
    pub ignored: Vec<BString>,
    pub conflicts: Vec<BString>,
}

pub struct StatusEntry {
    pub path: BString,
    pub status: FileStatus,
    pub old_path: Option<BString>,  // For renames
}

/// Compute repository status
pub fn compute_status(
    repo: &Repository,
    options: &StatusOptions,
) -> Result<StatusResult, anyhow::Error>;

pub struct StatusOptions {
    pub show_untracked: ShowUntracked,
    pub show_ignored: bool,
    pub pathspec: Option<Pathspec>,
}

pub enum ShowUntracked {
    No,
    Normal,
    All,
}

/// Clone operation
pub fn clone_repository(
    url: &str,
    dest: &Path,
    options: &CloneOptions,
) -> Result<Repository, anyhow::Error>;

pub struct CloneOptions {
    pub bare: bool,
    pub depth: Option<u32>,
    pub branch: Option<String>,
    pub single_branch: bool,
    pub recurse_submodules: bool,
    pub progress: bool,
}

/// Push command orchestration.
///
/// The push command ties together multiple subsystems:
/// 1. Config: read push.default, remote.<name>.*, branch.<name>.*, url rewriting
/// 2. Refs: resolve local branch refs and their upstream tracking
/// 3. Transport: connect to remote's receive-pack service
/// 4. Protocol: execute send-pack exchange (ref updates + thin pack)
/// 5. Hooks: run pre-push hook before sending data
/// 6. Output: display progress via sideband, report per-ref results
pub fn push_command(
    repo: &Repository,
    remote_name: Option<&str>,
    refspecs: &[String],
    options: &PushCommandOptions,
) -> Result<(), anyhow::Error>;

pub struct PushCommandOptions {
    pub force: bool,
    pub force_with_lease: bool,
    /// Specific ref expectations for force-with-lease
    pub force_with_lease_refs: Vec<(String, Option<ObjectId>)>,
    pub delete: bool,
    pub tags: bool,
    pub set_upstream: bool,
    pub atomic: bool,
    pub dry_run: bool,
    pub verbose: bool,
    pub progress: bool,
    pub no_verify: bool,  // skip pre-push hook
    pub push_options: Vec<String>,
}

/// Resolve what to push when no explicit refspec is given.
/// Uses push.default config and branch.<name>.remote/merge settings.
pub fn resolve_push_refspecs(
    repo: &Repository,
    remote_name: &str,
) -> Result<Vec<RefSpec>, anyhow::Error>;
```

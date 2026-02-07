# Data Model: Repository & Setup

## Core Types

```rust
use git_odb::ObjectDatabase;
use git_ref::RefStore;
use git_config::ConfigSet;
use git_index::Index;
use git_hash::HashAlgorithm;

/// The central repository struct tying all subsystems together.
pub struct Repository {
    /// Path to the .git directory
    git_dir: PathBuf,
    /// Path to the working tree (None for bare repos)
    work_tree: Option<PathBuf>,
    /// Path to the common dir (for worktrees; same as git_dir for normal repos)
    common_dir: PathBuf,
    /// Object database
    odb: ObjectDatabase,
    /// Reference store
    refs: Box<dyn RefStore>,
    /// Merged configuration
    config: ConfigSet,
    /// Index (lazy-loaded)
    index: std::cell::OnceCell<Index>,
    /// Hash algorithm
    hash_algo: HashAlgorithm,
    /// Repository kind
    kind: RepositoryKind,
}

/// Type of repository
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryKind {
    /// Normal repo with working tree
    Normal,
    /// Bare repo (no working tree)
    Bare,
    /// Linked worktree
    LinkedWorktree,
}

impl Repository {
    /// Open an existing repository at the given path (or discover from CWD)
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepoError>;

    /// Discover a repository starting from the given directory
    pub fn discover(start: impl AsRef<Path>) -> Result<Self, RepoError>;

    /// Initialize a new repository
    pub fn init(path: impl AsRef<Path>) -> Result<Self, RepoError>;

    /// Initialize a bare repository
    pub fn init_bare(path: impl AsRef<Path>) -> Result<Self, RepoError>;

    // --- Accessors ---

    /// Path to the .git directory
    pub fn git_dir(&self) -> &Path;
    /// Path to the working tree (None for bare repos)
    pub fn work_tree(&self) -> Option<&Path>;
    /// Path to the common directory (shared in worktrees)
    pub fn common_dir(&self) -> &Path;
    /// Repository kind
    pub fn kind(&self) -> RepositoryKind;
    /// Is this a bare repository?
    pub fn is_bare(&self) -> bool;

    // --- Subsystems ---

    /// Access the object database
    pub fn odb(&self) -> &ObjectDatabase;
    /// Access the reference store
    pub fn refs(&self) -> &dyn RefStore;
    /// Access the configuration
    pub fn config(&self) -> &ConfigSet;
    /// Access the index (lazy-loaded)
    pub fn index(&self) -> Result<&Index, RepoError>;
    /// Reload the index from disk
    pub fn reload_index(&self) -> Result<&Index, RepoError>;
    /// Hash algorithm in use
    pub fn hash_algo(&self) -> HashAlgorithm;

    // --- Convenience methods ---

    /// Resolve HEAD to an OID
    pub fn head_oid(&self) -> Result<Option<ObjectId>, RepoError>;
    /// Get the current branch name (None if detached)
    pub fn current_branch(&self) -> Result<Option<RefName>, RepoError>;
    /// Check if this is on an unborn branch (no commits yet)
    pub fn is_unborn(&self) -> Result<bool, RepoError>;
}

/// Discovery result before full repository opening.
pub struct DiscoveredRepo {
    pub git_dir: PathBuf,
    pub work_tree: Option<PathBuf>,
    pub common_dir: PathBuf,
    pub kind: RepositoryKind,
}

/// Discover a git repository starting from the given directory.
pub fn discover_git_dir(start: &Path) -> Result<DiscoveredRepo, RepoError>;

/// Repository initialization options.
pub struct InitOptions {
    pub bare: bool,
    pub default_branch: Option<String>,
    pub template_dir: Option<PathBuf>,
    pub hash_algorithm: HashAlgorithm,
    pub shared: Option<SharedPermission>,
}

pub enum SharedPermission {
    Umask,
    Group,
    All,
    Custom(u32),
}

/// Initialize a new git repository.
pub fn init_repository(path: &Path, options: &InitOptions) -> Result<DiscoveredRepo, RepoError>;

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("not a git repository (or any of the parent directories): {0}")]
    NotFound(PathBuf),

    #[error("invalid git directory: {path}: {reason}")]
    InvalidGitDir { path: PathBuf, reason: String },

    #[error("repository already exists: {0}")]
    AlreadyExists(PathBuf),

    #[error("bare repository has no working tree")]
    BareNoWorkTree,

    #[error("unable to read HEAD: {0}")]
    InvalidHead(String),

    #[error(transparent)]
    Config(#[from] git_config::ConfigError),

    #[error(transparent)]
    Odb(#[from] git_odb::OdbError),

    #[error(transparent)]
    Ref(#[from] git_ref::RefError),

    #[error(transparent)]
    Index(#[from] git_index::IndexError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

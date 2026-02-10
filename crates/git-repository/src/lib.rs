//! Repository discovery, initialization, and central access for all git subsystems.

mod discover;
pub mod editor;
mod env;
mod error;
pub mod gpg;
pub mod hooks;
mod init;
mod worktree;

pub use error::RepoError;

use std::path::{Path, PathBuf};

use git_config::ConfigSet;
use git_hash::{HashAlgorithm, ObjectId};
use git_index::Index;
use git_odb::ObjectDatabase;
use git_ref::{FilesRefStore, RefName, RefStore, Reference};

/// Type of repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryKind {
    /// Normal repo with a working tree.
    Normal,
    /// Bare repo (no working tree).
    Bare,
    /// Linked worktree sharing objects/refs with a main repo.
    LinkedWorktree,
}

/// Result of repository discovery before full opening.
#[derive(Debug)]
pub struct DiscoveredRepo {
    pub git_dir: PathBuf,
    pub work_tree: Option<PathBuf>,
    pub common_dir: PathBuf,
    pub kind: RepositoryKind,
}

/// Options for repository initialization.
pub struct InitOptions {
    pub bare: bool,
    pub default_branch: Option<String>,
    pub template_dir: Option<PathBuf>,
    pub hash_algorithm: HashAlgorithm,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            bare: false,
            default_branch: None,
            template_dir: None,
            hash_algorithm: HashAlgorithm::Sha1,
        }
    }
}

/// The central repository struct tying all subsystems together.
impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("git_dir", &self.git_dir)
            .field("work_tree", &self.work_tree)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

pub struct Repository {
    /// Path to the .git directory.
    git_dir: PathBuf,
    /// Path to the working tree (None for bare repos).
    work_tree: Option<PathBuf>,
    /// Path to the common dir (for worktrees; same as git_dir for normal repos).
    common_dir: PathBuf,
    /// Object database.
    odb: ObjectDatabase,
    /// Reference store.
    refs: FilesRefStore,
    /// Merged configuration.
    config: ConfigSet,
    /// Index (lazy-loaded). None means not yet loaded.
    index: Option<Index>,
    /// Path to the index file.
    index_path: PathBuf,
    /// Hash algorithm.
    hash_algo: HashAlgorithm,
    /// Repository kind.
    kind: RepositoryKind,
}

impl Repository {
    /// Open an existing repository at the given path.
    ///
    /// `path` should point to either the `.git` directory or the working tree root.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepoError> {
        let path = path.as_ref();
        let discovered = if path.join("HEAD").is_file() && path.join("objects").is_dir() {
            // Path is a git dir (bare repo or .git directory)
            discover::open_git_dir(path)?
        } else if path.join(".git").exists() {
            // Path is a working tree root
            discover::open_git_dir_from_work_tree(path)?
        } else {
            return Err(RepoError::NotFound(path.to_path_buf()));
        };
        Self::from_discovered(discovered)
    }

    /// Discover a repository starting from the given directory, walking up.
    pub fn discover(start: impl AsRef<Path>) -> Result<Self, RepoError> {
        let discovered = discover::discover_git_dir(start.as_ref())?;
        Self::from_discovered(discovered)
    }

    /// Initialize a new repository at the given path.
    pub fn init(path: impl AsRef<Path>) -> Result<Self, RepoError> {
        let opts = InitOptions::default();
        let discovered = init::init_repository(path.as_ref(), &opts)?;
        Self::from_discovered(discovered)
    }

    /// Initialize a new bare repository at the given path.
    pub fn init_bare(path: impl AsRef<Path>) -> Result<Self, RepoError> {
        let opts = InitOptions {
            bare: true,
            ..Default::default()
        };
        let discovered = init::init_repository(path.as_ref(), &opts)?;
        Self::from_discovered(discovered)
    }

    /// Initialize a new repository with custom options.
    pub fn init_opts(path: impl AsRef<Path>, opts: &InitOptions) -> Result<Self, RepoError> {
        let discovered = init::init_repository(path.as_ref(), opts)?;
        Self::from_discovered(discovered)
    }

    /// Build a Repository from a DiscoveredRepo.
    fn from_discovered(discovered: DiscoveredRepo) -> Result<Self, RepoError> {
        let env_overrides = env::EnvOverrides::from_env();
        Self::from_discovered_with_env(discovered, &env_overrides)
    }

    /// Build a Repository from a DiscoveredRepo with explicit environment overrides.
    fn from_discovered_with_env(
        discovered: DiscoveredRepo,
        env_overrides: &env::EnvOverrides,
    ) -> Result<Self, RepoError> {
        let DiscoveredRepo {
            git_dir,
            work_tree,
            common_dir,
            kind,
        } = discovered;

        // Apply env overrides for work tree
        let work_tree = if let Some(ref wt) = env_overrides.work_tree {
            Some(wt.clone())
        } else {
            work_tree
        };

        // Apply env override for common dir
        let common_dir = if let Some(ref cd) = env_overrides.common_dir {
            cd.clone()
        } else {
            common_dir
        };

        // Determine objects directory
        let objects_dir = if let Some(ref od) = env_overrides.object_directory {
            od.clone()
        } else {
            common_dir.join("objects")
        };

        let odb = ObjectDatabase::open(&objects_dir)?;

        // Load config
        let config = ConfigSet::load(Some(&git_dir))?;

        // Determine hash algorithm from config or default
        let hash_algo = match config.get_string("extensions.objectformat") {
            Ok(Some(ref name)) => {
                HashAlgorithm::from_name(name).unwrap_or(HashAlgorithm::Sha1)
            }
            _ => HashAlgorithm::Sha1,
        };

        // Set up ref store from common_dir (refs are shared in worktrees)
        let refs = FilesRefStore::new(&common_dir);

        // Determine index file path
        let index_path = if let Some(ref idx) = env_overrides.index_file {
            idx.clone()
        } else {
            git_dir.join("index")
        };

        Ok(Repository {
            git_dir,
            work_tree,
            common_dir,
            odb,
            refs,
            config,
            index: None,
            index_path,
            hash_algo,
            kind,
        })
    }

    // --- Path accessors ---

    /// Path to the .git directory.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// Path to the working tree (None for bare repos).
    pub fn work_tree(&self) -> Option<&Path> {
        self.work_tree.as_deref()
    }

    /// Path to the common directory (shared in worktrees).
    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// Repository kind.
    pub fn kind(&self) -> RepositoryKind {
        self.kind
    }

    /// Is this a bare repository?
    pub fn is_bare(&self) -> bool {
        self.kind == RepositoryKind::Bare
    }

    // --- Subsystem accessors ---

    /// Access the object database.
    pub fn odb(&self) -> &ObjectDatabase {
        &self.odb
    }

    /// Access the reference store.
    pub fn refs(&self) -> &FilesRefStore {
        &self.refs
    }

    /// Access the configuration.
    pub fn config(&self) -> &ConfigSet {
        &self.config
    }

    /// Access the configuration mutably.
    pub fn config_mut(&mut self) -> &mut ConfigSet {
        &mut self.config
    }

    /// Access the index (lazy-loaded).
    pub fn index(&mut self) -> Result<&Index, RepoError> {
        if self.index.is_none() {
            self.load_index()?;
        }
        Ok(self.index.as_ref().unwrap())
    }

    /// Access the index mutably (lazy-loaded).
    pub fn index_mut(&mut self) -> Result<&mut Index, RepoError> {
        if self.index.is_none() {
            self.load_index()?;
        }
        Ok(self.index.as_mut().unwrap())
    }

    /// Replace the cached index with the given one.
    pub fn set_index(&mut self, index: Index) {
        self.index = Some(index);
    }

    /// Write the current in-memory index back to disk.
    pub fn write_index(&self) -> Result<(), RepoError> {
        if let Some(ref idx) = self.index {
            idx.write_to(&self.index_path)?;
        }
        Ok(())
    }

    /// Reload the index from disk, replacing any cached copy.
    pub fn reload_index(&mut self) -> Result<&Index, RepoError> {
        self.index = None;
        self.load_index()?;
        Ok(self.index.as_ref().unwrap())
    }

    fn load_index(&mut self) -> Result<(), RepoError> {
        let idx = if self.index_path.exists() {
            Index::read_from(&self.index_path)?
        } else {
            Index::new()
        };
        self.index = Some(idx);
        Ok(())
    }

    /// Hash algorithm in use.
    pub fn hash_algo(&self) -> HashAlgorithm {
        self.hash_algo
    }

    // --- Convenience methods ---

    /// Resolve HEAD to an OID.
    pub fn head_oid(&self) -> Result<Option<ObjectId>, RepoError> {
        let head_ref = RefName::new("HEAD").map_err(RepoError::from)?;
        let resolved = self.refs.resolve_to_oid(&head_ref)?;
        Ok(resolved)
    }

    /// Get the current branch name (None if detached HEAD).
    pub fn current_branch(&self) -> Result<Option<String>, RepoError> {
        let head_ref = RefName::new("HEAD").map_err(RepoError::from)?;
        match self.refs.resolve(&head_ref)? {
            Some(Reference::Symbolic { target, .. }) => {
                let name = target.as_str();
                // Strip refs/heads/ prefix if present
                let branch = name
                    .strip_prefix("refs/heads/")
                    .unwrap_or(name);
                Ok(Some(branch.to_string()))
            }
            Some(Reference::Direct { .. }) => Ok(None), // detached HEAD
            None => Ok(None),
        }
    }

    /// Check if this is on an unborn branch (no commits yet).
    pub fn is_unborn(&self) -> Result<bool, RepoError> {
        let head_ref = RefName::new("HEAD").map_err(RepoError::from)?;
        match self.refs.resolve(&head_ref)? {
            Some(Reference::Symbolic { target, .. }) => {
                // HEAD points to a symbolic ref; check if that ref exists
                let resolved = self.refs.resolve_to_oid(&target)?;
                Ok(resolved.is_none())
            }
            Some(Reference::Direct { .. }) => Ok(false),
            None => Ok(true),
        }
    }
}

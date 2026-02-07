# Data Model: Advanced Features

## Key Types

```rust
/// GC configuration.
pub struct GcConfig {
    pub auto: u32,              // gc.auto, default 6700
    pub auto_pack_limit: u32,   // gc.autoPackLimit, default 50
    pub prune_expire: String,   // gc.pruneExpire, default "2.weeks.ago"
    pub aggressive_depth: u32,  // gc.aggressiveDepth, default 50
    pub aggressive_window: u32, // gc.aggressiveWindow, default 250
}

/// Run garbage collection.
pub fn gc(repo: &Repository, options: &GcOptions) -> Result<GcResult, anyhow::Error>;

pub struct GcOptions {
    pub aggressive: bool,
    pub auto: bool,
    pub prune: Option<String>,  // Expiry date
    pub quiet: bool,
}

pub struct GcResult {
    pub objects_packed: u32,
    pub packs_removed: u32,
    pub objects_pruned: u32,
}

/// Fsck result.
pub struct FsckResult {
    pub errors: Vec<FsckError>,
    pub dangling: Vec<(ObjectType, ObjectId)>,
    pub unreachable: Vec<(ObjectType, ObjectId)>,
}

pub struct FsckError {
    pub oid: ObjectId,
    pub error_type: FsckErrorType,
    pub message: String,
}

pub enum FsckErrorType {
    MissingObject,
    CorruptObject,
    InvalidTree,
    InvalidCommit,
    InvalidTag,
    DuplicateEntry,
    BadTimestamp,
}

/// Submodule configuration.
pub struct SubmoduleConfig {
    pub name: String,
    pub path: BString,
    pub url: String,
    pub branch: Option<String>,
    pub update: SubmoduleUpdateStrategy,
}

pub enum SubmoduleUpdateStrategy {
    Checkout,
    Rebase,
    Merge,
    None,
}

/// Hook runner.
pub struct HookRunner {
    hooks_dir: PathBuf,
}

impl HookRunner {
    pub fn new(hooks_dir: PathBuf) -> Self;
    /// Run a hook if it exists. Returns Ok(true) if hook ran and succeeded,
    /// Ok(false) if hook doesn't exist, Err if hook failed.
    pub fn run(&self, hook_name: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<bool, anyhow::Error>;
}

/// Archive generation.
pub fn create_archive(
    repo: &Repository,
    tree_oid: &ObjectId,
    options: &ArchiveOptions,
    output: &mut dyn std::io::Write,
) -> Result<(), anyhow::Error>;

pub struct ArchiveOptions {
    pub format: ArchiveFormat,
    pub prefix: Option<String>,
    pub compression: Option<u32>,
}

pub enum ArchiveFormat {
    Tar,
    TarGz,
    Zip,
}

/// GPG signing interface.
pub fn sign_buffer(content: &[u8], key: Option<&str>) -> Result<Vec<u8>, anyhow::Error>;
pub fn verify_signature(content: &[u8], signature: &[u8]) -> Result<SignatureVerification, anyhow::Error>;

pub struct SignatureVerification {
    pub valid: bool,
    pub signer: Option<String>,
    pub key_id: Option<String>,
    pub trust_level: TrustLevel,
}

pub enum TrustLevel {
    Unknown,
    Never,
    Marginal,
    Full,
    Ultimate,
}
```

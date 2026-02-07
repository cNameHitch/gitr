# Data Model: History & Inspection Commands

## Key Types

```rust
/// Blame result for a file.
pub struct BlameResult {
    pub entries: Vec<BlameEntry>,
}

pub struct BlameEntry {
    pub commit: ObjectId,
    pub original_path: BString,
    pub original_line: u32,
    pub final_line: u32,
    pub num_lines: u32,
    pub author: Signature,
    pub summary: BString,
}

/// Compute blame for a file at a given revision.
pub fn blame(
    repo: &Repository,
    path: &BStr,
    revision: &ObjectId,
    options: &BlameOptions,
) -> Result<BlameResult, anyhow::Error>;

pub struct BlameOptions {
    pub line_range: Option<(u32, u32)>,
    pub detect_copies: bool,
    pub ignore_whitespace: bool,
}

/// Bisect state machine.
pub struct BisectState {
    pub good: Vec<ObjectId>,
    pub bad: Option<ObjectId>,
    pub original_head: ObjectId,
}

impl BisectState {
    pub fn load(repo: &Repository) -> Result<Option<Self>, anyhow::Error>;
    pub fn save(&self, repo: &Repository) -> Result<(), anyhow::Error>;
    pub fn next_commit(&self, repo: &Repository) -> Result<Option<ObjectId>, anyhow::Error>;
    pub fn is_done(&self) -> bool;
}

/// Describe result.
pub struct DescribeResult {
    pub tag: Option<String>,
    pub distance: u32,
    pub oid: ObjectId,
    pub dirty: bool,
}

impl DescribeResult {
    pub fn to_string(&self, long: bool) -> String;
}

/// Format-patch output.
pub fn format_patch(
    repo: &Repository,
    commits: &[ObjectId],
    options: &FormatPatchOptions,
) -> Result<Vec<PathBuf>, anyhow::Error>;

pub struct FormatPatchOptions {
    pub output_dir: PathBuf,
    pub numbered: bool,
    pub cover_letter: bool,
    pub thread: bool,
}
```

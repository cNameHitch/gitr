use std::path::PathBuf;

/// Environment variable overrides for repository operations.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct EnvOverrides {
    /// GIT_DIR override
    pub git_dir: Option<PathBuf>,
    /// GIT_WORK_TREE override
    pub work_tree: Option<PathBuf>,
    /// GIT_CEILING_DIRECTORIES (colon-separated)
    pub ceiling_directories: Vec<PathBuf>,
    /// GIT_OBJECT_DIRECTORY override
    pub object_directory: Option<PathBuf>,
    /// GIT_ALTERNATE_OBJECT_DIRECTORIES (colon-separated)
    pub alternate_object_directories: Vec<PathBuf>,
    /// GIT_COMMON_DIR override
    pub common_dir: Option<PathBuf>,
    /// GIT_INDEX_FILE override
    pub index_file: Option<PathBuf>,
}

impl EnvOverrides {
    /// Read all standard git environment variables.
    pub fn from_env() -> Self {
        Self {
            git_dir: std::env::var_os("GIT_DIR").map(PathBuf::from),
            work_tree: std::env::var_os("GIT_WORK_TREE").map(PathBuf::from),
            ceiling_directories: parse_path_list_env("GIT_CEILING_DIRECTORIES"),
            object_directory: std::env::var_os("GIT_OBJECT_DIRECTORY").map(PathBuf::from),
            alternate_object_directories: parse_path_list_env(
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            ),
            common_dir: std::env::var_os("GIT_COMMON_DIR").map(PathBuf::from),
            index_file: std::env::var_os("GIT_INDEX_FILE").map(PathBuf::from),
        }
    }
}

/// Parse a colon-separated (or semicolon on Windows) path list from an env var.
fn parse_path_list_env(var: &str) -> Vec<PathBuf> {
    match std::env::var_os(var) {
        Some(val) => {
            let s = val.to_string_lossy();
            let sep = if cfg!(windows) { ';' } else { ':' };
            s.split(sep)
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .collect()
        }
        None => Vec::new(),
    }
}

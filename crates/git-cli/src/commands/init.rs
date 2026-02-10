use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use git_repository::InitOptions;

use crate::Cli;

#[derive(Args)]
pub struct InitArgs {
    /// Create a bare repository
    #[arg(long)]
    bare: bool,

    /// Override the name of the initial branch
    #[arg(short = 'b', long, value_name = "branch-name")]
    initial_branch: Option<String>,

    /// Directory from which templates will be used
    #[arg(long, value_name = "template-directory")]
    template: Option<PathBuf>,

    /// Specify the hash algorithm to use
    #[arg(long, value_name = "hash")]
    object_format: Option<String>,

    /// Be quiet, only report errors
    #[arg(short, long)]
    quiet: bool,

    /// Directory to create the repository in
    directory: Option<PathBuf>,
}

pub fn run(args: &InitArgs, _cli: &Cli) -> Result<i32> {
    let target = match &args.directory {
        Some(dir) => dir.clone(),
        None => std::env::current_dir()?,
    };

    // Create target directory and parents if needed (FR-006)
    if !target.exists() {
        std::fs::create_dir_all(&target)?;
    }

    let hash_algo = if let Some(ref fmt) = args.object_format {
        git_hash::HashAlgorithm::from_name(fmt)
            .ok_or_else(|| anyhow::anyhow!("unknown hash algorithm '{}'", fmt))?
    } else {
        git_hash::HashAlgorithm::Sha1
    };

    let opts = InitOptions {
        bare: args.bare,
        default_branch: args.initial_branch.clone(),
        template_dir: args.template.clone(),
        hash_algorithm: hash_algo,
    };

    let repo = git_repository::Repository::init_opts(&target, &opts)?;

    if !args.quiet {
        let stderr = io::stderr();
        let mut err = stderr.lock();
        let git_dir = std::fs::canonicalize(repo.git_dir()).unwrap_or_else(|_| repo.git_dir().to_path_buf());
        let mut display_path = git_dir.display().to_string();
        if !display_path.ends_with('/') {
            display_path.push('/');
        }
        writeln!(err, "Initialized empty Git repository in {}", display_path)?;
    }

    Ok(0)
}

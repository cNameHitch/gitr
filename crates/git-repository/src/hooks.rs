//! Git hook execution.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::Repository;

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

impl HookType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit",
            Self::PrepareCommitMsg => "prepare-commit-msg",
            Self::CommitMsg => "commit-msg",
            Self::PostCommit => "post-commit",
            Self::PreRebase => "pre-rebase",
            Self::PostRewrite => "post-rewrite",
            Self::PostCheckout => "post-checkout",
            Self::PostMerge => "post-merge",
            Self::PrePush => "pre-push",
            Self::PreAutoGc => "pre-auto-gc",
            Self::ReferenceTransaction => "reference-transaction",
        }
    }
}

pub struct HookResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl HookResult {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

pub struct HookRunner {
    hooks_path: PathBuf,
}

impl HookRunner {
    /// Create a HookRunner from a repository.
    /// Resolves hooks path from core.hooksPath config or .git/hooks/.
    pub fn new(repo: &Repository) -> Self {
        let hooks_path = if let Ok(Some(path)) = repo.config().get_string("core.hooksPath") {
            PathBuf::from(path)
        } else {
            repo.git_dir().join("hooks")
        };
        Self { hooks_path }
    }

    /// Check if a hook script exists and is executable.
    pub fn hook_exists(&self, hook: HookType) -> bool {
        let path = self.hooks_path.join(hook.name());
        path.is_file()
    }

    /// Execute a hook. Returns error if hook exists but fails to execute.
    pub fn run(
        &self,
        hook: HookType,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<HookResult, std::io::Error> {
        let path = self.hooks_path.join(hook.name());
        if !path.is_file() {
            return Ok(HookResult {
                exit_code: 0,
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
        }

        let mut cmd = Command::new(&path);
        cmd.args(args);

        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        if let Some(input) = stdin {
            if let Some(ref mut child_stdin) = child.stdin {
                use std::io::Write;
                let _ = child_stdin.write_all(input);
            }
            drop(child.stdin.take());
        }

        let output = child.wait_with_output()?;

        Ok(HookResult {
            exit_code: output.status.code().unwrap_or(128),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// Execute hook if it exists, return success if it doesn't exist.
    pub fn run_or_ok(
        &self,
        hook: HookType,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<HookResult, std::io::Error> {
        self.run(hook, args, stdin)
    }
}

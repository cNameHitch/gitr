use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

use crate::error::UtilError;
use crate::Result;

/// Stdio mode for subprocess streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioMode {
    /// Inherit the parent process's stream.
    Inherit,
    /// Pipe the stream (capture it).
    Pipe,
    /// Redirect to /dev/null.
    Null,
}

impl StdioMode {
    fn to_stdio(self) -> Stdio {
        match self {
            StdioMode::Inherit => Stdio::inherit(),
            StdioMode::Pipe => Stdio::piped(),
            StdioMode::Null => Stdio::null(),
        }
    }
}

/// Result of running a subprocess.
#[derive(Debug)]
pub struct GitCommandResult {
    /// The exit status.
    pub status: ExitStatus,
    /// Captured stdout (empty if not piped).
    pub stdout: Vec<u8>,
    /// Captured stderr (empty if not piped).
    pub stderr: Vec<u8>,
}

impl GitCommandResult {
    /// Returns true if the process exited successfully.
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

/// Builder for subprocess execution.
///
/// Wraps `std::process::Command` with a fluent API and adds timeout support.
pub struct GitCommand {
    program: OsString,
    args: Vec<OsString>,
    env_vars: Vec<(OsString, OsString)>,
    stdin_mode: StdioMode,
    stdout_mode: StdioMode,
    stderr_mode: StdioMode,
    working_dir: Option<PathBuf>,
    timeout: Option<Duration>,
}

impl GitCommand {
    /// Create a new command builder for the given program.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            env_vars: Vec::new(),
            stdin_mode: StdioMode::Inherit,
            stdout_mode: StdioMode::Inherit,
            stderr_mode: StdioMode::Inherit,
            working_dir: None,
            timeout: None,
        }
    }

    /// Add an argument.
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// Add multiple arguments.
    pub fn args(mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        for arg in args {
            self.args.push(arg.as_ref().to_os_string());
        }
        self
    }

    /// Set an environment variable.
    pub fn env(mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> Self {
        self.env_vars
            .push((key.as_ref().to_os_string(), val.as_ref().to_os_string()));
        self
    }

    /// Set stdin mode.
    pub fn stdin(mut self, mode: StdioMode) -> Self {
        self.stdin_mode = mode;
        self
    }

    /// Set stdout mode.
    pub fn stdout(mut self, mode: StdioMode) -> Self {
        self.stdout_mode = mode;
        self
    }

    /// Set stderr mode.
    pub fn stderr(mut self, mode: StdioMode) -> Self {
        self.stderr_mode = mode;
        self
    }

    /// Set the working directory.
    pub fn working_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.working_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Set a timeout for the command.
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    /// Build the underlying `std::process::Command`.
    fn build_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        for (key, val) in &self.env_vars {
            cmd.env(key, val);
        }
        cmd.stdin(self.stdin_mode.to_stdio());
        cmd.stdout(self.stdout_mode.to_stdio());
        cmd.stderr(self.stderr_mode.to_stdio());
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }
        cmd
    }

    /// Get the command string for error messages.
    fn command_string(&self) -> String {
        let mut s = self.program.to_string_lossy().to_string();
        for arg in &self.args {
            s.push(' ');
            s.push_str(&arg.to_string_lossy());
        }
        s
    }

    /// Run the command and wait for it to complete, capturing output.
    pub fn run(&self) -> Result<GitCommandResult> {
        let mut cmd = self.build_command();
        let cmd_str = self.command_string();

        let mut child = cmd.spawn().map_err(|e| UtilError::Subprocess {
            command: cmd_str.clone(),
            source: e,
        })?;

        if let Some(timeout) = self.timeout {
            // Wait with timeout
            match child.try_wait() {
                Ok(Some(_)) => {
                    let output = child.wait_with_output().map_err(|e| {
                        UtilError::Subprocess {
                            command: cmd_str.clone(),
                            source: e,
                        }
                    })?;
                    Ok(GitCommandResult {
                        status: output.status,
                        stdout: output.stdout,
                        stderr: output.stderr,
                    })
                }
                Ok(None) => {
                    // Not finished yet, wait with timeout
                    let start = std::time::Instant::now();
                    loop {
                        std::thread::sleep(Duration::from_millis(10));
                        match child.try_wait() {
                            Ok(Some(_)) => {
                                // Collect remaining output
                                let output = child.wait_with_output().map_err(|e| {
                                    UtilError::Subprocess {
                                        command: cmd_str.clone(),
                                        source: e,
                                    }
                                })?;
                                return Ok(GitCommandResult {
                                    status: output.status,
                                    stdout: output.stdout,
                                    stderr: output.stderr,
                                });
                            }
                            Ok(None) => {
                                if start.elapsed() > timeout {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    return Err(UtilError::SubprocessTimeout {
                                        command: cmd_str,
                                    });
                                }
                            }
                            Err(e) => {
                                return Err(UtilError::Subprocess {
                                    command: cmd_str,
                                    source: e,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    Err(UtilError::Subprocess {
                        command: cmd_str,
                        source: e,
                    })
                }
            }
        } else {
            // No timeout, just wait
            let output =
                child
                    .wait_with_output()
                    .map_err(|e| UtilError::Subprocess {
                        command: cmd_str,
                        source: e,
                    })?;
            Ok(GitCommandResult {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
            })
        }
    }

    /// Spawn the command without waiting for it to complete.
    pub fn spawn(&self) -> Result<Child> {
        let mut cmd = self.build_command();
        let cmd_str = self.command_string();
        cmd.spawn().map_err(|e| UtilError::Subprocess {
            command: cmd_str,
            source: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_echo() {
        let result = GitCommand::new("echo")
            .arg("hello")
            .stdout(StdioMode::Pipe)
            .stderr(StdioMode::Pipe)
            .run()
            .unwrap();

        assert!(result.success());
        assert_eq!(result.stdout.trim_ascii(), b"hello");
    }

    #[test]
    fn capture_stderr() {
        let result = GitCommand::new("sh")
            .arg("-c")
            .arg("echo error >&2")
            .stdout(StdioMode::Pipe)
            .stderr(StdioMode::Pipe)
            .run()
            .unwrap();

        assert!(result.success());
        assert_eq!(result.stderr.trim_ascii(), b"error");
    }

    #[test]
    fn exit_code() {
        let result = GitCommand::new("sh")
            .arg("-c")
            .arg("exit 42")
            .stdout(StdioMode::Pipe)
            .stderr(StdioMode::Pipe)
            .run()
            .unwrap();

        assert!(!result.success());
        assert_eq!(result.status.code(), Some(42));
    }

    #[test]
    fn working_directory() {
        let result = GitCommand::new("pwd")
            .stdout(StdioMode::Pipe)
            .working_dir("/tmp")
            .run()
            .unwrap();

        assert!(result.success());
        // On macOS, /tmp is a symlink to /private/tmp
        let output = String::from_utf8_lossy(&result.stdout);
        assert!(
            output.trim() == "/tmp" || output.trim() == "/private/tmp",
            "unexpected pwd output: {}",
            output.trim()
        );
    }

    #[test]
    fn environment_variable() {
        let result = GitCommand::new("sh")
            .arg("-c")
            .arg("echo $MY_TEST_VAR")
            .env("MY_TEST_VAR", "hello_from_test")
            .stdout(StdioMode::Pipe)
            .run()
            .unwrap();

        assert!(result.success());
        assert_eq!(result.stdout.trim_ascii(), b"hello_from_test");
    }

    #[test]
    fn timeout_succeeds() {
        let result = GitCommand::new("echo")
            .arg("fast")
            .stdout(StdioMode::Pipe)
            .timeout(Duration::from_secs(5))
            .run()
            .unwrap();

        assert!(result.success());
    }

    #[test]
    fn spawn_and_wait() {
        let mut child = GitCommand::new("echo")
            .arg("spawned")
            .stdout(StdioMode::Null)
            .spawn()
            .unwrap();

        let status = child.wait().unwrap();
        assert!(status.success());
    }

    #[test]
    fn pipe_stdin() {
        use std::io::Write;

        let mut child = GitCommand::new("cat")
            .stdin(StdioMode::Pipe)
            .stdout(StdioMode::Pipe)
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"piped input").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"piped input");
    }
}

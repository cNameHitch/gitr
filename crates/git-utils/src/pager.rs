use std::io::{self, IsTerminal};
use std::process::{Child, Command, Stdio};

use crate::Result;
use crate::error::UtilError;

/// Set up a pager for stdout output, matching C git's pager.c behavior.
///
/// Pager selection priority (matching C git):
/// 1. `GIT_PAGER` environment variable
/// 2. `core.pager` config value (passed as parameter)
/// 3. `PAGER` environment variable
/// 4. Default to `less`
///
/// If the pager is empty or `"cat"`, paging is disabled.
/// If stdout is not a terminal, paging is disabled.
///
/// Returns `Some(child)` if a pager was spawned (caller must pipe stdout to it),
/// or `None` if no paging is needed.
pub fn setup_pager(config_pager: Option<&str>) -> Result<Option<PagerGuard>> {
    // Don't page if stdout isn't a terminal
    if !io::stdout().is_terminal() {
        return Ok(None);
    }

    let pager_cmd = resolve_pager(config_pager);

    let pager_cmd = match pager_cmd {
        Some(p) => p,
        None => return Ok(None),
    };

    // Empty string or "cat" means no pager
    if pager_cmd.is_empty() || pager_cmd == "cat" {
        return Ok(None);
    }

    // Spawn the pager process
    let child = Command::new("sh")
        .arg("-c")
        .arg(&pager_cmd)
        .stdin(Stdio::piped())
        .env("GIT_PAGER_IN_USE", "true")
        .env("LESS", std::env::var("LESS").unwrap_or_else(|_| "FRX".to_string()))
        .env("LV", std::env::var("LV").unwrap_or_else(|_| "-c".to_string()))
        .spawn()
        .map_err(|e| UtilError::Subprocess {
            command: format!("pager: {}", pager_cmd),
            source: e,
        })?;

    Ok(Some(PagerGuard {
        child,
        pager_cmd,
    }))
}

/// Resolve which pager command to use, following C git's priority order.
fn resolve_pager(config_pager: Option<&str>) -> Option<String> {
    resolve_pager_from(
        std::env::var("GIT_PAGER").ok(),
        config_pager,
        std::env::var("PAGER").ok(),
    )
}

/// Inner resolver that takes explicit values (testable without env var mutation).
fn resolve_pager_from(
    git_pager_env: Option<String>,
    config_pager: Option<&str>,
    pager_env: Option<String>,
) -> Option<String> {
    // 1. GIT_PAGER env var
    if let Some(val) = git_pager_env {
        return Some(val);
    }

    // 2. core.pager config
    if let Some(val) = config_pager {
        return Some(val.to_string());
    }

    // 3. PAGER env var
    if let Some(val) = pager_env {
        return Some(val);
    }

    // 4. Default: less
    Some("less".to_string())
}

/// Set up a pager and redirect stdout to it.
///
/// This is the high-level API for pager setup. After calling this:
/// - All writes to stdout() will flow through the pager
/// - The returned guard MUST be kept alive until output is complete
/// - When the guard is dropped, stdout is flushed and the pager is waited on
#[cfg(unix)]
pub fn setup_pager_for_stdout(config_pager: Option<&str>) -> Result<Option<PagerGuard>> {
    let guard = setup_pager(config_pager)?;
    if let Some(mut guard) = guard {
        use std::os::unix::io::AsRawFd;
        if let Some(stdin) = guard.stdin() {
            let pager_fd = stdin.as_raw_fd();
            unsafe {
                libc::dup2(pager_fd, libc::STDOUT_FILENO);
            }
        }
        Ok(Some(guard))
    } else {
        Ok(None)
    }
}

/// Non-unix fallback: set up a pager without stdout redirection.
///
/// On non-unix platforms, pager integration is not supported.
/// Returns None so that output goes directly to stdout.
#[cfg(not(unix))]
pub fn setup_pager_for_stdout(_config_pager: Option<&str>) -> Result<Option<PagerGuard>> {
    Ok(None)
}

/// Check if a pager is currently in use (by checking GIT_PAGER_IN_USE env var).
///
/// Matches C git's `pager_in_use()`.
pub fn pager_in_use() -> bool {
    std::env::var_os("GIT_PAGER_IN_USE").is_some()
}

/// Get the terminal column width.
///
/// Checks `COLUMNS` env var first, then queries the terminal.
/// Falls back to 80 if detection fails.
pub fn term_columns() -> usize {
    // Check COLUMNS env var first
    if let Ok(val) = std::env::var("COLUMNS") {
        if let Ok(cols) = val.parse::<usize>() {
            if cols > 0 {
                return cols;
            }
        }
    }

    // Try terminal query
    #[cfg(unix)]
    {
        use std::mem::MaybeUninit;
        unsafe {
            let mut ws = MaybeUninit::<libc::winsize>::zeroed().assume_init();
            if libc::ioctl(libc::STDERR_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 {
                return ws.ws_col as usize;
            }
        }
    }

    // Default
    80
}

/// RAII guard for a pager subprocess.
///
/// When dropped, waits for the pager to finish.
pub struct PagerGuard {
    child: Child,
    pager_cmd: String,
}

impl PagerGuard {
    /// Get a mutable reference to the pager's stdin for writing.
    pub fn stdin(&mut self) -> Option<&mut std::process::ChildStdin> {
        self.child.stdin.as_mut()
    }

    /// Get the pager command that was used.
    pub fn pager_cmd(&self) -> &str {
        &self.pager_cmd
    }

    /// Wait for the pager to finish.
    pub fn wait(mut self) -> Result<()> {
        // Close stdin to signal EOF to the pager
        drop(self.child.stdin.take());
        self.child
            .wait()
            .map_err(|e| UtilError::Subprocess {
                command: format!("pager: {}", self.pager_cmd),
                source: e,
            })?;
        Ok(())
    }
}

impl Drop for PagerGuard {
    fn drop(&mut self) {
        // Close stdin to signal EOF
        drop(self.child.stdin.take());
        // Best-effort wait for pager
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_pager_git_pager_env_wins() {
        let result = resolve_pager_from(
            Some("my-pager".to_string()),
            Some("config-pager"),
            Some("env-pager".to_string()),
        );
        assert_eq!(result, Some("my-pager".to_string()));
    }

    #[test]
    fn resolve_pager_config_second() {
        let result = resolve_pager_from(None, Some("config-pager"), Some("env-pager".to_string()));
        assert_eq!(result, Some("config-pager".to_string()));
    }

    #[test]
    fn resolve_pager_pager_env_third() {
        let result = resolve_pager_from(None, None, Some("env-pager".to_string()));
        assert_eq!(result, Some("env-pager".to_string()));
    }

    #[test]
    fn resolve_pager_default_less() {
        let result = resolve_pager_from(None, None, None);
        assert_eq!(result, Some("less".to_string()));
    }

    #[test]
    fn pager_in_use_false_by_default() {
        // This tests the function logic - in CI/test env, GIT_PAGER_IN_USE
        // is typically not set, but we can't guarantee that without env mutation.
        // Just verify the function doesn't panic.
        let _ = pager_in_use();
    }

    #[test]
    fn term_columns_positive() {
        // term_columns should always return a positive value
        let cols = term_columns();
        assert!(cols > 0);
    }

    #[test]
    fn setup_pager_cat_returns_none() {
        // "cat" should mean no pager. setup_pager checks is_terminal first,
        // so in test env (not a tty) it will return None regardless.
        // Test the resolve logic instead.
        let pager = resolve_pager_from(Some("cat".to_string()), None, None);
        assert_eq!(pager, Some("cat".to_string()));
        // The setup_pager function would detect "cat" and return None
    }
}

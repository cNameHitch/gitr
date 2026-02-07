//! SSH transport implementation.
//!
//! Spawns an SSH process to connect to the remote repository's
//! git-upload-pack or git-receive-pack service.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

use crate::{GitUrl, Service, Transport, TransportError};

/// SSH transport using an external SSH process.
pub struct SshTransport {
    child: Child,
}

impl Transport for SshTransport {
    fn reader(&mut self) -> &mut dyn Read {
        self.child.stdout.as_mut().expect("stdout not captured")
    }

    fn writer(&mut self) -> &mut dyn Write {
        self.child.stdin.as_mut().expect("stdin not captured")
    }

    fn close(mut self: Box<Self>) -> Result<(), TransportError> {
        // Close stdin to signal EOF to the remote process
        drop(self.child.stdin.take());
        let status = self.child.wait()?;
        if !status.success() {
            // SSH exits non-zero for various reasons (e.g., server closed connection).
            // This is not always an error in the git sense (e.g., after a successful push,
            // the remote may close the connection and ssh returns 128).
            // We only report fatal errors here.
            let code = status.code().unwrap_or(-1);
            if code == 128 || code == 255 {
                return Err(TransportError::Ssh(format!(
                    "ssh process exited with code {}",
                    code
                )));
            }
        }
        Ok(())
    }
}

/// Resolve the SSH command to use.
///
/// Checks, in order:
/// 1. `GIT_SSH_COMMAND` environment variable
/// 2. `core.sshCommand` config (not checked here — caller should pass it)
/// 3. `GIT_SSH` environment variable
/// 4. Default: "ssh"
fn resolve_ssh_command(config_ssh_command: Option<&str>) -> String {
    if let Ok(cmd) = std::env::var("GIT_SSH_COMMAND") {
        return cmd;
    }
    if let Some(cmd) = config_ssh_command {
        return cmd.to_string();
    }
    if let Ok(cmd) = std::env::var("GIT_SSH") {
        return cmd;
    }
    "ssh".to_string()
}

/// Connect to a remote repository over SSH.
pub fn connect(url: &GitUrl, service: Service) -> Result<Box<dyn Transport>, TransportError> {
    connect_with_config(url, service, None)
}

/// Connect to a remote repository over SSH with optional ssh command from config.
pub fn connect_with_config(
    url: &GitUrl,
    service: Service,
    ssh_command: Option<&str>,
) -> Result<Box<dyn Transport>, TransportError> {
    let host = url
        .host
        .as_deref()
        .ok_or_else(|| TransportError::InvalidUrl("SSH URL requires a host".into()))?;

    let ssh_cmd = resolve_ssh_command(ssh_command);

    // Build the command: ssh [options] host git-upload-pack 'path'
    // If GIT_SSH_COMMAND is a complex command, we need to use shell
    let mut cmd = if ssh_cmd.contains(' ') {
        // Complex command — use shell
        let mut c = Command::new("sh");
        c.arg("-c");

        let mut shell_cmd = ssh_cmd.clone();

        if let Some(port) = url.port {
            shell_cmd.push_str(&format!(" -p {}", port));
        }

        if let Some(ref user) = url.user {
            shell_cmd.push_str(&format!(" {}@{}", user, host));
        } else {
            shell_cmd.push_str(&format!(" {}", host));
        }

        shell_cmd.push_str(&format!(" {} '{}'", service.as_str(), url.path));
        c.arg(shell_cmd);
        c
    } else {
        let mut c = Command::new(&ssh_cmd);

        if let Some(port) = url.port {
            c.arg("-p").arg(port.to_string());
        }

        // Request protocol v2 if supported
        c.arg("-o").arg("SendEnv=GIT_PROTOCOL");

        if let Some(ref user) = url.user {
            c.arg(format!("{}@{}", user, host));
        } else {
            c.arg(host);
        }

        c.arg(service.as_str());
        c.arg(&url.path);
        c
    };

    // Set GIT_PROTOCOL env for v2 negotiation
    cmd.env("GIT_PROTOCOL", "version=2");

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        TransportError::Ssh(format!("failed to spawn ssh: {}", e))
    })?;

    Ok(Box::new(SshTransport { child }))
}

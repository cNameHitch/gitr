//! Credential helper interface.
//!
//! Calls `git credential fill` to obtain authentication credentials
//! from the user's configured credential helpers.

use crate::TransportError;

/// Credential request parameters.
#[derive(Debug, Clone)]
pub struct CredentialRequest {
    pub protocol: String,
    pub host: String,
    pub path: Option<String>,
    pub username: Option<String>,
}

/// Credential response.
#[derive(Debug, Clone)]
pub struct CredentialResponse {
    pub username: String,
    pub password: String,
}

/// Get credentials using git's credential helper system.
pub fn get_credentials(request: &CredentialRequest) -> Result<CredentialResponse, TransportError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut input = format!(
        "protocol={}\nhost={}\n",
        request.protocol, request.host
    );
    if let Some(ref path) = request.path {
        input.push_str(&format!("path={}\n", path));
    }
    if let Some(ref username) = request.username {
        input.push_str(&format!("username={}\n", username));
    }
    input.push('\n');

    let mut child = Command::new("git")
        .args(["credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| TransportError::ConnectionFailed(format!("git credential fill failed: {}", e)))?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(input.as_bytes())?;
    }
    drop(child.stdin.take());

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(TransportError::AuthenticationFailed);
    }

    let response_str = String::from_utf8_lossy(&output.stdout);
    let mut username = None;
    let mut password = None;

    for line in response_str.lines() {
        if let Some(val) = line.strip_prefix("username=") {
            username = Some(val.to_string());
        } else if let Some(val) = line.strip_prefix("password=") {
            password = Some(val.to_string());
        }
    }

    match (username, password) {
        (Some(u), Some(p)) => Ok(CredentialResponse {
            username: u,
            password: p,
        }),
        _ => Err(TransportError::AuthenticationFailed),
    }
}

/// Approve credentials (tell the helper the credentials worked).
pub fn approve_credentials(request: &CredentialRequest, response: &CredentialResponse) -> Result<(), TransportError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let input = format!(
        "protocol={}\nhost={}\nusername={}\npassword={}\n\n",
        request.protocol, request.host, response.username, response.password
    );

    let mut child = Command::new("git")
        .args(["credential", "approve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| TransportError::AuthenticationFailed)?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(input.as_bytes())?;
    }
    drop(child.stdin.take());
    let _ = child.wait();
    Ok(())
}

/// Reject credentials (tell the helper the credentials failed).
pub fn reject_credentials(request: &CredentialRequest, response: &CredentialResponse) -> Result<(), TransportError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let input = format!(
        "protocol={}\nhost={}\nusername={}\npassword={}\n\n",
        request.protocol, request.host, response.username, response.password
    );

    let mut child = Command::new("git")
        .args(["credential", "reject"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| TransportError::AuthenticationFailed)?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(input.as_bytes())?;
    }
    drop(child.stdin.take());
    let _ = child.wait();
    Ok(())
}

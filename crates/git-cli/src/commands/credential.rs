use std::io::{self, BufRead, Write};

use anyhow::{bail, Result};
use clap::Args;

use crate::Cli;

#[derive(Args)]
pub struct CredentialArgs {
    /// Operation: fill, approve, or reject
    operation: String,
}

pub fn run(args: &CredentialArgs, cli: &Cli) -> Result<i32> {
    match args.operation.as_str() {
        "fill" => credential_fill(cli),
        "approve" => credential_approve(cli),
        "reject" => credential_reject(cli),
        other => {
            bail!("unknown credential operation: {}", other);
        }
    }
}

/// Parse credential attributes from stdin.
fn parse_credential_input() -> Result<CredentialRequest> {
    let stdin = io::stdin();
    let mut cred = CredentialRequest::default();

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }

        if let Some((key, value)) = line.split_once('=') {
            match key {
                "protocol" => cred.protocol = Some(value.to_string()),
                "host" => cred.host = Some(value.to_string()),
                "path" => cred.path = Some(value.to_string()),
                "username" => cred.username = Some(value.to_string()),
                "password" => cred.password = Some(value.to_string()),
                "password_expiry_utc" => cred.password_expiry_utc = Some(value.to_string()),
                "url" => {
                    // Parse URL into components
                    if let Some((proto, rest)) = value.split_once("://") {
                        cred.protocol = Some(proto.to_string());
                        if let Some((host, path)) = rest.split_once('/') {
                            cred.host = Some(host.to_string());
                            cred.path = Some(path.to_string());
                        } else {
                            cred.host = Some(rest.to_string());
                        }
                    }
                }
                _ => {
                    // Ignore unknown keys
                }
            }
        }
    }

    Ok(cred)
}

/// Write credential attributes to stdout.
fn write_credential_output(cred: &CredentialRequest) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if let Some(ref protocol) = cred.protocol {
        writeln!(out, "protocol={}", protocol)?;
    }
    if let Some(ref host) = cred.host {
        writeln!(out, "host={}", host)?;
    }
    if let Some(ref path) = cred.path {
        writeln!(out, "path={}", path)?;
    }
    if let Some(ref username) = cred.username {
        writeln!(out, "username={}", username)?;
    }
    if let Some(ref password) = cred.password {
        writeln!(out, "password={}", password)?;
    }
    if let Some(ref expiry) = cred.password_expiry_utc {
        writeln!(out, "password_expiry_utc={}", expiry)?;
    }
    writeln!(out)?; // Empty line to terminate

    Ok(())
}

fn credential_fill(cli: &Cli) -> Result<i32> {
    let mut cred = parse_credential_input()?;

    // Try to get helpers from config
    let helpers = get_credential_helpers(cli);

    for helper in &helpers {
        if let Ok(result) = run_credential_helper(helper, "get", &cred) {
            if result.username.is_some() && result.password.is_some() {
                cred.username = result.username.or(cred.username);
                cred.password = result.password.or(cred.password);
                cred.password_expiry_utc = result.password_expiry_utc.or(cred.password_expiry_utc);
                break;
            }
        }
    }

    write_credential_output(&cred)?;

    Ok(0)
}

fn credential_approve(cli: &Cli) -> Result<i32> {
    let cred = parse_credential_input()?;
    let helpers = get_credential_helpers(cli);

    for helper in &helpers {
        let _ = run_credential_helper(helper, "store", &cred);
    }

    Ok(0)
}

fn credential_reject(cli: &Cli) -> Result<i32> {
    let cred = parse_credential_input()?;
    let helpers = get_credential_helpers(cli);

    for helper in &helpers {
        let _ = run_credential_helper(helper, "erase", &cred);
    }

    Ok(0)
}

/// Get configured credential helpers.
fn get_credential_helpers(cli: &Cli) -> Vec<String> {
    let mut helpers = Vec::new();

    // Try to read from config
    if let Ok(repo) = super::open_repo(cli) {
        if let Ok(Some(helper)) = repo.config().get_string("credential.helper") {
            helpers.push(helper);
        }
    }

    helpers
}

/// Run a credential helper subprocess.
fn run_credential_helper(
    helper: &str,
    action: &str,
    cred: &CredentialRequest,
) -> Result<CredentialRequest> {
    // Build the command name
    let cmd = if helper.starts_with('/') || helper.starts_with('!') {
        helper.trim_start_matches('!').to_string()
    } else {
        format!("git-credential-{}", helper)
    };

    // Build stdin input
    let mut input = String::new();
    if let Some(ref protocol) = cred.protocol {
        input.push_str(&format!("protocol={}\n", protocol));
    }
    if let Some(ref host) = cred.host {
        input.push_str(&format!("host={}\n", host));
    }
    if let Some(ref path) = cred.path {
        input.push_str(&format!("path={}\n", path));
    }
    if let Some(ref username) = cred.username {
        input.push_str(&format!("username={}\n", username));
    }
    if let Some(ref password) = cred.password {
        input.push_str(&format!("password={}\n", password));
    }
    input.push('\n');

    let output = std::process::Command::new(&cmd)
        .arg(action)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(input.as_bytes())?;
            }
            child.wait_with_output()
        })?;

    // Parse output
    let mut result = CredentialRequest::default();
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    for line in stdout_str.lines() {
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "username" => result.username = Some(value.to_string()),
                "password" => result.password = Some(value.to_string()),
                "password_expiry_utc" => result.password_expiry_utc = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Ok(result)
}

#[derive(Default)]
struct CredentialRequest {
    protocol: Option<String>,
    host: Option<String>,
    path: Option<String>,
    username: Option<String>,
    password: Option<String>,
    password_expiry_utc: Option<String>,
}

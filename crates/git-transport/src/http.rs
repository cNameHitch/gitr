//! HTTP/HTTPS smart transport implementation.
//!
//! Implements the git smart HTTP protocol for fetch and push.
//! Each request/response cycle is a separate HTTP POST.

use std::io::{Cursor, Read, Write};

use crate::{GitUrl, Service, Transport, TransportError};

/// HTTP transport state.
pub struct HttpTransport {
    /// Base URL for the repository.
    base_url: String,
    /// The service we're talking to.
    service: Service,
    /// Buffer for data to be sent in the next request.
    write_buf: Vec<u8>,
    /// Response data from the last request.
    read_buf: Cursor<Vec<u8>>,
    /// Whether the initial info/refs request has been made.
    initial_request_done: bool,
}

impl HttpTransport {
    /// Perform the initial GET to /info/refs?service=... and return the response.
    fn do_initial_request(&mut self) -> Result<(), TransportError> {
        if self.initial_request_done {
            return Ok(());
        }

        let url = format!(
            "{}/info/refs?service={}",
            self.base_url,
            self.service.as_str()
        );

        // Use a simple blocking HTTP client via std
        let response = http_get(&url)?;
        self.read_buf = Cursor::new(response);
        self.initial_request_done = true;
        Ok(())
    }

    /// Perform a POST to the service endpoint with the write buffer contents.
    #[allow(dead_code)]
    fn do_post(&mut self) -> Result<(), TransportError> {
        let url = format!("{}/{}", self.base_url, self.service.as_str());
        let content_type = format!(
            "application/x-{}-request",
            self.service.as_str()
        );
        let accept = format!(
            "application/x-{}-result",
            self.service.as_str()
        );

        let body = std::mem::take(&mut self.write_buf);
        let response = http_post(&url, &content_type, &accept, &body)?;
        self.read_buf = Cursor::new(response);
        Ok(())
    }
}

impl Transport for HttpTransport {
    fn reader(&mut self) -> &mut dyn Read {
        // Ensure initial request is done
        if !self.initial_request_done {
            if let Err(e) = self.do_initial_request() {
                // Store error as empty read
                eprintln!("HTTP initial request error: {}", e);
            }
        }
        &mut self.read_buf
    }

    fn writer(&mut self) -> &mut dyn Write {
        &mut self.write_buf
    }

    fn close(self: Box<Self>) -> Result<(), TransportError> {
        Ok(())
    }

    fn is_stateless(&self) -> bool {
        true
    }
}

/// Connect to a remote repository over HTTP/HTTPS.
pub fn connect(url: &GitUrl, service: Service) -> Result<Box<dyn Transport>, TransportError> {
    let base_url = format!(
        "{}://{}{}{}",
        url.scheme,
        url.host.as_deref().unwrap_or(""),
        url.port
            .map(|p| format!(":{}", p))
            .unwrap_or_default(),
        url.path
    );

    let mut transport = HttpTransport {
        base_url,
        service,
        write_buf: Vec::new(),
        read_buf: Cursor::new(Vec::new()),
        initial_request_done: false,
    };

    // Perform the initial info/refs discovery
    transport.do_initial_request()?;

    Ok(Box::new(transport))
}

/// Simple blocking HTTP GET using std::process::Command (curl).
fn http_get(url: &str) -> Result<Vec<u8>, TransportError> {
    let output = std::process::Command::new("curl")
        .args(["-sfL", "--include", url])
        .output()
        .map_err(|e| TransportError::ConnectionFailed(format!("curl not found: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TransportError::Http {
            status: 0,
            message: format!("HTTP GET failed: {}", stderr),
        });
    }

    // Parse response to extract body (skip headers)
    let response = output.stdout;
    extract_http_body(&response)
}

/// Simple blocking HTTP POST using std::process::Command (curl).
#[allow(dead_code)]
fn http_post(
    url: &str,
    content_type: &str,
    accept: &str,
    body: &[u8],
) -> Result<Vec<u8>, TransportError> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let mut child = Command::new("curl")
        .args([
            "-sf",
            "--include",
            "-X", "POST",
            "-H", &format!("Content-Type: {}", content_type),
            "-H", &format!("Accept: {}", accept),
            "--data-binary", "@-",
            url,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| TransportError::ConnectionFailed(format!("curl not found: {}", e)))?;

    // Write body to stdin
    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(body)?;
    }
    drop(child.stdin.take());

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TransportError::Http {
            status: 0,
            message: format!("HTTP POST failed: {}", stderr),
        });
    }

    extract_http_body(&output.stdout)
}

/// Extract HTTP response body by skipping headers.
fn extract_http_body(response: &[u8]) -> Result<Vec<u8>, TransportError> {
    // Find \r\n\r\n boundary between headers and body
    for i in 0..response.len().saturating_sub(3) {
        if &response[i..i + 4] == b"\r\n\r\n" {
            return Ok(response[i + 4..].to_vec());
        }
    }
    // No headers found — return entire response (might be body-only)
    Ok(response.to_vec())
}

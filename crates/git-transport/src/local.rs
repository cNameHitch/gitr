//! Local transport implementation.
//!
//! Directly spawns the local git-upload-pack or git-receive-pack process
//! for file:// and local path URLs.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};

use crate::{GitUrl, Service, Transport, TransportError};

/// Local transport using a subprocess.
pub struct LocalTransport {
    child: Child,
}

impl Transport for LocalTransport {
    fn reader(&mut self) -> &mut dyn Read {
        self.child.stdout.as_mut().expect("stdout not captured")
    }

    fn writer(&mut self) -> &mut dyn Write {
        self.child.stdin.as_mut().expect("stdin not captured")
    }

    fn close(mut self: Box<Self>) -> Result<(), TransportError> {
        drop(self.child.stdin.take());
        let status = self.child.wait()?;
        if !status.success() {
            let code = status.code().unwrap_or(-1);
            return Err(TransportError::ConnectionFailed(format!(
                "local transport process exited with code {}",
                code
            )));
        }
        Ok(())
    }
}

/// Connect to a local repository.
pub fn connect(url: &GitUrl, service: Service) -> Result<Box<dyn Transport>, TransportError> {
    let child = Command::new(service.as_str())
        .arg(&url.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            TransportError::ConnectionFailed(format!(
                "failed to spawn {}: {}",
                service.as_str(),
                e
            ))
        })?;

    Ok(Box::new(LocalTransport { child }))
}

//! Transport abstraction for git wire protocol communication.
//!
//! This crate provides the physical transport layer for git network operations.
//! It handles SSH (subprocess), HTTP/HTTPS (smart protocol), and local (direct
//! file access) transports. Higher-level protocol logic lives in `git-protocol`.

pub mod credential;
pub mod http;
pub mod local;
pub mod ssh;
pub mod url;

use std::io::{Read, Write};

use bstr::BString;
use git_hash::ObjectId;

/// Errors that can occur during transport operations.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("unsupported URL scheme: {0}")]
    UnsupportedScheme(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("authentication failed")]
    AuthenticationFailed,

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("HTTP error: {status}: {message}")]
    Http { status: u16, message: String },

    #[error("server error: {0}")]
    ServerError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Git URL scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Ssh,
    Git,
    Http,
    Https,
    File,
    /// Local path without scheme prefix.
    Local,
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scheme::Ssh => write!(f, "ssh"),
            Scheme::Git => write!(f, "git"),
            Scheme::Http => write!(f, "http"),
            Scheme::Https => write!(f, "https"),
            Scheme::File => write!(f, "file"),
            Scheme::Local => write!(f, "local"),
        }
    }
}

/// Parsed git URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUrl {
    pub scheme: Scheme,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub path: String,
}

impl std::fmt::Display for GitUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.scheme {
            Scheme::Ssh if self.user.is_some() && self.port.is_none() => {
                // SCP-like syntax: user@host:path
                write!(
                    f,
                    "{}@{}:{}",
                    self.user.as_deref().unwrap_or("git"),
                    self.host.as_deref().unwrap_or(""),
                    self.path
                )
            }
            Scheme::Local => write!(f, "{}", self.path),
            Scheme::File => write!(f, "file://{}", self.path),
            _ => {
                write!(f, "{}://", self.scheme)?;
                if let Some(ref user) = self.user {
                    write!(f, "{}@", user)?;
                }
                if let Some(ref host) = self.host {
                    write!(f, "{}", host)?;
                }
                if let Some(port) = self.port {
                    write!(f, ":{}", port)?;
                }
                write!(f, "{}", self.path)
            }
        }
    }
}

/// Service type for git transport connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// git-upload-pack (for fetch/clone).
    UploadPack,
    /// git-receive-pack (for push).
    ReceivePack,
}

impl Service {
    /// Service name as used in the protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            Service::UploadPack => "git-upload-pack",
            Service::ReceivePack => "git-receive-pack",
        }
    }
}

/// Protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    V0,
    V1,
    V2,
}

/// Result of the initial transport handshake.
#[derive(Debug)]
pub struct HandshakeResult {
    pub protocol_version: ProtocolVersion,
    pub capabilities: Vec<String>,
    /// Advertised refs: (OID, refname). Empty for v2 (refs come via ls-refs).
    pub refs: Vec<(ObjectId, BString)>,
    /// Raw initial response lines for protocol parsing.
    pub extra_lines: Vec<Vec<u8>>,
}

/// Trait for transport connections.
///
/// A transport provides bidirectional I/O with a remote git process.
/// The connection lifecycle is:
/// 1. Connect via `connect()` function
/// 2. Read/write using the reader/writer
/// 3. Close when done
pub trait Transport: Send {
    /// Get a reader for the server's response.
    fn reader(&mut self) -> &mut dyn Read;

    /// Get a writer for sending data to the server.
    fn writer(&mut self) -> &mut dyn Write;

    /// Close the transport connection.
    fn close(self: Box<Self>) -> Result<(), TransportError>;

    /// Whether this transport supports stateless operation (HTTP).
    fn is_stateless(&self) -> bool {
        false
    }
}

/// Open a transport connection for the given URL and service.
pub fn connect(
    url: &GitUrl,
    service: Service,
) -> Result<Box<dyn Transport>, TransportError> {
    match url.scheme {
        Scheme::Ssh => ssh::connect(url, service),
        Scheme::Git => {
            // Git protocol uses a similar subprocess mechanism
            Err(TransportError::UnsupportedScheme(
                "git:// protocol not yet implemented".into(),
            ))
        }
        Scheme::Http | Scheme::Https => http::connect(url, service),
        Scheme::File | Scheme::Local => local::connect(url, service),
    }
}

//! Git wire protocol implementation.
//!
//! This crate implements the git wire protocol for fetch and push operations.
//! It handles pkt-line framing, capability negotiation, v1/v2 protocol
//! exchanges, and remote configuration.

pub mod bundle;
pub mod capability;
pub mod fetch;
pub mod pktline;
pub mod push;
pub mod remote;
pub mod sideband;
pub mod v1;
pub mod v2;

use git_transport::TransportError;

/// Errors that can occur during protocol operations.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("invalid pkt-line: {0}")]
    InvalidPktLine(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("server error: {0}")]
    ServerError(String),

    #[error("push rejected: {0}")]
    PushRejected(String),

    #[error("unsupported capability: {0}")]
    UnsupportedCapability(String),

    #[error("invalid refspec: {0}")]
    InvalidRefSpec(String),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Pack(#[from] git_pack::PackError),

    #[error(transparent)]
    Ref(#[from] git_ref::RefError),

    #[error(transparent)]
    Config(#[from] git_config::ConfigError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

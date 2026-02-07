# Data Model: Transport Protocol

## Core Types

```rust
// --- git-transport crate ---

/// Parsed git URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUrl {
    pub scheme: Scheme,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Ssh,
    Git,
    Http,
    Https,
    File,
    Local,
}

impl GitUrl {
    pub fn parse(url: &str) -> Result<Self, TransportError>;
}

/// Trait for transport connections.
pub trait Transport: Send {
    /// Perform a handshake and return the server's ref list and capabilities
    fn handshake(&mut self, service: Service) -> Result<HandshakeResult, TransportError>;
    /// Get a reader for the server's response
    fn reader(&mut self) -> &mut dyn std::io::Read;
    /// Get a writer for sending data to the server
    fn writer(&mut self) -> &mut dyn std::io::Write;
    /// Close the connection
    fn close(self: Box<Self>) -> Result<(), TransportError>;
}

pub enum Service {
    UploadPack,   // For fetch
    ReceivePack,  // For push
}

pub struct HandshakeResult {
    pub protocol_version: ProtocolVersion,
    pub capabilities: Vec<String>,
    pub refs: Vec<(ObjectId, BString)>,
}

pub enum ProtocolVersion {
    V0,
    V1,
    V2,
}

/// Open a transport connection for the given URL.
pub fn connect(url: &GitUrl, service: Service) -> Result<Box<dyn Transport>, TransportError>;

/// Credential request.
pub struct CredentialRequest {
    pub protocol: String,
    pub host: String,
    pub path: Option<String>,
    pub username: Option<String>,
}

pub struct CredentialResponse {
    pub username: String,
    pub password: String,
}

/// Call credential helpers to get authentication credentials.
pub fn get_credentials(request: &CredentialRequest) -> Result<CredentialResponse, TransportError>;

// --- git-protocol crate ---

/// Pkt-line reader.
pub struct PktLineReader<R> {
    reader: R,
}

impl<R: std::io::Read> PktLineReader<R> {
    pub fn new(reader: R) -> Self;
    /// Read one pkt-line. Returns None for flush packet.
    pub fn read_line(&mut self) -> Result<Option<Vec<u8>>, ProtocolError>;
    /// Read lines until flush.
    pub fn read_until_flush(&mut self) -> Result<Vec<Vec<u8>>, ProtocolError>;
}

/// Pkt-line writer.
pub struct PktLineWriter<W> {
    writer: W,
}

impl<W: std::io::Write> PktLineWriter<W> {
    pub fn new(writer: W) -> Self;
    pub fn write_line(&mut self, data: &[u8]) -> Result<(), ProtocolError>;
    pub fn write_flush(&mut self) -> Result<(), ProtocolError>;
    pub fn write_delimiter(&mut self) -> Result<(), ProtocolError>;
}

/// Remote configuration from git config.
pub struct RemoteConfig {
    pub name: String,
    pub url: GitUrl,
    pub push_url: Option<GitUrl>,
    pub fetch_refspecs: Vec<RefSpec>,
    pub push_refspecs: Vec<RefSpec>,
}

/// A refspec for fetch or push.
#[derive(Debug, Clone)]
pub struct RefSpec {
    pub source: BString,
    pub destination: BString,
    pub force: bool,
}

impl RefSpec {
    pub fn parse(spec: &str) -> Result<Self, ProtocolError>;
}

/// Perform a fetch operation.
pub fn fetch(
    transport: &mut dyn Transport,
    repo: &Repository,
    wants: &[ObjectId],
    options: &FetchOptions,
) -> Result<FetchResult, ProtocolError>;

pub struct FetchOptions {
    pub depth: Option<u32>,
    pub filter: Option<String>,
    pub progress: bool,
}

pub struct FetchResult {
    pub pack_path: Option<PathBuf>,
    pub ref_updates: Vec<(RefName, ObjectId)>,
    pub new_commits: usize,
}

/// Perform a push operation.
pub fn push(
    transport: &mut dyn Transport,
    repo: &Repository,
    ref_updates: &[PushUpdate],
    options: &PushOptions,
) -> Result<PushResult, ProtocolError>;

pub struct PushUpdate {
    pub local_ref: Option<ObjectId>,  // None = delete
    pub remote_ref: RefName,
    pub force: bool,
    /// Expected old OID on remote for --force-with-lease (None = no check)
    pub expected_remote_oid: Option<ObjectId>,
}

pub struct PushOptions {
    pub progress: bool,
    pub atomic: bool,
    pub push_options: Vec<String>,  // --push-option values sent to server
    pub thin: bool,  // Generate thin packs (default true)
}

pub struct PushResult {
    pub ok: bool,
    pub ref_results: Vec<(RefName, PushRefResult)>,
    pub server_message: Option<String>,
}

pub enum PushRefResult {
    Ok,
    /// Non-fast-forward or force-with-lease mismatch
    Rejected(String),
    /// Server-side error
    Error(String),
}

/// Perform the send-pack push protocol exchange.
///
/// Protocol flow:
/// 1. Read server's ref advertisement from handshake result
/// 2. Determine ref update commands by comparing local vs remote refs
/// 3. Send ref update lines: `<old-oid> <new-oid> <refname>\n`
/// 4. Send flush packet
/// 5. Generate and stream thin pack (objects reachable from new OIDs
///    but not from remote's advertised OIDs)
/// 6. Read server's status report (`unpack ok/ng`, per-ref `ok/ng`)
/// 7. Return structured PushResult
pub fn push(
    transport: &mut dyn Transport,
    repo: &Repository,
    ref_updates: &[PushUpdate],
    options: &PushOptions,
) -> Result<PushResult, ProtocolError>;

/// Determine which objects need to be sent for a push.
///
/// Computes the set difference: objects reachable from `local_oids`
/// minus objects reachable from `remote_oids`. Used to feed the
/// thin pack generator.
pub fn compute_push_objects(
    repo: &Repository,
    local_oids: &[ObjectId],
    remote_oids: &[ObjectId],
) -> Result<Vec<ObjectId>, ProtocolError>;

/// Sideband demultiplexer.
pub struct SidebandReader<R> {
    reader: PktLineReader<R>,
}

impl<R: std::io::Read> SidebandReader<R> {
    /// Read the next data packet. Progress and errors are handled internally.
    pub fn read_data(&mut self) -> Result<Option<Vec<u8>>, ProtocolError>;
}

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
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

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
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Pack(#[from] git_pack::PackError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

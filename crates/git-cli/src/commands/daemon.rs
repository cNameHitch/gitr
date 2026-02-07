use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::Args;

use crate::Cli;

#[derive(Args)]
pub struct DaemonArgs {
    /// Be verbose about incoming connections
    #[arg(long)]
    verbose: bool,

    /// Log to syslog instead of stderr
    #[arg(long)]
    syslog: bool,

    /// Export all repositories (not just those with git-daemon-export-ok)
    #[arg(long = "export-all")]
    export_all: bool,

    /// Connection timeout in seconds
    #[arg(long, default_value = "0")]
    timeout: u32,

    /// Initial connection timeout
    #[arg(long = "init-timeout", default_value = "0")]
    init_timeout: u32,

    /// Maximum simultaneous connections
    #[arg(long = "max-connections", default_value = "32")]
    max_connections: u32,

    /// Only serve exactly the specified directories
    #[arg(long = "strict-paths")]
    strict_paths: bool,

    /// Remap all path requests to be relative to this path
    #[arg(long = "base-path")]
    base_path: Option<PathBuf>,

    /// Allow cloning from ~user directories
    #[arg(long = "user-path")]
    user_path: Option<Option<String>>,

    /// Allow or enable/disable services
    #[arg(long = "enable")]
    enable: Vec<String>,

    #[arg(long = "disable")]
    disable: Vec<String>,

    /// Run as an inetd service (read/write on stdin/stdout)
    #[arg(long)]
    inetd: bool,

    /// Listen on a specific address
    #[arg(long, default_value = "0.0.0.0")]
    listen: String,

    /// Listen on a specific port
    #[arg(long, default_value = "9418")]
    port: u16,

    /// Detach from terminal
    #[arg(long)]
    detach: bool,

    /// PID file path
    #[arg(long = "pid-file")]
    pid_file: Option<PathBuf>,

    /// Run as user
    #[arg(long)]
    user: Option<String>,

    /// Run as group
    #[arg(long)]
    group: Option<String>,

    /// Log destination
    #[arg(long = "log-destination")]
    log_destination: Option<String>,

    /// Directories to serve
    directories: Vec<PathBuf>,
}

pub fn run(args: &DaemonArgs, _cli: &Cli) -> Result<i32> {
    let stderr = io::stderr();
    let mut err = stderr.lock();

    if args.inetd {
        return run_inetd(args);
    }

    let addr = format!("{}:{}", args.listen, args.port);

    if args.verbose {
        writeln!(err, "Ready to rumble on {}", addr)?;
    }

    // Write PID file
    if let Some(ref pid_path) = args.pid_file {
        std::fs::write(pid_path, format!("{}", std::process::id()))?;
    }

    let listener = TcpListener::bind(&addr)?;

    if args.verbose {
        writeln!(err, "Listening on {}", addr)?;
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let verbose = args.verbose;
                let export_all = args.export_all;
                let base_path = args.base_path.clone();
                let directories = args.directories.clone();
                let strict_paths = args.strict_paths;

                std::thread::spawn(move || {
                    if let Err(e) = handle_client(
                        stream,
                        verbose,
                        export_all,
                        base_path.as_deref(),
                        &directories,
                        strict_paths,
                    ) {
                        eprintln!("client error: {}", e);
                    }
                });
            }
            Err(e) => {
                if args.verbose {
                    writeln!(err, "Accept error: {}", e)?;
                }
            }
        }
    }

    Ok(0)
}

fn run_inetd(args: &DaemonArgs) -> Result<i32> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // Read the request line (pkt-line format)
    let request = read_pktline(&mut stdin)?;
    let request_str = String::from_utf8_lossy(&request);

    if args.verbose {
        eprintln!("inetd request: {}", request_str.trim());
    }

    // Parse: "git-upload-pack /path\0host=hostname\0"
    let parts: Vec<&str> = request_str.split('\0').collect();
    let cmd_and_path = parts.first().unwrap_or(&"");
    let (service, path) = if let Some(rest) = cmd_and_path.strip_prefix("git-upload-pack ") {
        ("git-upload-pack", rest.trim())
    } else if let Some(rest) = cmd_and_path.strip_prefix("git-receive-pack ") {
        ("git-receive-pack", rest.trim())
    } else {
        bail!("unknown service request: {}", cmd_and_path);
    };

    // Resolve path
    let repo_path = if let Some(ref base) = args.base_path {
        base.join(path.trim_start_matches('/'))
    } else {
        PathBuf::from(path)
    };

    // Check export-ok
    if !args.export_all {
        let export_ok = repo_path.join("git-daemon-export-ok");
        if !export_ok.exists() {
            write_pktline(&mut stdout, b"ERR Repository not exported\n")?;
            return Ok(1);
        }
    }

    // Execute the service
    let output = std::process::Command::new("git")
        .arg(service)
        .arg(&repo_path)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?;

    if output.success() {
        Ok(0)
    } else {
        Ok(1)
    }
}

fn handle_client(
    mut stream: TcpStream,
    verbose: bool,
    export_all: bool,
    base_path: Option<&Path>,
    directories: &[PathBuf],
    strict_paths: bool,
) -> Result<()> {
    if verbose {
        if let Ok(addr) = stream.peer_addr() {
            eprintln!("Connection from {}", addr);
        }
    }

    // Read the request
    let request = read_pktline_stream(&mut stream)?;
    let request_str = String::from_utf8_lossy(&request);

    // Parse service and path
    let parts: Vec<&str> = request_str.split('\0').collect();
    let cmd_and_path = parts.first().unwrap_or(&"");
    let (service, path) = if let Some(rest) = cmd_and_path.strip_prefix("git-upload-pack ") {
        ("git-upload-pack", rest.trim())
    } else if let Some(rest) = cmd_and_path.strip_prefix("git-receive-pack ") {
        ("git-receive-pack", rest.trim())
    } else {
        write_pktline_stream(&mut stream, b"ERR Invalid request\n")?;
        return Ok(());
    };

    // Resolve path
    let repo_path = if let Some(base) = base_path {
        base.join(path.trim_start_matches('/'))
    } else {
        PathBuf::from(path)
    };

    // Validate path
    if strict_paths && !directories.is_empty() {
        let canonical = std::fs::canonicalize(&repo_path).unwrap_or_default();
        let allowed = directories
            .iter()
            .any(|d| canonical.starts_with(d));
        if !allowed {
            write_pktline_stream(&mut stream, b"ERR Access denied\n")?;
            return Ok(());
        }
    }

    // Check export-ok
    if !export_all {
        let export_ok = repo_path.join("git-daemon-export-ok");
        if !export_ok.exists() {
            write_pktline_stream(&mut stream, b"ERR Repository not exported\n")?;
            return Ok(());
        }
    }

    // Spawn the service process
    let child = std::process::Command::new("git")
        .arg(service)
        .arg(&repo_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    // Proxy I/O between stream and process (simplified)
    let mut child_stdin = child.stdin.unwrap();
    let mut child_stdout = child.stdout.unwrap();

    // Read from stream, write to process
    let mut stream_clone = stream.try_clone()?;
    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match stream_clone.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if child_stdin.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Read from process, write to stream
    let mut buf = [0u8; 8192];
    loop {
        match child_stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if stream.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let _ = reader.join();

    Ok(())
}

fn read_pktline(input: &mut impl Read) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    input.read_exact(&mut len_buf)?;
    let len_str = std::str::from_utf8(&len_buf)?;
    let len = u16::from_str_radix(len_str, 16)?;
    if len <= 4 {
        return Ok(Vec::new());
    }
    let mut data = vec![0u8; (len - 4) as usize];
    input.read_exact(&mut data)?;
    Ok(data)
}

fn write_pktline(output: &mut impl Write, data: &[u8]) -> Result<()> {
    let len = data.len() + 4;
    write!(output, "{:04x}", len)?;
    output.write_all(data)?;
    Ok(())
}

fn read_pktline_stream(stream: &mut TcpStream) -> Result<Vec<u8>> {
    read_pktline(stream)
}

fn write_pktline_stream(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    write_pktline(stream, data)
}

//! Unix socket server that accepts Varlink requests from subportal clients.
//!
//! The [`Server`] binds to a Unix domain socket and accepts one connection at a
//! time via [`Server::accept`]. Each accepted connection yields a parsed
//! [`Request`] and a [`Responder`] that the caller uses to send back either
//! a success [`Response`] or a [`SubportalError`].

use std::path::{Path, PathBuf};

use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};
use tracing::info;

use crate::consts::default_socket_path;
use crate::protocol::{
    read_message, write_message, Request, Response, SubportalError, VarlinkRequest, VarlinkResponse,
};

/// Information about the peer process that connected.
#[derive(Debug, Clone, Default)]
pub struct PeerInfo {
    /// PID of the connecting process (from `SO_PEERCRED`).
    pub pid: Option<u32>,
    /// SSH remote host, resolved from `/proc/<pid>/cmdline` if the peer is an
    /// `sshd` or `ssh` process.
    pub ssh_host: Option<String>,
}

/// A Unix socket server that accepts Varlink requests.
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
}

/// Holds a parsed request and the stream, allowing the handler to send a response.
pub struct Responder {
    stream: UnixStream,
    /// Information about the connecting peer.
    pub peer: PeerInfo,
}

impl Server {
    /// Bind to the given Unix socket path.
    ///
    /// Creates the parent directory if needed and removes any stale socket file.
    pub async fn bind(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist.
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Remove stale socket file if present.
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }

        let listener = UnixListener::bind(&path)?;
        info!("listening on {}", path.display());
        Ok(Self { listener, path })
    }

    /// Return the socket path the server is bound to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bind using the default socket path.
    pub async fn bind_default() -> anyhow::Result<Self> {
        Self::bind(default_socket_path()).await
    }

    /// Accept the next connection, read the request, and return it with a `Responder`.
    pub async fn accept(&self) -> anyhow::Result<(Request, Responder)> {
        let (mut stream, _addr) = self.listener.accept().await?;

        let peer = resolve_peer(&stream);
        if let Some(ref host) = peer.ssh_host {
            tracing::debug!("connection from SSH host {host} (pid {:?})", peer.pid);
        } else {
            tracing::debug!("connection from pid {:?}", peer.pid);
        }

        let varlink_req: VarlinkRequest = read_message(&mut BufReader::new(&mut stream)).await?;
        let request = Request::from_varlink(&varlink_req)?;

        Ok((request, Responder { stream, peer }))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Best-effort removal of the socket file.
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Responder {
    /// Send a successful response.
    pub async fn send_ok(mut self, response: Response) -> anyhow::Result<()> {
        let varlink_resp = response.to_varlink();
        write_message(&mut self.stream, &varlink_resp).await
    }

    /// Send an error response.
    pub async fn send_error(mut self, error: SubportalError) -> anyhow::Result<()> {
        let varlink_resp: VarlinkResponse = error.to_varlink();
        write_message(&mut self.stream, &varlink_resp).await
    }
}

/// Resolve peer information from a Unix stream using `SO_PEERCRED`.
fn resolve_peer(stream: &UnixStream) -> PeerInfo {
    let pid = stream
        .peer_cred()
        .ok()
        .and_then(|cred| cred.pid())
        .map(|p| p as u32);

    let ssh_host = pid.and_then(resolve_ssh_host);

    PeerInfo { pid, ssh_host }
}

/// Try to resolve the SSH remote host from `/proc/<pid>/cmdline`.
///
/// Walks the process tree upward looking for an `sshd` process whose cmdline
/// contains `sshd: <user>@<ip>` or similar patterns.
fn resolve_ssh_host(pid: u32) -> Option<String> {
    // First try the process itself, then walk up the parent chain.
    let mut current_pid = pid;
    for _ in 0..10 {
        if let Some(host) = parse_ssh_host_from_cmdline(current_pid) {
            return Some(host);
        }
        // Try the parent process.
        match get_parent_pid(current_pid) {
            Some(ppid) if ppid > 1 && ppid != current_pid => current_pid = ppid,
            _ => break,
        }
    }
    None
}

/// Parse `/proc/<pid>/cmdline` looking for an SSH host pattern.
fn parse_ssh_host_from_cmdline(pid: u32) -> Option<String> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let cmdline_str = String::from_utf8_lossy(&cmdline);

    // sshd child processes have cmdline like "sshd: user@1.2.3.4" or
    // "sshd: user [priv]"
    if cmdline_str.starts_with("sshd: ") {
        // Extract the part after "sshd: "
        let rest = cmdline_str.strip_prefix("sshd: ")?;
        // Look for user@host pattern
        if let Some(at_pos) = rest.find('@') {
            let after_at = &rest[at_pos + 1..];
            // The host ends at whitespace or NUL
            let host = after_at
                .split(|c: char| c.is_whitespace() || c == '\0')
                .next()?;
            if !host.is_empty() {
                return Some(host.to_string());
            }
        }
    }

    None
}

/// Get the parent PID of a process from `/proc/<pid>/stat`.
fn get_parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Format: "pid (comm) state ppid ..."
    // Find the closing paren to skip the comm field (which can contain spaces).
    let after_comm = stat.rfind(')')? + 2;
    let fields: Vec<&str> = stat[after_comm..].split_whitespace().collect();
    // fields[0] = state, fields[1] = ppid
    fields.get(1)?.parse().ok()
}

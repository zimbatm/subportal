//! Unix socket server that accepts Varlink requests from subportal clients.
//!
//! The [`Server`] binds to a Unix domain socket and accepts one connection at a
//! time via [`Server::accept`]. Each accepted connection yields a parsed
//! [`Request`] and a [`Responder`] that the caller uses to send back either
//! a success [`Response`] or a [`SubportalError`].

use std::path::{Path, PathBuf};

use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

use crate::consts::default_socket_path;
use crate::protocol::{read_message, write_message, Request, Response, SubportalError};

/// Information about the peer process that connected.
#[derive(Debug, Clone, Default)]
pub struct PeerInfo {
    /// PID of the connecting process (from `SO_PEERCRED`).
    pub pid: Option<u32>,
    /// UID of the connecting process (from `SO_PEERCRED`).
    pub uid: Option<u32>,
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
    ///
    /// Returns the parsed request, the optional `host` identifier sent by the
    /// client, and the responder for sending the reply.
    pub async fn accept(&self) -> anyhow::Result<(Request, Option<String>, Responder)> {
        let (mut stream, _addr) = self.listener.accept().await?;

        let peer = resolve_peer(&stream);

        // Reject connections from different, non-root UIDs.
        if let Some(uid) = peer.uid {
            let my_uid = unsafe { libc::getuid() };
            if uid != 0 && uid != my_uid {
                warn!(
                    peer_uid = uid,
                    server_uid = my_uid,
                    "rejecting connection: uid mismatch"
                );
                anyhow::bail!("uid mismatch: peer uid {} != server uid {}", uid, my_uid);
            }
        }

        let value: serde_json::Value = read_message(&mut BufReader::new(&mut stream)).await?;

        // Extract the optional host identifier before dispatching the request.
        let host = value
            .get("parameters")
            .and_then(|p| p.get("host"))
            .and_then(|v| v.as_str())
            .map(String::from);

        if let Some(ref h) = host {
            info!("connection from host {h} (pid {:?})", peer.pid);
        } else {
            info!("connection from pid {:?}", peer.pid);
        }

        let request: Request = serde_json::from_value(value)?;

        Ok((request, host, Responder { stream, peer }))
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
        let wire_resp = response.to_wire();
        write_message(&mut self.stream, &wire_resp).await
    }

    /// Send an error response.
    pub async fn send_error(mut self, error: SubportalError) -> anyhow::Result<()> {
        let wire_resp = error.to_wire();
        write_message(&mut self.stream, &wire_resp).await
    }
}

/// Resolve peer information from a Unix stream using `SO_PEERCRED`.
fn resolve_peer(stream: &UnixStream) -> PeerInfo {
    let cred = stream.peer_cred().ok();

    let pid = cred.as_ref().and_then(|c| c.pid()).map(|p| p as u32);
    let uid = cred.as_ref().map(|c| c.uid());

    PeerInfo { pid, uid }
}

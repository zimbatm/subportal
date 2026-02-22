//! Unix socket client for sending requests to the subportal daemon.
//!
//! The [`Client`] opens a new Unix connection for each request (one-shot
//! pattern), sends a Varlink JSON message, and reads the response.
//!
//! Errors are returned as [`ClientError`], which distinguishes transport
//! failures ([`ClientError::DaemonUnreachable`]) from protocol-level errors
//! returned by the daemon ([`ClientError::Protocol`]).
//!
//! The target socket path is read from the `SUBPORTAL_SOCKET` environment
//! variable, falling back to [`crate::consts::default_socket_path`].

use std::path::{Path, PathBuf};

use tokio::io::BufReader;
use tokio::net::UnixStream;

use crate::consts::{default_socket_path, SOCKET_PATH_ENV};
use crate::protocol::{read_message, write_message, Request, Response, SubportalError, WireResponse};

/// Errors returned by [`Client::call`].
#[derive(Debug)]
pub enum ClientError {
    /// The daemon socket could not be reached (connection refused, not found, etc.).
    DaemonUnreachable,
    /// The daemon responded with a protocol-level error.
    Protocol(SubportalError),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::DaemonUnreachable => write!(f, "daemon is not reachable"),
            ClientError::Protocol(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<SubportalError> for ClientError {
    fn from(e: SubportalError) -> Self {
        ClientError::Protocol(e)
    }
}

/// A client that connects to the subportal daemon over a Unix socket.
pub struct Client {
    path: PathBuf,
    /// Hostname of this machine, included in every request so the daemon can
    /// identify which server the request originates from.
    host: Option<String>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Create a new client, reading the socket path from `SUBPORTAL_SOCKET` env
    /// or using the default.
    pub fn new() -> Self {
        let path = std::env::var(SOCKET_PATH_ENV)
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(default_socket_path);
        Self {
            path,
            host: get_hostname(),
        }
    }

    /// Return the socket path this client connects to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Create a new client that connects to the given socket path.
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            host: get_hostname(),
        }
    }

    /// Send a request and receive the response. Opens a new connection each time.
    ///
    /// Returns [`ClientError::DaemonUnreachable`] if the socket cannot be
    /// connected to, and [`ClientError::Protocol`] for errors returned by the
    /// daemon itself (e.g. `NoClient` when no desktop client is connected).
    pub async fn call(&self, request: &Request) -> Result<Response, ClientError> {
        let mut stream = UnixStream::connect(&self.path)
            .await
            .map_err(|_| ClientError::DaemonUnreachable)?;

        let mut value =
            serde_json::to_value(request).map_err(|_| ClientError::DaemonUnreachable)?;

        // Inject the host identifier into the request parameters.
        if let Some(ref host) = self.host {
            if let Some(params) = value.get_mut("parameters").and_then(|v| v.as_object_mut()) {
                params.insert("host".into(), serde_json::json!(host));
            }
        }

        write_message(&mut stream, &value)
            .await
            .map_err(|_| ClientError::DaemonUnreachable)?;

        let wire_resp: WireResponse = read_message(&mut BufReader::new(&mut stream))
            .await
            .map_err(|_| ClientError::DaemonUnreachable)?;

        // Check for error response
        if let Some(ref err_id) = wire_resp.error {
            let params = wire_resp
                .parameters
                .as_ref()
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            if let Some(e) = SubportalError::from_wire(err_id, &params) {
                return Err(ClientError::Protocol(e));
            }
            return Err(ClientError::Protocol(SubportalError::NotSupported {
                capability: "unknown".into(),
            }));
        }

        // Parse success response based on what we sent
        let params = wire_resp.parameters.unwrap_or_default();
        match request {
            Request::RevokeClient { .. } => Ok(Response::Ok),
            Request::Ping {} => {
                let capabilities = params
                    .get("capabilities")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let version = params
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Ok(Response::Ping {
                    capabilities,
                    version,
                })
            }
            Request::Notify { .. } => {
                // Check if the response includes a notification ID
                if let Some(id) = params.get("id").and_then(|v| v.as_str()) {
                    Ok(Response::NotifyDelivered { id: id.to_string() })
                } else {
                    Ok(Response::Ok)
                }
            }
            Request::GenerateTicket { .. } => {
                let ticket_json = params
                    .get("ticket_json")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Response::Ticket { ticket_json })
            }
            _ => Ok(Response::Ok),
        }
    }
}

/// Get the local hostname via `gethostname(2)`.
fn get_hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if ret == 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        std::str::from_utf8(&buf[..end]).ok().map(String::from)
    } else {
        None
    }
}

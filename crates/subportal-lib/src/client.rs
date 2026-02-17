//! Unix socket client for sending requests to the subportal daemon.
//!
//! The [`Client`] opens a new Unix connection for each request (one-shot
//! pattern), sends a Varlink JSON message, and reads the response. If the
//! daemon is unreachable, all calls return [`SubportalError::NoClient`].
//!
//! The target socket path is read from the `SUBPORTAL_SOCKET` environment
//! variable, falling back to [`crate::consts::default_socket_path`].

use std::path::{Path, PathBuf};

use tokio::io::BufReader;
use tokio::net::UnixStream;

use crate::consts::{default_socket_path, SOCKET_PATH_ENV};
use crate::protocol::{
    read_message, write_message, Request, Response, SubportalError, VarlinkResponse,
};

/// A client that connects to the subportal daemon over a Unix socket.
pub struct Client {
    path: PathBuf,
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
        Self { path }
    }

    /// Create a new client that connects to the given socket path.
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Send a request and receive the response. Opens a new connection each time.
    /// Returns `SubportalError::NoClient` if the daemon is unreachable.
    pub async fn call(&self, request: &Request) -> Result<Response, SubportalError> {
        let mut stream = UnixStream::connect(&self.path)
            .await
            .map_err(|_| SubportalError::NoClient)?;

        let varlink_req = request.to_varlink();
        write_message(&mut stream, &varlink_req)
            .await
            .map_err(|_| SubportalError::NoClient)?;

        let varlink_resp: VarlinkResponse = read_message(&mut BufReader::new(&mut stream))
            .await
            .map_err(|_| SubportalError::NoClient)?;

        // Check for error response
        if let Some(ref err_id) = varlink_resp.error {
            let params = varlink_resp.parameters.as_ref()
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            if let Some(e) = SubportalError::from_varlink(err_id, &params) {
                return Err(e);
            }
            return Err(SubportalError::NotSupported { capability: "unknown".into() });
        }

        // Parse success response based on what we sent
        match request {
            Request::Ping => {
                let params = varlink_resp.parameters.unwrap_or_default();
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
                Ok(Response::Ping { capabilities, version })
            }
            _ => Ok(Response::Ok),
        }
    }
}

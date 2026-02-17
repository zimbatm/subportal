use std::net::SocketAddr;

use tokio::net::TcpStream;

use crate::consts::{DEFAULT_PORT, PORT_ENV};
use crate::protocol::{
    read_message, write_message, Request, Response, SubportalError, VarlinkResponse,
};

/// A client that connects to the subportal daemon over TCP.
pub struct Client {
    addr: SocketAddr,
}

impl Client {
    /// Create a new client, reading port from `SUBPORTAL_PORT` env or using the default.
    pub fn new() -> Self {
        let port = std::env::var(PORT_ENV)
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
        }
    }

    /// Send a request and receive the response. Opens a new TCP connection each time.
    /// Returns `SubportalError::NoClient` if the daemon is unreachable.
    pub async fn call(&self, request: &Request) -> Result<Response, SubportalError> {
        let mut stream = TcpStream::connect(self.addr).await.map_err(|_| SubportalError::NoClient)?;

        let varlink_req = request.to_varlink();
        write_message(&mut stream, &varlink_req)
            .await
            .map_err(|_| SubportalError::NoClient)?;

        let varlink_resp: VarlinkResponse = read_message(&mut stream)
            .await
            .map_err(|_| SubportalError::NoClient)?;

        // Check for error response
        if let Some(ref err_id) = varlink_resp.error {
            if let Some(e) = SubportalError::from_varlink_id(err_id) {
                return Err(e);
            }
            return Err(SubportalError::NotSupported);
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

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tracing::info;

use crate::consts::DEFAULT_PORT;
use crate::protocol::{
    read_message, write_message, Request, Response, SubportalError, VarlinkRequest, VarlinkResponse,
};

/// A TCP server that accepts Varlink requests.
pub struct Server {
    listener: TcpListener,
}

/// Holds a parsed request and the TCP stream, allowing the handler to send a response.
pub struct Responder {
    stream: TcpStream,
}

impl Server {
    /// Bind to the given port on localhost.
    pub async fn bind(port: u16) -> anyhow::Result<Self> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(addr).await?;
        info!("listening on {addr}");
        Ok(Self { listener })
    }

    /// Bind using the default port.
    pub async fn bind_default() -> anyhow::Result<Self> {
        Self::bind(DEFAULT_PORT).await
    }

    /// Accept the next connection, read the request, and return it with a `Responder`.
    pub async fn accept(&self) -> anyhow::Result<(Request, Responder)> {
        let (mut stream, peer) = self.listener.accept().await?;
        tracing::debug!("connection from {peer}");

        let varlink_req: VarlinkRequest = read_message(&mut stream).await?;
        let request = Request::from_varlink(&varlink_req)?;

        Ok((request, Responder { stream }))
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

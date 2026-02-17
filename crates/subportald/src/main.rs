//! subportald -- the subportal client daemon.
//!
//! Runs on the user's desktop machine and listens for Varlink requests on a
//! Unix domain socket from server-side tools. Each request is dispatched to
//! the local xdg-desktop-portal D-Bus interface to open URLs, transfer files,
//! or show desktop notifications.

mod handler;
mod portal;

use anyhow::Result;
use clap::Parser;
use subportal_lib::consts::default_socket_path;
use subportal_lib::server::Server;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "subportald", about = "subportal client daemon")]
struct Cli {
    /// Unix socket path to listen on
    #[arg(short, long, default_value_os_t = default_socket_path())]
    socket: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let server = Server::bind(&cli.socket).await?;

    info!("subportald running on {}", cli.socket.display());

    loop {
        match server.accept().await {
            Ok((request, responder)) => {
                let ssh_host = responder.peer.ssh_host.clone();
                tokio::spawn(async move {
                    if let Some(ref host) = ssh_host {
                        info!("request from SSH host {host}: {request:?}");
                    }
                    match handler::handle(request).await {
                        Ok(response) => {
                            if let Err(e) = responder.send_ok(response).await {
                                error!("failed to send response: {e:#}");
                            }
                        }
                        Err(e) => {
                            if let Err(send_err) = responder.send_error(e).await {
                                error!("failed to send error response: {send_err:#}");
                            }
                        }
                    }
                });
            }
            Err(e) => {
                error!("failed to accept connection: {e:#}");
            }
        }
    }
}

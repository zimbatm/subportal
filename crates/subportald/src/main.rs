mod handler;
mod portal;

use anyhow::Result;
use clap::Parser;
use subportal_lib::consts::DEFAULT_PORT;
use subportal_lib::server::Server;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "subportald", about = "subportal client daemon")]
struct Cli {
    /// TCP port to listen on
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let server = Server::bind(cli.port).await?;

    info!("subportald running on port {}", cli.port);

    loop {
        match server.accept().await {
            Ok((request, responder)) => {
                tokio::spawn(async move {
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

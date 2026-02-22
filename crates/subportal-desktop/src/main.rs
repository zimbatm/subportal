//! subportal-desktop -- the subportal client daemon.
//!
//! Runs on the user's desktop machine. Connects to enrolled subportal agents
//! via iroh (peer-to-peer QUIC) and handles requests to open URLs, transfer
//! files, or show desktop notifications through the local desktop portal.

mod dismiss;
mod enroll;
mod focus;
mod handler;
mod portal;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use subportal_iroh::consts::{data_dir, ALPN, KEYPAIR_FILE};
use subportal_iroh::control::{
    read_control, write_control, ClientHello, ControlMessage, ServerHello,
};
use subportal_iroh::keypair::load_or_generate_keypair;
use subportal_iroh::peers::ServerRegistry;
use subportal_iroh::ticket::Ticket;
use subportal_iroh::transport;
use subportal_lib::protocol::Request;
use tokio::io::BufReader;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "subportal-desktop", about = "subportal client daemon")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Connect to enrolled servers (default)
    Run,
    /// Enroll with a server (reads ticket JSON from stdin)
    Enroll,
    /// Remove an enrolled server
    Forget {
        /// Server name or endpoint ID
        name_or_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => run().await,
        Command::Enroll => enroll::enroll_from_stdin().await,
        Command::Forget { name_or_id } => forget(&name_or_id).await,
    }
}

async fn run() -> Result<()> {
    let dir = data_dir();

    info!("subportal-desktop v{}", subportal_lib::consts::VERSION);
    info!("data dir: {}", dir.display());

    let key = load_or_generate_keypair(&dir.join(KEYPAIR_FILE)).await?;

    let registry = ServerRegistry::load(&dir).await?;
    let servers = registry.list().to_vec();

    if servers.is_empty() {
        info!("no enrolled servers -- use 'subportal-desktop enroll' to add one");
        // Still wait for signal so systemd doesn't restart us in a loop
        tokio::signal::ctrl_c().await?;
        return Ok(());
    }

    let endpoint = iroh::Endpoint::builder()
        .secret_key(key)
        .bind()
        .await
        .context("failed to bind iroh endpoint")?;

    info!(
        "subportal-desktop running, endpoint: {}, connecting to {} server(s)",
        endpoint.id(),
        servers.len()
    );

    let mut handles = Vec::new();

    for server in &servers {
        let ep = endpoint.clone();
        let server = server.clone();
        let handle = tokio::spawn(async move {
            loop {
                info!(name = %server.name, "connecting to server");
                match connect_to_server(&ep, &server).await {
                    Ok(()) => {
                        info!(name = %server.name, "disconnected from server");
                    }
                    Err(e) => {
                        warn!(name = %server.name, "connection error: {e:#}");
                    }
                }
                // Reconnect after a delay
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for shutdown
    tokio::signal::ctrl_c().await?;
    info!("shutting down");
    for h in handles {
        h.abort();
    }
    endpoint.close().await;

    Ok(())
}

async fn connect_to_server(
    endpoint: &iroh::Endpoint,
    server: &subportal_iroh::peers::ServerEntry,
) -> Result<()> {
    // Build EndpointAddr from server entry
    let ticket = Ticket {
        endpoint_id: server.endpoint_id.clone(),
        addrs: server.addrs.clone(),
        relay_url: server.relay_url.clone(),
        token: String::new(),
        hostname: server.name.clone(),
    };
    let addr = ticket.to_endpoint_addr()?;

    let conn = endpoint
        .connect(addr, ALPN)
        .await
        .context("failed to connect to server")?;

    info!(name = %server.name, "connected");

    // Open control bi-stream
    let (mut ctrl_send, ctrl_recv) = conn
        .open_bi()
        .await
        .context("failed to open control stream")?;

    let mut ctrl_reader = BufReader::new(ctrl_recv);

    // Send ClientHello
    let hello = ClientHello {
        name: gethostname(),
        capabilities: handler::CAPABILITIES
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        platform: std::env::consts::OS.to_string(),
        token: None, // reconnection, not enrollment
    };
    write_control(&mut ctrl_send, &hello).await?;

    // Read ServerHello
    let server_hello: ServerHello = read_control(&mut ctrl_reader)
        .await
        .context("failed to read ServerHello")?;

    if !server_hello.enrolled {
        anyhow::bail!("server rejected connection (not enrolled)");
    }

    info!(
        hostname = %server_hello.hostname,
        "server hello received"
    );

    // Spawn control message reader (dismiss notifications from other devices)
    let server_name = server.name.clone();
    tokio::spawn(async move {
        read_control_loop(ctrl_reader, &server_name).await;
    });

    // Spawn focus update sender
    let focus_send = ctrl_send;
    tokio::spawn(async move {
        focus::send_focus_updates(focus_send).await;
    });

    // Accept bi-streams (each = one request from server)
    let hostname = server_hello.hostname.clone();
    loop {
        let (send, recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(e) => {
                warn!("bi-stream accept error: {e:#}");
                break;
            }
        };

        let host = hostname.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_request_stream(send, recv, &host).await {
                warn!("request handler error: {e:#}");
            }
        });
    }

    Ok(())
}

async fn handle_request_stream(
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    host: &str,
) -> Result<()> {
    let req = transport::recv_request(&mut recv).await?;

    // Extract notification_id from raw parameters before parsing into typed Request.
    let notification_id = req
        .parameters
        .get("notification_id")
        .and_then(|v| v.as_str())
        .map(String::from);

    let request = Request::from_varlink(&req)?;

    info!("request from {host}: {request:?}");

    match handler::handle(request, Some(host), notification_id).await {
        Ok(response) => {
            let varlink_resp = response.to_varlink();
            transport::send_response(&mut send, &varlink_resp).await?;
        }
        Err(e) => {
            let varlink_resp = e.to_varlink();
            transport::send_response(&mut send, &varlink_resp).await?;
        }
    }

    Ok(())
}

async fn read_control_loop(mut reader: BufReader<iroh::endpoint::RecvStream>, server_name: &str) {
    loop {
        match read_control::<ControlMessage>(&mut reader).await {
            Ok(msg) => match msg {
                ControlMessage::DismissNotification { ref id } => {
                    info!(server = %server_name, "dismiss notification: {id}");
                    dismiss::dismiss_notification(id).await;
                }
                ControlMessage::FocusUpdate { .. } => {
                    // Server doesn't send focus updates to client
                }
            },
            Err(_) => {
                info!(server = %server_name, "control stream closed");
                break;
            }
        }
    }
}

async fn forget(name_or_id: &str) -> Result<()> {
    let dir = data_dir();
    let mut registry = ServerRegistry::load(&dir).await?;
    if registry.remove(name_or_id).await? {
        println!("Server '{}' removed.", name_or_id);
    } else {
        anyhow::bail!("No server found matching '{}'", name_or_id);
    }
    Ok(())
}

fn gethostname() -> String {
    let mut buf = [0u8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if ret == 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        std::str::from_utf8(&buf[..end])
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    }
}

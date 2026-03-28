//! subportal -- server-side CLI for the subportal protocol.
//!
//! Provides `status`, `open`, and `notify` subcommands for interacting with
//! the subportal agent from the server side, plus `agent`, `ticket`, `clients`,
//! and `revoke` subcommands for managing the agent daemon and enrolled clients.

mod agent;
mod enrollment;
mod hub;
mod router;

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::{Parser, Subcommand};
use subportal_lib::client::{Client, ClientError};
use subportal_lib::consts::MAX_FILE_SIZE;
use subportal_lib::protocol::{Request, Response, SubportalError};

#[derive(Parser)]
#[command(name = "subportal", about = "subportal CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check daemon connectivity, show capabilities and latency
    Status,
    /// Open a URL or file on the client
    Open {
        /// URL or file path to open
        target: String,
    },
    /// Send a notification to the client
    Notify {
        /// Notification title
        title: String,
        /// Notification body
        body: Option<String>,
        /// Urgency level (low, normal, critical)
        #[arg(short, long)]
        urgency: Option<String>,
        /// Icon name
        #[arg(short, long)]
        icon: Option<String>,
    },
    /// Start the agent daemon
    Agent {
        /// Use a custom relay server URL (e.g. https://relay.example.com)
        #[arg(long)]
        relay_url: Option<String>,
    },
    /// Generate an enrollment ticket (requires running agent)
    Ticket {
        /// Token TTL in seconds
        #[arg(long, default_value_t = subportal_iroh::consts::DEFAULT_TOKEN_TTL_SECS)]
        ttl: u64,
        /// Also display the ticket as a QR code in the terminal
        #[arg(long)]
        qr: bool,
    },
    /// List enrolled clients
    Clients,
    /// Revoke an enrolled client
    Revoke {
        /// Client name or endpoint ID
        name_or_id: String,
    },
}

fn is_url(target: &str) -> bool {
    target
        .find("://")
        .map(|pos| {
            pos > 0
                && target[..pos]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        })
        .unwrap_or(false)
}

fn daemon_unreachable_error(client: &Client) {
    let path = client.path();
    eprintln!("subportal: daemon is not reachable");
    if path.exists() {
        eprintln!("  socket: {} (exists but not responding)", path.display());
    } else {
        eprintln!("  socket: {} (not found)", path.display());
    }
    eprintln!("  hint: start the agent with: subportal agent");
}

fn no_desktop_error() {
    eprintln!("subportal: no desktop client connected");
    eprintln!("  hint: is subportal-desktop running on your local machine?");
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Command::Agent { relay_url } => match agent::run(relay_url.as_deref()).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("subportal agent: {e:#}");
                ExitCode::from(1)
            }
        },
        Command::Ticket { ttl, qr } => match enrollment::print_ticket(ttl, qr).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("subportal ticket: {e:#}");
                ExitCode::from(1)
            }
        },
        Command::Clients => match enrollment::list_clients().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("subportal clients: {e:#}");
                ExitCode::from(1)
            }
        },
        Command::Revoke { ref name_or_id } => match enrollment::revoke_client(name_or_id).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("subportal revoke: {e:#}");
                ExitCode::from(1)
            }
        },
        Command::Status => {
            let client = Client::new();
            let start = Instant::now();
            match client.call(&Request::Ping {}).await {
                Ok(Response::Ping {
                    capabilities,
                    version,
                    clients,
                    endpoint_id,
                }) => {
                    let latency = start.elapsed();
                    println!("subportal v{version}");
                    println!("latency: {:.1}ms", latency.as_secs_f64() * 1000.0);
                    println!("endpoint: {endpoint_id}");
                    println!("socket: {}", client.path().display());
                    if clients.is_empty() {
                        println!("clients: none");
                    } else {
                        println!("clients: {} ({})", clients.len(), clients.join(", "));
                    }
                    println!("capabilities: {}", capabilities.join(", "));
                    ExitCode::SUCCESS
                }
                Ok(_) => {
                    eprintln!("unexpected response from daemon");
                    ExitCode::from(1)
                }
                Err(ClientError::DaemonUnreachable) => {
                    daemon_unreachable_error(&client);
                    ExitCode::from(1)
                }
                Err(ClientError::Protocol(SubportalError::NoClient)) => {
                    no_desktop_error();
                    ExitCode::from(1)
                }
                Err(e) => {
                    eprintln!("subportal: {}", e);
                    ExitCode::from(1)
                }
            }
        }
        Command::Open { ref target } => {
            let client = Client::new();
            let request = if is_url(target) {
                Request::OpenURI {
                    uri: target.clone(),
                }
            } else {
                let path = Path::new(target);
                let data = match std::fs::read(path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("subportal: cannot read '{}': {}", target, e);
                        return ExitCode::from(1);
                    }
                };

                if data.len() > MAX_FILE_SIZE {
                    eprintln!(
                        "subportal: file '{}' is too large ({} bytes, max {} bytes)",
                        target,
                        data.len(),
                        MAX_FILE_SIZE
                    );
                    return ExitCode::from(1);
                }

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());

                let mime = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();

                let content = BASE64.encode(&data);
                Request::OpenFile {
                    name,
                    mime,
                    content,
                }
            };

            match client.call(&request).await {
                Ok(_) => ExitCode::SUCCESS,
                Err(ClientError::DaemonUnreachable) => {
                    daemon_unreachable_error(&client);
                    ExitCode::from(1)
                }
                Err(ClientError::Protocol(SubportalError::NoClient)) => {
                    no_desktop_error();
                    ExitCode::from(1)
                }
                Err(ClientError::Protocol(SubportalError::UserDenied)) => {
                    eprintln!("subportal: request was denied by the user");
                    ExitCode::from(1)
                }
                Err(e) => {
                    eprintln!("subportal: {}", e);
                    ExitCode::from(1)
                }
            }
        }
        Command::Notify {
            ref title,
            ref body,
            ref urgency,
            ref icon,
        } => {
            let client = Client::new();
            let request = Request::Notify {
                title: title.clone(),
                body: body.clone(),
                urgency: urgency.clone(),
                icon: icon.clone(),
            };

            match client.call(&request).await {
                Ok(_) => ExitCode::SUCCESS,
                Err(ClientError::DaemonUnreachable) => {
                    daemon_unreachable_error(&client);
                    ExitCode::from(1)
                }
                Err(ClientError::Protocol(SubportalError::NoClient)) => {
                    no_desktop_error();
                    ExitCode::from(1)
                }
                Err(e) => {
                    eprintln!("subportal: {}", e);
                    ExitCode::from(1)
                }
            }
        }
    }
}

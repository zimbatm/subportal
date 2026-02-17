//! subportal -- explicit CLI for the subportal protocol.
//!
//! Provides `status`, `open`, and `notify` subcommands for interacting with
//! the subportal daemon from the server side. Unlike the drop-in replacements,
//! this binary is invoked explicitly and provides richer output (e.g. latency
//! and capability reporting via `status`).

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use clap::{Parser, Subcommand};
use subportal_lib::client::Client;
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

fn no_client_error(client: &Client) {
    let path = client.path();
    eprintln!("subportal: daemon is not reachable");
    if path.exists() {
        eprintln!("  socket: {} (exists but not responding)", path.display());
        eprintln!("  hint: is subportald running? check with: systemctl --user status subportald");
    } else {
        eprintln!("  socket: {} (not found)", path.display());
        eprintln!("  hint: is the SSH tunnel active? configure with:");
        eprintln!("    RemoteForward {} <local-socket-path>", path.display());
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Command::Status => {
            let start = Instant::now();
            match client.call(&Request::Ping).await {
                Ok(Response::Ping {
                    capabilities,
                    version,
                }) => {
                    let latency = start.elapsed();
                    println!("subportald v{version}");
                    println!("latency: {:.1}ms", latency.as_secs_f64() * 1000.0);
                    println!("capabilities: {}", capabilities.join(", "));
                    ExitCode::SUCCESS
                }
                Ok(_) => {
                    eprintln!("unexpected response from daemon");
                    ExitCode::from(1)
                }
                Err(SubportalError::NoClient) => {
                    no_client_error(&client);
                    ExitCode::from(1)
                }
                Err(e) => {
                    eprintln!("subportal: {}", e);
                    ExitCode::from(1)
                }
            }
        }
        Command::Open { ref target } => {
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
                Err(SubportalError::NoClient) => {
                    no_client_error(&client);
                    ExitCode::from(1)
                }
                Err(SubportalError::UserDenied) => {
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
            let request = Request::Notify {
                title: title.clone(),
                body: body.clone(),
                urgency: urgency.clone(),
                icon: icon.clone(),
            };

            match client.call(&request).await {
                Ok(_) => ExitCode::SUCCESS,
                Err(SubportalError::NoClient) => {
                    no_client_error(&client);
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

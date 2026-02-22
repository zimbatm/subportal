use anyhow::Result;
use chrono::Utc;
use subportal_iroh::consts::data_dir;
use subportal_iroh::peers::{ClientEntry, ClientRegistry};
use subportal_lib::client::{Client, ClientError};
use subportal_lib::protocol::{Request, Response};

/// Generate and print an enrollment ticket to stdout.
///
/// Connects to the running agent via its Unix socket and asks it to generate
/// a ticket with a pending token. The token is stored in the agent's in-memory
/// list so it can be validated when the enrolling client connects.
pub async fn print_ticket(ttl: u64) -> Result<()> {
    let client = Client::new();
    let request = Request::GenerateTicket { ttl };

    match client.call(&request).await {
        Ok(Response::Ticket { ticket_json }) => {
            println!("{ticket_json}");
            Ok(())
        }
        Ok(_) => {
            anyhow::bail!("unexpected response from agent");
        }
        Err(ClientError::DaemonUnreachable) => {
            let path = client.path();
            eprintln!("Failed to generate ticket: daemon is not reachable");
            if path.exists() {
                eprintln!("  socket: {} (exists but not responding)", path.display());
            } else {
                eprintln!("  socket: {} (not found)", path.display());
            }
            eprintln!();
            eprintln!("Is the agent running? Start it with: subportal agent");
            anyhow::bail!("could not connect to running agent");
        }
        Err(e) => {
            anyhow::bail!("failed to generate ticket: {e}");
        }
    }
}

/// List enrolled clients.
pub async fn list_clients() -> Result<()> {
    let dir = data_dir();
    let registry = ClientRegistry::load(&dir).await?;

    let clients = registry.list();
    if clients.is_empty() {
        println!("No enrolled clients.");
        return Ok(());
    }

    for client in clients {
        let last_seen = client
            .last_seen
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "never".to_string());
        println!(
            "{} ({})\n  enrolled: {}\n  last seen: {}\n  capabilities: {}",
            client.name,
            &client.endpoint_id[..16.min(client.endpoint_id.len())],
            client.enrolled_at.to_rfc3339(),
            last_seen,
            client.capabilities.join(", "),
        );
    }

    Ok(())
}

/// Revoke an enrolled client by name or endpoint ID.
///
/// Connects to the running agent via its Unix socket and asks it to revoke the
/// client. This removes the client from the persistent registry and disconnects
/// it immediately if currently connected.
pub async fn revoke_client(name_or_id: &str) -> Result<()> {
    let client = Client::new();
    let request = Request::RevokeClient {
        name_or_id: name_or_id.to_string(),
    };

    match client.call(&request).await {
        Ok(Response::Ok) => {
            println!("Client '{}' revoked.", name_or_id);
            Ok(())
        }
        Ok(_) => {
            anyhow::bail!("unexpected response from agent");
        }
        Err(ClientError::Protocol(subportal_lib::protocol::SubportalError::NotFound { what })) => {
            anyhow::bail!("{what}");
        }
        Err(ClientError::DaemonUnreachable) => {
            let path = client.path();
            eprintln!("Failed to revoke client: daemon is not reachable");
            if path.exists() {
                eprintln!("  socket: {} (exists but not responding)", path.display());
            } else {
                eprintln!("  socket: {} (not found)", path.display());
            }
            eprintln!();
            eprintln!("Is the agent running? Start it with: subportal agent");
            anyhow::bail!("could not connect to running agent");
        }
        Err(e) => {
            anyhow::bail!("failed to revoke client: {e}");
        }
    }
}

/// Create a ClientEntry for a newly enrolled client.
pub fn enroll_client(endpoint_id: &str, name: &str, capabilities: Vec<String>) -> ClientEntry {
    ClientEntry {
        endpoint_id: endpoint_id.to_string(),
        name: name.to_string(),
        enrolled_at: Utc::now(),
        last_seen: Some(Utc::now()),
        capabilities,
    }
}

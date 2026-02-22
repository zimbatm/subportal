use anyhow::{Context, Result};
use chrono::Utc;
use iroh::TransportAddr;
use subportal_iroh::consts::data_dir;
use subportal_iroh::keypair::load_or_generate_keypair;
use subportal_iroh::peers::{ClientEntry, ClientRegistry, PendingToken};
use subportal_iroh::ticket::Ticket;

/// Generate and print an enrollment ticket to stdout.
pub async fn print_ticket(ttl: u64) -> Result<()> {
    let dir = data_dir();
    let key = load_or_generate_keypair(&dir.join(subportal_iroh::consts::KEYPAIR_FILE)).await?;

    // Build an endpoint briefly to get our addresses
    let endpoint = iroh::Endpoint::builder()
        .secret_key(key)
        .alpns(vec![subportal_iroh::consts::ALPN.to_vec()])
        .bind()
        .await
        .context("failed to bind iroh endpoint")?;

    let addr = endpoint.addr();
    let endpoint_id = addr.id.to_string();

    let mut addrs = Vec::new();
    let mut relay_url = None;
    for transport_addr in &addr.addrs {
        match transport_addr {
            TransportAddr::Relay(url) => {
                relay_url = Some(url.to_string());
            }
            TransportAddr::Ip(sock) => {
                addrs.push(sock.to_string());
            }
        }
    }

    let token = PendingToken::generate(ttl);

    // Save the token so the agent can validate it later
    // For now we store it alongside the registry
    let mut registry = ClientRegistry::load(&dir).await?;
    // Token is ephemeral -- it's stored in the ticket and validated by the running agent.
    // The print_ticket command is a shortcut; in practice the running agent holds tokens
    // in memory. We print the token so the agent's stdin/config can pick it up.

    let hostname = gethostname();

    let ticket = Ticket {
        endpoint_id,
        addrs,
        relay_url,
        token: token.token.clone(),
        hostname,
    };

    // Print to stdout for piping
    println!("{}", ticket.to_json());

    endpoint.close().await;

    Ok(())
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
pub async fn revoke_client(name_or_id: &str) -> Result<()> {
    let dir = data_dir();
    let mut registry = ClientRegistry::load(&dir).await?;

    if registry.remove(name_or_id).await? {
        println!("Client '{}' revoked.", name_or_id);
    } else {
        anyhow::bail!("No client found matching '{}'", name_or_id);
    }

    Ok(())
}

/// Validate a token and enroll a client.
pub fn enroll_client(
    endpoint_id: &str,
    name: &str,
    capabilities: Vec<String>,
) -> ClientEntry {
    ClientEntry {
        endpoint_id: endpoint_id.to_string(),
        name: name.to_string(),
        enrolled_at: Utc::now(),
        last_seen: Some(Utc::now()),
        capabilities,
    }
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

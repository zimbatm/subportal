use anyhow::{Context, Result};
use chrono::Utc;
use subportal_iroh::consts::{data_dir, ALPN, KEYPAIR_FILE};
use subportal_iroh::control::{read_control, write_control, ClientHello, ServerHello};
use subportal_iroh::keypair::load_or_generate_keypair;
use subportal_iroh::peers::{ServerEntry, ServerRegistry};
use subportal_iroh::ticket::Ticket;
use tokio::io::BufReader;

/// Enroll with a server by reading a ticket from stdin.
pub async fn enroll_from_stdin() -> Result<()> {
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
        .context("failed to read ticket from stdin")?;

    let ticket = Ticket::from_json(input.trim())?;

    eprintln!(
        "Enrolling with server '{}' ({})",
        ticket.hostname,
        &ticket.endpoint_id[..16.min(ticket.endpoint_id.len())]
    );

    // Load or generate our keypair
    let dir = data_dir();
    let key = load_or_generate_keypair(&dir.join(KEYPAIR_FILE)).await?;

    // Try to connect and enroll
    let endpoint = iroh::Endpoint::builder()
        .secret_key(key)
        .bind()
        .await
        .context("failed to bind iroh endpoint")?;

    let addr = ticket.to_endpoint_addr()?;
    let conn = endpoint
        .connect(addr, ALPN)
        .await
        .context("failed to connect to server")?;

    // Open control stream and send enrollment hello
    let (mut ctrl_send, ctrl_recv) = conn
        .open_bi()
        .await
        .context("failed to open control stream")?;

    let mut ctrl_reader = BufReader::new(ctrl_recv);

    let hello = ClientHello {
        name: crate::gethostname(),
        capabilities: crate::handler::CAPABILITIES
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        platform: std::env::consts::OS.to_string(),
        token: Some(ticket.token.clone()),
    };
    write_control(&mut ctrl_send, &hello).await?;

    let server_hello: ServerHello = read_control(&mut ctrl_reader)
        .await
        .context("failed to read server response")?;

    if !server_hello.enrolled {
        anyhow::bail!("enrollment rejected by server (invalid or expired token)");
    }

    // Save server to registry
    let entry = ServerEntry {
        endpoint_id: ticket.endpoint_id,
        name: ticket.hostname.clone(),
        addrs: ticket.addrs,
        relay_url: ticket.relay_url,
        enrolled_at: Utc::now(),
    };

    let mut registry = ServerRegistry::load(&dir).await?;
    registry.add(entry).await?;

    conn.close(0u8.into(), b"enrolled");
    endpoint.close().await;

    eprintln!("Enrolled with server '{}'.", ticket.hostname);

    Ok(())
}

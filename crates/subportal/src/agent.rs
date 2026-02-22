use anyhow::{Context, Result};
use iroh::endpoint::Connection;
use subportal_iroh::consts::{data_dir, ALPN, KEYPAIR_FILE};
use subportal_iroh::control::{
    read_control, write_control, ClientHello, ControlMessage, FocusState, ServerHello,
};
use subportal_iroh::keypair::load_or_generate_keypair;
use subportal_iroh::peers::ClientRegistry;
use subportal_lib::server::Server;
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::hub::{self, ConnectedClient, SharedHub};

/// Run the agent: listen on Unix socket + iroh endpoint.
pub async fn run() -> Result<()> {
    let dir = data_dir();
    let key = load_or_generate_keypair(&dir.join(KEYPAIR_FILE)).await?;

    let registry = ClientRegistry::load(&dir).await?;

    // Bind the Unix socket for local tools
    let server = Server::bind_default().await?;
    info!("Unix socket: {}", server.path().display());

    // Create iroh endpoint
    let endpoint = iroh::Endpoint::builder()
        .secret_key(key)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("failed to bind iroh endpoint")?;

    let addr = endpoint.addr();
    info!("iroh endpoint: {}", addr.id);

    let hostname = gethostname();
    let hub = hub::shared(registry, endpoint.clone(), hostname);

    // Spawn iroh accept loop
    let iroh_hub = hub.clone();
    let iroh_endpoint = endpoint.clone();
    tokio::spawn(async move {
        accept_iroh_loop(iroh_endpoint, iroh_hub).await;
    });

    // Spawn Unix socket accept loop
    let unix_hub = hub.clone();
    tokio::spawn(async move {
        accept_unix_loop(server, unix_hub).await;
    });

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("shutting down");
    endpoint.close().await;

    Ok(())
}

async fn accept_iroh_loop(endpoint: iroh::Endpoint, hub: SharedHub) {
    loop {
        let incoming = match endpoint.accept().await {
            Some(incoming) => incoming,
            None => {
                info!("iroh endpoint closed");
                break;
            }
        };

        let conn = match incoming.await {
            Ok(conn) => conn,
            Err(e) => {
                warn!("failed to accept iroh connection: {e:#}");
                continue;
            }
        };

        let hub = hub.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_iroh_connection(conn, hub).await {
                warn!("iroh connection handler error: {e:#}");
            }
        });
    }
}

async fn handle_iroh_connection(conn: Connection, hub: SharedHub) -> Result<()> {
    let remote_id = conn.remote_id();
    let remote_id_str = remote_id.to_string();
    info!(remote = %remote_id_str, "iroh connection established");

    // Open control bi-stream (the first bi-stream is the control channel)
    let (mut ctrl_send, ctrl_recv) = conn
        .accept_bi()
        .await
        .context("failed to accept control bi-stream")?;

    let mut ctrl_reader = BufReader::new(ctrl_recv);

    // Read ClientHello
    let client_hello: ClientHello = read_control(&mut ctrl_reader)
        .await
        .context("failed to read ClientHello")?;

    info!(
        name = %client_hello.name,
        platform = %client_hello.platform,
        token = ?client_hello.token.is_some(),
        "received ClientHello"
    );

    // Validate enrollment or reconnection
    let enrolled = if let Some(ref token) = client_hello.token {
        // Enrollment: validate token via the Hub's pending_tokens
        let mut hub_lock = hub.lock().await;
        let valid = hub_lock.consume_token(token);
        let hostname = hub_lock.hostname.clone();
        if !valid {
            drop(hub_lock);
            let hello = ServerHello {
                hostname,
                enrolled: false,
            };
            write_control(&mut ctrl_send, &hello).await?;
            anyhow::bail!("invalid or expired enrollment token");
        }

        // Register the client
        let entry = crate::enrollment::enroll_client(
            &remote_id_str,
            &client_hello.name,
            client_hello.capabilities.clone(),
        );
        hub_lock.registry.add(entry).await?;
        drop(hub_lock);

        info!(name = %client_hello.name, "client enrolled");
        true
    } else {
        // Reconnection: check if client is in registry
        let hub_lock = hub.lock().await;
        let known = hub_lock.registry.find_by_id(&remote_id_str).is_some();
        let hostname = hub_lock.hostname.clone();
        drop(hub_lock);

        if !known {
            let hello = ServerHello {
                hostname,
                enrolled: false,
            };
            write_control(&mut ctrl_send, &hello).await?;
            anyhow::bail!("unknown client {}", remote_id_str);
        }
        true
    };

    // Send ServerHello
    let hostname = hub.lock().await.hostname.clone();
    let hello = ServerHello { hostname, enrolled };
    write_control(&mut ctrl_send, &hello).await?;

    // Create control message channel
    let (control_tx, mut control_rx) = mpsc::channel::<ControlMessage>(32);

    // Register connected client
    let connected = ConnectedClient {
        endpoint_id: remote_id_str.clone(),
        name: client_hello.name.clone(),
        connection: conn.clone(),
        focus: FocusState::Active,
        capabilities: client_hello.capabilities.clone(),
        control_tx,
    };

    {
        let mut hub_lock = hub.lock().await;
        hub_lock.add_client(connected);
        hub_lock.registry.touch(&remote_id_str).await.ok();
    }

    // Spawn writer for outgoing control messages
    let writer_eid = remote_id_str.clone();
    tokio::spawn(async move {
        while let Some(msg) = control_rx.recv().await {
            if let Err(e) = write_control(&mut ctrl_send, &msg).await {
                warn!(endpoint_id = %writer_eid, "control write error: {e:#}");
                break;
            }
        }
    });

    // Read control messages from the client
    let reader_hub = hub.clone();
    let reader_eid = remote_id_str.clone();
    loop {
        match read_control::<ControlMessage>(&mut ctrl_reader).await {
            Ok(msg) => match msg {
                ControlMessage::FocusUpdate { state } => {
                    let mut hub_lock = reader_hub.lock().await;
                    hub_lock.update_focus(&reader_eid, state);
                }
                ControlMessage::DismissNotification { ref id } => {
                    let hub_lock = reader_hub.lock().await;
                    hub_lock.broadcast_dismiss(id, Some(&reader_eid)).await;
                }
            },
            Err(_) => {
                info!(endpoint_id = %reader_eid, "control stream closed");
                break;
            }
        }
    }

    // Client disconnected, remove from hub
    {
        let mut hub_lock = hub.lock().await;
        hub_lock.remove_client(&remote_id_str);
    }

    Ok(())
}

async fn accept_unix_loop(server: Server, hub: SharedHub) {
    loop {
        match server.accept().await {
            Ok((request, host, responder)) => {
                let hub = hub.clone();
                tokio::spawn(async move {
                    if let Some(ref h) = host {
                        info!("unix request from {h}: {request:?}");
                    }
                    let mut hub_lock = hub.lock().await;
                    match hub_lock.route_request(&request).await {
                        Ok(response) => {
                            if let Err(e) = responder.send_ok(response).await {
                                error!("failed to send response: {e:#}");
                            }
                        }
                        Err(e) => {
                            if let Err(send_err) = responder.send_error(e).await {
                                error!("failed to send error: {send_err:#}");
                            }
                        }
                    }
                });
            }
            Err(e) => {
                error!("failed to accept unix connection: {e:#}");
            }
        }
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

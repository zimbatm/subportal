use anyhow::{Context, Result};
use iroh::endpoint::Connection;
use iroh::{RelayMap, RelayMode};
use std::time::Instant;
use subportal_iroh::consts::{data_dir, ALPN, KEYPAIR_FILE};
use subportal_iroh::control::{
    read_control, write_control, ClientHello, ControlMessage, FocusState, ServerHello,
};
use subportal_iroh::keypair::load_or_generate_keypair;
use subportal_iroh::peers::ClientRegistry;
use subportal_lib::protocol::{Request, Response, SubportalError};
use subportal_lib::server::Server;
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::hub::{self, send_to_connection, ConnectedClient, SharedHub};
use crate::router::{self, Strategy};

/// Run the agent: listen on Unix socket + iroh endpoint.
pub async fn run(relay_url: Option<&str>) -> Result<()> {
    let dir = data_dir();
    let key = load_or_generate_keypair(&dir.join(KEYPAIR_FILE)).await?;

    let registry = ClientRegistry::load(&dir).await?;

    // Bind the Unix socket for local tools
    let server = Server::bind_default().await?;
    info!("Unix socket: {}", server.path().display());

    // Create iroh endpoint
    let mut builder = iroh::Endpoint::builder()
        .secret_key(key)
        .alpns(vec![ALPN.to_vec()]);

    if let Some(url) = relay_url {
        info!("using custom relay: {url}");
        let relay_map = RelayMap::try_from_iter([url]).context("invalid relay URL")?;
        builder = builder.relay_mode(RelayMode::Custom(relay_map));
    }

    let endpoint = builder
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
        platform: client_hello.platform.clone(),
        connection: conn.clone(),
        focus: FocusState::Active,
        capabilities: client_hello.capabilities.clone(),
        last_active: Instant::now(),
        control_tx,
    };

    {
        let mut hub_lock = hub.lock().await;
        hub_lock.add_client(connected);
        hub_lock
            .registry
            .touch(&remote_id_str, &client_hello.capabilities)
            .await
            .ok();
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
                    match handle_unix_request(&hub, &request).await {
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

/// Snapshot the capable clients for `cap`, best-first, cloning their iroh
/// `Connection`s so QUIC I/O can proceed without holding the hub lock. The order
/// is the routing/failover order from [`router::rank`].
async fn ranked_connections(hub: &SharedHub, cap: &str) -> Vec<(String, Connection)> {
    let hub_lock = hub.lock().await;
    let infos = hub_lock.client_infos();
    router::rank(&infos, cap)
        .into_iter()
        .filter_map(|info| {
            hub_lock
                .clients
                .get(&info.endpoint_id)
                .map(|c| (info.endpoint_id.clone(), c.connection.clone()))
        })
        .collect()
}

/// Handle a unix request, releasing the hub mutex before performing QUIC I/O.
async fn handle_unix_request(
    hub: &SharedHub,
    request: &Request,
) -> Result<Response, SubportalError> {
    let strategy = router::strategy_for(request);

    match strategy {
        Strategy::Direct => {
            // Fast path: lock hub, handle directly, unlock. No QUIC I/O.
            let mut hub_lock = hub.lock().await;
            hub_lock.handle_direct(request).await
        }
        Strategy::Failover(cap) => {
            // Single target: try the best device, fail over to the next only on
            // a transport failure. A user decision (approve/deny) is final.
            let targets = ranked_connections(hub, cap).await;
            let value = serde_json::to_value(request).map_err(|_| SubportalError::NoClient)?;
            router::failover_decision(targets, |connection| {
                let value = value.clone();
                async move { send_to_connection(&connection, &value).await }
            })
            .await
        }
        Strategy::Race(cap) => {
            // Send to every capable device at once; the first user *decision*
            // (approve or deny) wins. A transport failure doesn't decide — keep
            // waiting on the others. Losing dialogs are abandoned when the
            // JoinSet drops; they clear on their own client-side timeout.
            // TODO: a Cancel control message would dismiss them immediately.
            let targets = ranked_connections(hub, cap).await;
            let value = serde_json::to_value(request).map_err(|_| SubportalError::NoClient)?;
            let mut set = tokio::task::JoinSet::new();
            for (eid, connection) in targets {
                let value = value.clone();
                set.spawn(async move { (eid, send_to_connection(&connection, &value).await) });
            }
            router::race_decision(set).await
        }
        Strategy::FanOut(cap) => {
            // Lock hub, find targets, clone Connections, allocate notification_id,
            // then unlock before QUIC I/O.
            let (connections, notification_id, value) = {
                let mut hub_lock = hub.lock().await;
                let infos = hub_lock.client_infos();
                let targets = router::fan_out(&infos, cap);
                if targets.is_empty() {
                    return Err(SubportalError::NoClient);
                }

                let notification_id = hub_lock.next_notification_id();

                // Collect endpoint_ids and clone their Connections
                let conns: Vec<(String, Connection)> = targets
                    .iter()
                    .filter_map(|t| {
                        hub_lock
                            .clients
                            .get(&t.endpoint_id)
                            .map(|c| (t.endpoint_id.clone(), c.connection.clone()))
                    })
                    .collect();

                // Serialize the request and inject notification_id
                let mut value =
                    serde_json::to_value(request).map_err(|_| SubportalError::NoClient)?;
                if let Some(params) = value.get_mut("parameters").and_then(|v| v.as_object_mut()) {
                    params.insert(
                        "notification_id".into(),
                        serde_json::Value::String(notification_id.clone()),
                    );
                }

                (conns, notification_id, value)
            };
            // Hub mutex is released here; perform QUIC I/O without the lock.
            let mut sent_to = Vec::new();
            for (eid, connection) in &connections {
                match send_to_connection(connection, &value).await {
                    Ok(_) => {
                        sent_to.push(eid.clone());
                    }
                    Err(e) => {
                        warn!(endpoint_id = %eid, "failed to fan-out to client: {e}");
                    }
                }
            }

            // Re-lock to store notification state
            {
                let mut hub_lock = hub.lock().await;
                hub_lock
                    .pending_notifications
                    .insert(notification_id.clone(), hub::NotificationState { sent_to });
            }

            Ok(Response::NotifyDelivered {
                id: notification_id,
            })
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

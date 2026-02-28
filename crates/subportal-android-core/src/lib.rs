//! Android bridge for subportal.
//!
//! Wraps the iroh connection lifecycle into a UniFFI-exported API that Kotlin
//! calls from a Foreground Service. Incoming requests are dispatched to the
//! Kotlin layer via the [`SubportalCallback`] interface.

uniffi::setup_scaffolding!();

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use subportal_iroh::consts::{ALPN, KEYPAIR_FILE};
use subportal_iroh::control::{
    read_control, write_control, ClientHello, ControlMessage, FocusState, ServerHello,
};
use subportal_iroh::keypair::load_or_generate_keypair;
use subportal_iroh::peers::{ServerEntry, ServerRegistry};
use subportal_iroh::ticket::Ticket;
use subportal_iroh::transport;
use subportal_lib::consts::{MAX_FILE_SIZE, VERSION};
use subportal_lib::protocol::{Request, Response, SubportalError as ProtoError};
use tokio::io::BufReader;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// UniFFI error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SubportalError {
    #[error("{msg}")]
    General { msg: String },
}

impl From<anyhow::Error> for SubportalError {
    fn from(e: anyhow::Error) -> Self {
        SubportalError::General {
            msg: format!("{e:#}"),
        }
    }
}

// ---------------------------------------------------------------------------
// UniFFI data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, uniffi::Record)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub connected: bool,
    /// ISO 8601 timestamp of when this server was enrolled.
    pub enrolled_at: String,
}

// ---------------------------------------------------------------------------
// UniFFI callback interface (Kotlin implements this)
// ---------------------------------------------------------------------------

#[uniffi::export(callback_interface)]
pub trait SubportalCallback: Send + Sync {
    /// A URI should be opened. Return true if handled.
    fn on_open_uri(&self, uri: String, host: String) -> bool;
    /// A file should be opened. Content is base64-encoded. Return true if handled.
    fn on_open_file(
        &self,
        name: String,
        mime: String,
        content_base64: String,
        host: String,
    ) -> bool;
    /// A notification should be shown.
    fn on_notify(
        &self,
        notification_id: String,
        title: String,
        body: Option<String>,
        urgency: Option<String>,
        host: String,
    );
    /// A notification should be dismissed.
    fn on_dismiss_notification(&self, id: String);
    /// Connection state changed for a server.
    fn on_connection_changed(&self, server_name: String, connected: bool);
}

// ---------------------------------------------------------------------------
// Capabilities advertised by the Android client
// ---------------------------------------------------------------------------

const CAPABILITIES: &[&str] = &["OpenURI", "OpenFile", "Notify"];

// ---------------------------------------------------------------------------
// SubportalCore (Kotlin calls this)
// ---------------------------------------------------------------------------

#[derive(uniffi::Object)]
pub struct SubportalCore {
    data_dir: PathBuf,
    device_name: String,
    callback: Arc<dyn SubportalCallback>,
    /// Tracks which servers are currently connected.
    connected: Mutex<HashMap<String, bool>>,
    /// Watch channel to signal the tokio runtime to stop.
    shutdown_tx: Mutex<Option<watch::Sender<bool>>>,
    /// Handle to the tokio runtime thread.
    runtime_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// Focus state driven by Kotlin.
    focus_tx: Mutex<Option<watch::Sender<FocusState>>>,
}

#[uniffi::export]
impl SubportalCore {
    #[uniffi::constructor]
    pub fn new(
        data_dir: String,
        device_name: String,
        callback: Box<dyn SubportalCallback>,
    ) -> Arc<Self> {
        // Initialize tracing (best-effort, ignore if already set).
        let _ = tracing_subscriber::fmt::try_init();

        Arc::new(Self {
            data_dir: PathBuf::from(data_dir),
            device_name,
            callback: Arc::from(callback),
            connected: Mutex::new(HashMap::new()),
            shutdown_tx: Mutex::new(None),
            runtime_handle: Mutex::new(None),
            focus_tx: Mutex::new(None),
        })
    }

    /// Start the connection loop. Spawns a tokio runtime on a background thread.
    pub fn start(self: Arc<Self>) {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (focus_tx, focus_rx) = watch::channel(FocusState::Active);

        {
            let mut tx = self.shutdown_tx.lock().unwrap();
            *tx = Some(shutdown_tx);
        }
        {
            let mut ftx = self.focus_tx.lock().unwrap();
            *ftx = Some(focus_tx);
        }

        let core = self.clone();
        let handle = std::thread::Builder::new()
            .name("subportal-rt".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build tokio runtime");
                rt.block_on(core.run_loop(shutdown_rx, focus_rx));
            })
            .expect("failed to spawn runtime thread");

        let mut rh = self.runtime_handle.lock().unwrap();
        *rh = Some(handle);
    }

    /// Stop the connection loop and shut down the tokio runtime.
    pub fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.runtime_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }

    /// Enroll with a server using a ticket JSON string. Returns the server name.
    pub fn enroll(self: Arc<Self>, ticket_json: String) -> Result<String, SubportalError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| SubportalError::General {
                msg: format!("failed to create runtime: {e}"),
            })?;
        rt.block_on(self.enroll_async(&ticket_json))
    }

    /// Remove an enrolled server by name or endpoint ID. Returns true if found.
    pub fn forget_server(self: Arc<Self>, id_or_name: String) -> Result<bool, SubportalError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| SubportalError::General {
                msg: format!("failed to create runtime: {e}"),
            })?;
        rt.block_on(async {
            let mut registry = ServerRegistry::load(&self.data_dir).await?;
            let removed = registry.remove(&id_or_name).await?;
            Ok(removed)
        })
    }

    /// List all enrolled servers with their connection status.
    pub fn list_servers(&self) -> Vec<ServerInfo> {
        let connected = self.connected.lock().unwrap();
        let dir = self.data_dir.clone();
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return vec![],
        };
        let servers = rt.block_on(async {
            ServerRegistry::load(&dir)
                .await
                .map(|r| r.list().to_vec())
                .unwrap_or_default()
        });
        servers
            .into_iter()
            .map(|s| {
                let is_connected = connected.get(&s.endpoint_id).copied().unwrap_or(false);
                ServerInfo {
                    id: s.endpoint_id,
                    name: s.name,
                    connected: is_connected,
                    enrolled_at: s.enrolled_at.to_rfc3339(),
                }
            })
            .collect()
    }

    /// Update focus state (called by Kotlin when screen on/off changes).
    pub fn set_focus_active(&self, active: bool) {
        if let Some(tx) = self.focus_tx.lock().unwrap().as_ref() {
            let state = if active {
                FocusState::Active
            } else {
                FocusState::Idle
            };
            let _ = tx.send(state);
        }
    }

    /// Stop and restart the connection loop. Useful when the QUIC connection
    /// goes stale (NAT timeout, network change).
    pub fn reconnect(self: Arc<Self>) {
        self.stop();
        self.clone().start();
    }

    /// Notify the iroh endpoint that the network changed (e.g. WiFi <-> cellular).
    pub fn network_changed(&self) {
        info!("network_changed hint received");
    }
}

// ---------------------------------------------------------------------------
// Internal async implementation
// ---------------------------------------------------------------------------

impl SubportalCore {
    async fn run_loop(
        self: Arc<Self>,
        mut shutdown_rx: watch::Receiver<bool>,
        focus_rx: watch::Receiver<FocusState>,
    ) {
        info!("subportal-android-core starting, v{VERSION}");
        info!("data dir: {}", self.data_dir.display());

        let key = match load_or_generate_keypair(&self.data_dir.join(KEYPAIR_FILE)).await {
            Ok(k) => k,
            Err(e) => {
                warn!("failed to load keypair: {e:#}");
                return;
            }
        };

        let endpoint = match iroh::Endpoint::builder().secret_key(key).bind().await {
            Ok(ep) => ep,
            Err(e) => {
                warn!("failed to bind iroh endpoint: {e:#}");
                return;
            }
        };

        info!("endpoint: {}", endpoint.id());

        let mut active: HashMap<String, JoinHandle<()>> = HashMap::new();

        // Initial load
        let servers = match ServerRegistry::load(&self.data_dir).await {
            Ok(reg) => reg.list().to_vec(),
            Err(e) => {
                warn!("failed to load server registry: {e:#}");
                vec![]
            }
        };

        if servers.is_empty() {
            info!("no enrolled servers");
        } else {
            info!("connecting to {} server(s)", servers.len());
        }

        for server in &servers {
            let handle = self
                .clone()
                .spawn_server_connection(&endpoint, server, focus_rx.clone());
            active.insert(server.endpoint_id.clone(), handle);
        }

        // Wait for shutdown signal. Periodically reload the registry in case
        // enroll/forget was called while running.
        let mut reload_interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("shutdown signal received");
                        break;
                    }
                }
                _ = reload_interval.tick() => {
                    if let Err(e) = self.reload_servers(&endpoint, &mut active, &focus_rx).await {
                        warn!("reload error: {e:#}");
                    }
                }
            }
        }

        // Cleanup
        for (_, h) in &active {
            h.abort();
        }
        endpoint.close().await;
        info!("subportal-android-core stopped");
    }

    fn spawn_server_connection(
        self: Arc<Self>,
        endpoint: &iroh::Endpoint,
        server: &ServerEntry,
        focus_rx: watch::Receiver<FocusState>,
    ) -> JoinHandle<()> {
        let ep = endpoint.clone();
        let server = server.clone();
        let core = self;
        tokio::spawn(async move {
            loop {
                info!(name = %server.name, "connecting to server");
                match core.connect_to_server(&ep, &server, focus_rx.clone()).await {
                    Ok(()) => {
                        info!(name = %server.name, "disconnected from server");
                    }
                    Err(e) => {
                        warn!(name = %server.name, "connection error: {e:#}");
                    }
                }
                core.set_connection_state(&server.endpoint_id, &server.name, false);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        })
    }

    async fn connect_to_server(
        self: &Arc<Self>,
        endpoint: &iroh::Endpoint,
        server: &ServerEntry,
        focus_rx: watch::Receiver<FocusState>,
    ) -> anyhow::Result<()> {
        let ticket = Ticket {
            endpoint_id: server.endpoint_id.clone(),
            addrs: server.addrs.clone(),
            relay_url: server.relay_url.clone(),
            token: String::new(),
            hostname: server.name.clone(),
        };
        let addr = ticket.to_endpoint_addr()?;

        let conn = endpoint.connect(addr, ALPN).await?;

        info!(name = %server.name, "connected");

        // Open control bi-stream
        let (mut ctrl_send, ctrl_recv) = conn.open_bi().await?;
        let mut ctrl_reader = BufReader::new(ctrl_recv);

        // Send ClientHello
        let hello = ClientHello {
            name: self.device_name.clone(),
            capabilities: CAPABILITIES.iter().map(|s| (*s).to_string()).collect(),
            platform: "android".to_string(),
            token: None, // reconnection, not enrollment
        };
        write_control(&mut ctrl_send, &hello).await?;

        // Read ServerHello
        let server_hello: ServerHello = read_control(&mut ctrl_reader).await?;
        if !server_hello.enrolled {
            anyhow::bail!("server rejected connection (not enrolled)");
        }

        info!(hostname = %server_hello.hostname, "server hello received");

        self.set_connection_state(&server.endpoint_id, &server.name, true);

        // Spawn control message reader (dismiss notifications from other devices)
        let server_name = server.name.clone();
        let cb = self.callback.clone();
        tokio::spawn(async move {
            Self::read_control_loop(ctrl_reader, &server_name, cb.as_ref()).await;
        });

        // Spawn focus update sender
        tokio::spawn(async move {
            Self::send_focus_updates(ctrl_send, focus_rx).await;
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
            let core = self.clone();
            tokio::spawn(async move {
                if let Err(e) = core.handle_request_stream(send, recv, &host).await {
                    warn!("request handler error: {e:#}");
                }
            });
        }

        Ok(())
    }

    async fn handle_request_stream(
        self: &Arc<Self>,
        mut send: iroh::endpoint::SendStream,
        mut recv: iroh::endpoint::RecvStream,
        host: &str,
    ) -> anyhow::Result<()> {
        let value: serde_json::Value = transport::recv_request(&mut recv).await?;

        let notification_id = value
            .get("parameters")
            .and_then(|p| p.get("notification_id"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let request: Request = serde_json::from_value(value)?;
        info!("request from {host}: {request:?}");

        match self.handle_request(request, host, notification_id) {
            Ok(response) => {
                let wire_resp = response.to_wire();
                transport::send_response(&mut send, &wire_resp).await?;
            }
            Err(e) => {
                let wire_resp = e.to_wire();
                transport::send_response(&mut send, &wire_resp).await?;
            }
        }

        Ok(())
    }

    fn handle_request(
        &self,
        request: Request,
        host: &str,
        notification_id: Option<String>,
    ) -> Result<Response, ProtoError> {
        match request {
            Request::Ping {} => {
                info!("ping");
                Ok(Response::Ping {
                    capabilities: CAPABILITIES.iter().map(|s| (*s).to_string()).collect(),
                    version: VERSION.to_string(),
                    clients: vec![],
                    endpoint_id: String::new(),
                })
            }
            Request::OpenURI { ref uri } => {
                info!("open_uri: {uri}");
                let handled = self.callback.on_open_uri(uri.clone(), host.to_string());
                if handled {
                    Ok(Response::Ok)
                } else {
                    Err(ProtoError::UserDenied)
                }
            }
            Request::OpenFile {
                ref name,
                ref mime,
                ref content,
            } => {
                info!("open_file: {name} ({mime})");
                let decoded_len = BASE64.decode(content).map(|d| d.len()).unwrap_or(0);
                if decoded_len > MAX_FILE_SIZE {
                    return Err(ProtoError::FileTooLarge {
                        max_bytes: MAX_FILE_SIZE as u64,
                    });
                }
                let handled = self.callback.on_open_file(
                    name.clone(),
                    mime.clone(),
                    content.clone(),
                    host.to_string(),
                );
                if handled {
                    Ok(Response::Ok)
                } else {
                    Err(ProtoError::UserDenied)
                }
            }
            Request::Notify {
                ref title,
                ref body,
                ref urgency,
                ..
            } => {
                info!("notify: {title}");
                let nid = notification_id.unwrap_or_default();
                self.callback.on_notify(
                    nid,
                    title.clone(),
                    body.clone(),
                    urgency.clone(),
                    host.to_string(),
                );
                Ok(Response::Ok)
            }
            Request::NotifyDismiss { ref id } => {
                info!("notify_dismiss: {id}");
                self.callback.on_dismiss_notification(id.clone());
                Ok(Response::Ok)
            }
            Request::GenerateTicket { .. } => Err(ProtoError::NotSupported {
                capability: "GenerateTicket".into(),
            }),
            Request::RevokeClient { .. } => Err(ProtoError::NotSupported {
                capability: "RevokeClient".into(),
            }),
        }
    }

    async fn enroll_async(&self, ticket_json: &str) -> Result<String, SubportalError> {
        let ticket = Ticket::parse(ticket_json)?;

        info!(
            "enrolling with server '{}' ({})",
            ticket.hostname,
            &ticket.endpoint_id[..16.min(ticket.endpoint_id.len())]
        );

        let key = load_or_generate_keypair(&self.data_dir.join(KEYPAIR_FILE)).await?;

        let endpoint = iroh::Endpoint::builder()
            .secret_key(key)
            .bind()
            .await
            .map_err(|e| SubportalError::General {
                msg: format!("failed to bind endpoint: {e}"),
            })?;

        let addr = ticket.to_endpoint_addr()?;
        let conn = endpoint
            .connect(addr, ALPN)
            .await
            .map_err(|e| SubportalError::General {
                msg: format!("failed to connect to server: {e}"),
            })?;

        let (mut ctrl_send, ctrl_recv) =
            conn.open_bi().await.map_err(|e| SubportalError::General {
                msg: format!("failed to open control stream: {e}"),
            })?;
        let mut ctrl_reader = BufReader::new(ctrl_recv);

        let hello = ClientHello {
            name: self.device_name.clone(),
            capabilities: CAPABILITIES.iter().map(|s| (*s).to_string()).collect(),
            platform: "android".to_string(),
            token: Some(ticket.token.clone()),
        };
        write_control(&mut ctrl_send, &hello).await?;

        let server_hello: ServerHello = read_control(&mut ctrl_reader).await?;
        if !server_hello.enrolled {
            return Err(SubportalError::General {
                msg: "enrollment rejected by server (invalid or expired token)".into(),
            });
        }

        let entry = ServerEntry {
            endpoint_id: ticket.endpoint_id,
            name: ticket.hostname.clone(),
            addrs: ticket.addrs,
            relay_url: ticket.relay_url,
            enrolled_at: chrono::Utc::now(),
        };

        let mut registry = ServerRegistry::load(&self.data_dir).await?;
        registry.add(entry).await?;

        conn.close(0u8.into(), b"enrolled");
        endpoint.close().await;

        info!("enrolled with server '{}'", ticket.hostname);

        Ok(ticket.hostname)
    }

    async fn read_control_loop(
        mut reader: BufReader<iroh::endpoint::RecvStream>,
        server_name: &str,
        callback: &dyn SubportalCallback,
    ) {
        loop {
            match read_control::<ControlMessage>(&mut reader).await {
                Ok(msg) => match msg {
                    ControlMessage::DismissNotification { ref id } => {
                        info!(server = %server_name, "dismiss notification: {id}");
                        callback.on_dismiss_notification(id.clone());
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

    async fn send_focus_updates(
        mut send: iroh::endpoint::SendStream,
        mut focus_rx: watch::Receiver<FocusState>,
    ) {
        // Send initial state
        let state = *focus_rx.borrow();
        let msg = ControlMessage::FocusUpdate { state };
        if let Err(e) = write_control(&mut send, &msg).await {
            warn!("failed to send initial focus update: {e:#}");
            return;
        }

        // Watch for changes
        loop {
            if focus_rx.changed().await.is_err() {
                break; // sender dropped
            }
            let state = *focus_rx.borrow();
            let msg = ControlMessage::FocusUpdate { state };
            if let Err(e) = write_control(&mut send, &msg).await {
                warn!("failed to send focus update: {e:#}");
                break;
            }
        }
    }

    fn set_connection_state(&self, endpoint_id: &str, server_name: &str, connected: bool) {
        {
            let mut map = self.connected.lock().unwrap();
            map.insert(endpoint_id.to_string(), connected);
        }
        self.callback
            .on_connection_changed(server_name.to_string(), connected);
    }

    async fn reload_servers(
        self: &Arc<Self>,
        endpoint: &iroh::Endpoint,
        active: &mut HashMap<String, JoinHandle<()>>,
        focus_rx: &watch::Receiver<FocusState>,
    ) -> anyhow::Result<()> {
        let registry = ServerRegistry::load(&self.data_dir).await?;
        let servers = registry.list().to_vec();

        let new_ids: std::collections::HashSet<String> =
            servers.iter().map(|s| s.endpoint_id.clone()).collect();

        // Remove connections for servers no longer in the registry
        let to_remove: Vec<String> = active
            .keys()
            .filter(|id| !new_ids.contains(*id))
            .cloned()
            .collect();
        for id in to_remove {
            if let Some(h) = active.remove(&id) {
                info!(endpoint_id = %id, "removing connection to departed server");
                h.abort();
            }
        }

        // Add connections for new servers
        for server in &servers {
            if !active.contains_key(&server.endpoint_id) {
                info!(name = %server.name, "adding connection to new server");
                let handle =
                    self.clone()
                        .spawn_server_connection(endpoint, server, focus_rx.clone());
                active.insert(server.endpoint_id.clone(), handle);
            }
        }

        Ok(())
    }
}

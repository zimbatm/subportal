use std::collections::HashMap;
use std::sync::Arc;

use iroh::endpoint::Connection;
use iroh::TransportAddr;
use subportal_iroh::control::{ControlMessage, FocusState};
use subportal_iroh::peers::{ClientRegistry, PendingToken};
use subportal_iroh::ticket::Ticket;
use subportal_lib::consts::VERSION;
use subportal_lib::protocol::{Request, Response, SubportalError, VarlinkRequest};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::router::{self, ClientInfo, Strategy};

/// A connected client device.
pub struct ConnectedClient {
    pub endpoint_id: String,
    pub name: String,
    pub connection: Connection,
    pub focus: FocusState,
    pub capabilities: Vec<String>,
    /// Channel for sending control messages to this client's control stream writer.
    pub control_tx: mpsc::Sender<ControlMessage>,
}

/// Central hub managing connected clients and request routing.
pub struct Hub {
    pub clients: HashMap<String, ConnectedClient>,
    pub registry: ClientRegistry,
    pub pending_notifications: HashMap<String, NotificationState>,
    pub pending_tokens: Vec<PendingToken>,
    pub endpoint: iroh::Endpoint,
    pub hostname: String,
    notification_counter: u64,
}

/// Tracks which clients have been sent a notification (used for dismiss routing).
pub struct NotificationState {
    #[allow(dead_code)]
    pub sent_to: Vec<String>,
}

impl Hub {
    pub fn new(registry: ClientRegistry, endpoint: iroh::Endpoint, hostname: String) -> Self {
        Self {
            clients: HashMap::new(),
            registry,
            pending_notifications: HashMap::new(),
            pending_tokens: Vec::new(),
            endpoint,
            hostname,
            notification_counter: 0,
        }
    }

    /// Add a connected client to the hub.
    pub fn add_client(&mut self, client: ConnectedClient) {
        info!(
            endpoint_id = %client.endpoint_id,
            name = %client.name,
            "client connected"
        );
        self.clients.insert(client.endpoint_id.clone(), client);
    }

    /// Remove a connected client from the hub.
    pub fn remove_client(&mut self, endpoint_id: &str) {
        if let Some(c) = self.clients.remove(endpoint_id) {
            info!(
                endpoint_id = %c.endpoint_id,
                name = %c.name,
                "client disconnected"
            );
        }
    }

    /// Update focus state for a client.
    pub fn update_focus(&mut self, endpoint_id: &str, state: FocusState) {
        if let Some(c) = self.clients.get_mut(endpoint_id) {
            c.focus = state;
        }
    }

    /// Generate a unique notification ID.
    fn next_notification_id(&mut self) -> String {
        self.notification_counter += 1;
        format!("n{}", self.notification_counter)
    }

    /// Get a snapshot of connected client info for routing.
    fn client_infos(&self) -> Vec<ClientInfo> {
        self.clients
            .values()
            .map(|c| ClientInfo {
                endpoint_id: c.endpoint_id.clone(),
                focus: c.focus,
                capabilities: c.capabilities.clone(),
            })
            .collect()
    }

    /// Route a request to the appropriate client(s) and return the response.
    pub async fn route_request(&mut self, request: &Request) -> Result<Response, SubportalError> {
        let varlink_req = request.to_varlink();
        let strategy = router::strategy_for(&varlink_req.method);

        match strategy {
            Strategy::Direct => {
                // Agent responds directly
                match request {
                    Request::Ping => {
                        let mut all_caps: Vec<String> = self
                            .clients
                            .values()
                            .flat_map(|c| c.capabilities.iter().cloned())
                            .collect();
                        all_caps.sort();
                        all_caps.dedup();
                        Ok(Response::Ping {
                            capabilities: all_caps,
                            version: VERSION.to_string(),
                        })
                    }
                    Request::NotifyDismiss { id } => {
                        self.broadcast_dismiss(id, None).await;
                        Ok(Response::Ok)
                    }
                    Request::GenerateTicket { ttl } => self.generate_ticket(*ttl),
                    Request::RevokeClient { name_or_id } => self.revoke_client(name_or_id).await,
                    _ => Ok(Response::Ok),
                }
            }
            Strategy::PickBest(cap) => {
                let infos = self.client_infos();
                let best = router::pick_best(&infos, cap).ok_or(SubportalError::NoClient)?;
                let endpoint_id = best.endpoint_id.clone();
                self.send_to_client(&endpoint_id, &varlink_req).await
            }
            Strategy::FanOut(cap) => {
                let infos = self.client_infos();
                let targets = router::fan_out(&infos, cap);
                if targets.is_empty() {
                    return Err(SubportalError::NoClient);
                }

                let notification_id = self.next_notification_id();
                let sent_to: Vec<String> = targets.iter().map(|c| c.endpoint_id.clone()).collect();

                // Clone the request and inject notification_id so clients
                // can map their local D-Bus IDs back to this agent-level ID.
                let mut fan_req = varlink_req.clone();
                if let serde_json::Value::Object(ref mut map) = fan_req.parameters {
                    map.insert(
                        "notification_id".to_string(),
                        serde_json::Value::String(notification_id.clone()),
                    );
                }

                // Send to all targets
                for target in &targets {
                    let eid = target.endpoint_id.clone();
                    if let Err(e) = self.send_to_client(&eid, &fan_req).await {
                        warn!(
                            endpoint_id = %eid,
                            "failed to fan-out to client: {e}"
                        );
                    }
                }

                self.pending_notifications
                    .insert(notification_id.clone(), NotificationState { sent_to });

                Ok(Response::NotifyDelivered {
                    id: notification_id,
                })
            }
        }
    }

    /// Send a Varlink request to a specific connected client via a new QUIC bi-stream.
    async fn send_to_client(
        &self,
        endpoint_id: &str,
        req: &VarlinkRequest,
    ) -> Result<Response, SubportalError> {
        let client = self
            .clients
            .get(endpoint_id)
            .ok_or(SubportalError::NoClient)?;

        let (mut send, mut recv) = client
            .connection
            .open_bi()
            .await
            .map_err(|_| SubportalError::NoClient)?;

        subportal_iroh::transport::send_request(&mut send, req)
            .await
            .map_err(|_| SubportalError::NoClient)?;

        let resp = subportal_iroh::transport::recv_response(&mut recv)
            .await
            .map_err(|_| SubportalError::NoClient)?;

        // Check for error response
        if let Some(ref err_id) = resp.error {
            let params = resp
                .parameters
                .as_ref()
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            if let Some(e) = SubportalError::from_varlink(err_id, &params) {
                return Err(e);
            }
        }

        Ok(Response::Ok)
    }

    /// Broadcast a dismiss notification to all clients except the originator.
    pub async fn broadcast_dismiss(&self, notification_id: &str, except: Option<&str>) {
        let msg = ControlMessage::DismissNotification {
            id: notification_id.to_string(),
        };
        for (eid, client) in &self.clients {
            if except == Some(eid.as_str()) {
                continue;
            }
            if let Err(e) = client.control_tx.send(msg.clone()).await {
                warn!(endpoint_id = %eid, "failed to send dismiss: {e}");
            }
        }
    }

    /// Validate and consume a pending enrollment token.
    /// Returns true if the token was valid and consumed.
    pub fn consume_token(&mut self, token: &str) -> bool {
        // Prune expired tokens
        self.pending_tokens.retain(|t| !t.is_expired());
        let valid = self.pending_tokens.iter().any(|t| t.token == token);
        if valid {
            self.pending_tokens.retain(|t| t.token != token);
        }
        valid
    }

    /// Generate an enrollment ticket with a pending token.
    fn generate_ticket(&mut self, ttl: u64) -> Result<Response, SubportalError> {
        let token = PendingToken::generate(ttl);

        let addr = self.endpoint.addr();
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
                _ => {}
            }
        }

        let ticket = Ticket {
            endpoint_id,
            addrs,
            relay_url,
            token: token.token.clone(),
            hostname: self.hostname.clone(),
        };

        // Store the token so it can be validated when the client connects
        self.pending_tokens.push(token);

        Ok(Response::Ticket {
            ticket_json: ticket.to_json(),
        })
    }

    /// Revoke an enrolled client by name or endpoint ID.
    ///
    /// Removes the client from the persistent registry and, if the client is
    /// currently connected, disconnects it immediately.
    async fn revoke_client(&mut self, name_or_id: &str) -> Result<Response, SubportalError> {
        let removed =
            self.registry
                .remove(name_or_id)
                .await
                .map_err(|_| SubportalError::NotFound {
                    what: format!("failed to update registry for '{name_or_id}'"),
                })?;

        if !removed {
            return Err(SubportalError::NotFound {
                what: format!("no client matching '{name_or_id}'"),
            });
        }

        // Disconnect any matching connected client
        let to_disconnect: Vec<String> = self
            .clients
            .values()
            .filter(|c| c.endpoint_id == name_or_id || c.name == name_or_id)
            .map(|c| c.endpoint_id.clone())
            .collect();

        for eid in &to_disconnect {
            if let Some(c) = self.clients.remove(eid) {
                info!(
                    endpoint_id = %c.endpoint_id,
                    name = %c.name,
                    "revoked and disconnected client"
                );
                c.connection.close(0u32.into(), b"revoked");
            }
        }

        Ok(Response::Ok)
    }
}

/// Thread-safe handle to the hub.
pub type SharedHub = Arc<Mutex<Hub>>;

/// Create a new shared hub.
pub fn shared(registry: ClientRegistry, endpoint: iroh::Endpoint, hostname: String) -> SharedHub {
    Arc::new(Mutex::new(Hub::new(registry, endpoint, hostname)))
}

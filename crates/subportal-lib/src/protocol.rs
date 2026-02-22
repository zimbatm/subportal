//! Varlink protocol types and wire I/O.
//!
//! This module defines the typed request/response enums ([`Request`],
//! [`Response`]), the wire response type ([`WireResponse`]), domain errors
//! ([`SubportalError`]), and async functions for reading/writing NUL-delimited
//! JSON messages.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::consts::MAX_MESSAGE_SIZE;

/// NUL byte delimiter for Varlink wire format.
const NUL: u8 = 0;

// ---------------------------------------------------------------------------
// Wire-level types (Varlink JSON framing)
// ---------------------------------------------------------------------------

/// A raw Varlink response as it appears on the wire.
#[derive(Debug, Serialize, Deserialize)]
pub struct WireResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Typed request / response enums
// ---------------------------------------------------------------------------

/// A typed subportal request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "parameters")]
pub enum Request {
    /// Check connectivity and discover the daemon's capabilities.
    #[serde(rename = "io.subportal.Ping")]
    Ping {},
    /// Open a URI in the client's default application.
    #[serde(rename = "io.subportal.OpenURI")]
    OpenURI { uri: String },
    /// Transfer a file (base64-encoded) and open it on the client.
    #[serde(rename = "io.subportal.OpenFile")]
    OpenFile {
        name: String,
        mime: String,
        content: String,
    },
    /// Show a desktop notification on the client.
    #[serde(rename = "io.subportal.Notify")]
    Notify {
        title: String,
        body: Option<String>,
        urgency: Option<String>,
        icon: Option<String>,
    },
    /// Dismiss a notification by its ID across all connected devices.
    #[serde(rename = "io.subportal.NotifyDismiss")]
    NotifyDismiss { id: String },
    /// Generate an enrollment ticket (agent-only, via Unix socket).
    #[serde(rename = "io.subportal.GenerateTicket")]
    GenerateTicket { ttl: u64 },
    /// Revoke an enrolled client by name or endpoint ID (agent-only, via Unix socket).
    #[serde(rename = "io.subportal.RevokeClient")]
    RevokeClient { name_or_id: String },
}

/// A typed subportal response.
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    /// Reply to a [`Request::Ping`] with capabilities and version.
    Ping {
        capabilities: Vec<String>,
        version: String,
        clients: Vec<String>,
        endpoint_id: String,
    },
    /// Generic success for non-Ping requests.
    Ok,
    /// Notification was delivered and assigned an ID for dismiss tracking.
    NotifyDelivered { id: String },
    /// Generated enrollment ticket JSON.
    Ticket { ticket_json: String },
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can be returned by the subportal protocol.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SubportalError {
    #[error("user denied the request")]
    UserDenied,
    #[error("capability not supported: {capability}")]
    NotSupported { capability: String },
    #[error("file too large (max {max_bytes} bytes)")]
    FileTooLarge { max_bytes: u64 },
    #[error("no client daemon reachable")]
    NoClient,
    #[error("not found: {what}")]
    NotFound { what: String },
}

impl SubportalError {
    pub fn wire_id(&self) -> &'static str {
        match self {
            Self::UserDenied => "io.subportal.UserDenied",
            Self::NotSupported { .. } => "io.subportal.NotSupported",
            Self::FileTooLarge { .. } => "io.subportal.FileTooLarge",
            Self::NoClient => "io.subportal.NoClient",
            Self::NotFound { .. } => "io.subportal.NotFound",
        }
    }

    pub fn from_wire(id: &str, parameters: &serde_json::Value) -> Option<Self> {
        match id {
            "io.subportal.UserDenied" => Some(Self::UserDenied),
            "io.subportal.NotSupported" => {
                let capability = parameters
                    .get("capability")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Some(Self::NotSupported { capability })
            }
            "io.subportal.FileTooLarge" => {
                let max_bytes = parameters
                    .get("max_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                Some(Self::FileTooLarge { max_bytes })
            }
            "io.subportal.NoClient" => Some(Self::NoClient),
            "io.subportal.NotFound" => {
                let what = parameters
                    .get("what")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Some(Self::NotFound { what })
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion: typed → wire
// ---------------------------------------------------------------------------

impl Response {
    pub fn to_wire(&self) -> WireResponse {
        match self {
            Response::Ping {
                capabilities,
                version,
                clients,
                endpoint_id,
            } => WireResponse {
                parameters: Some(serde_json::json!({
                    "capabilities": capabilities,
                    "version": version,
                    "clients": clients,
                    "endpoint_id": endpoint_id,
                })),
                error: None,
            },
            Response::Ok => WireResponse {
                parameters: Some(serde_json::Value::Object(Default::default())),
                error: None,
            },
            Response::NotifyDelivered { id } => WireResponse {
                parameters: Some(serde_json::json!({ "id": id })),
                error: None,
            },
            Response::Ticket { ref ticket_json } => WireResponse {
                parameters: Some(serde_json::json!({ "ticket_json": ticket_json })),
                error: None,
            },
        }
    }
}

impl SubportalError {
    pub fn to_wire(&self) -> WireResponse {
        let parameters = match self {
            Self::NotSupported { capability } => {
                serde_json::json!({ "capability": capability })
            }
            Self::FileTooLarge { max_bytes } => {
                serde_json::json!({ "max_bytes": max_bytes })
            }
            Self::NotFound { what } => {
                serde_json::json!({ "what": what })
            }
            _ => serde_json::Value::Object(Default::default()),
        };
        WireResponse {
            parameters: Some(parameters),
            error: Some(self.wire_id().to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire I/O: read/write NUL-delimited JSON
// ---------------------------------------------------------------------------

/// Read a NUL-delimited JSON message from a buffered reader.
pub async fn read_message<T: serde::de::DeserializeOwned>(
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
) -> anyhow::Result<T> {
    let mut buf = Vec::new();
    reader
        .take(MAX_MESSAGE_SIZE as u64 + 1)
        .read_until(NUL, &mut buf)
        .await?;

    if buf.last() == Some(&NUL) {
        buf.pop(); // remove the NUL delimiter
    } else if buf.len() > MAX_MESSAGE_SIZE {
        anyhow::bail!("message exceeds maximum size ({MAX_MESSAGE_SIZE} bytes)");
    } else {
        anyhow::bail!("connection closed before NUL delimiter");
    }

    let msg = serde_json::from_slice(&buf)?;
    Ok(msg)
}

/// Write a NUL-delimited JSON message to an async writer.
pub async fn write_message<T: Serialize>(
    stream: &mut (impl tokio::io::AsyncWrite + Unpin),
    msg: &T,
) -> anyhow::Result<()> {
    let data = serde_json::to_vec(msg)?;
    stream.write_all(&data).await?;
    stream.write_u8(NUL).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader, DuplexStream};

    // -----------------------------------------------------------------------
    // Tier 1 -- Protocol unit tests (no I/O)
    // -----------------------------------------------------------------------

    #[test]
    fn request_ping_serde_round_trip() {
        let req = Request::Ping {};
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.Ping");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, Request::Ping {});
    }

    #[test]
    fn request_open_uri_serde_round_trip() {
        let req = Request::OpenURI {
            uri: "https://example.com".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.OpenURI");
        assert_eq!(json["parameters"]["uri"], "https://example.com");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_open_file_serde_round_trip() {
        let req = Request::OpenFile {
            name: "test.txt".into(),
            mime: "text/plain".into(),
            content: "aGVsbG8=".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.OpenFile");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_notify_serde_round_trip_full() {
        let req = Request::Notify {
            title: "Alert".into(),
            body: Some("Something happened".into()),
            urgency: Some("critical".into()),
            icon: Some("dialog-warning".into()),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.Notify");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_notify_serde_round_trip_minimal() {
        let req = Request::Notify {
            title: "Hello".into(),
            body: None,
            urgency: None,
            icon: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_notify_dismiss_serde_round_trip() {
        let req = Request::NotifyDismiss {
            id: "notif-123".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.NotifyDismiss");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_generate_ticket_serde_round_trip() {
        let req = Request::GenerateTicket { ttl: 600 };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.GenerateTicket");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_revoke_client_serde_round_trip() {
        let req = Request::RevokeClient {
            name_or_id: "laptop".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"], "io.subportal.RevokeClient");
        let back: Request = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_ignores_unknown_fields_in_parameters() {
        // Extra fields like "host" and "notification_id" should be silently ignored.
        let json = serde_json::json!({
            "method": "io.subportal.OpenURI",
            "parameters": { "uri": "https://example.com", "host": "myhost" }
        });
        let req: Request = serde_json::from_value(json).unwrap();
        assert_eq!(
            req,
            Request::OpenURI {
                uri: "https://example.com".into()
            }
        );
    }

    #[test]
    fn request_unknown_method_fails() {
        let json = serde_json::json!({
            "method": "io.subportal.Unknown",
            "parameters": {}
        });
        assert!(serde_json::from_value::<Request>(json).is_err());
    }

    #[test]
    fn request_missing_params_fails() {
        // OpenURI missing uri
        let json = serde_json::json!({
            "method": "io.subportal.OpenURI",
            "parameters": {}
        });
        assert!(serde_json::from_value::<Request>(json).is_err());

        // Notify missing title
        let json = serde_json::json!({
            "method": "io.subportal.Notify",
            "parameters": { "body": "test" }
        });
        assert!(serde_json::from_value::<Request>(json).is_err());
    }

    #[test]
    fn response_ping_to_wire() {
        let resp = Response::Ping {
            capabilities: vec!["open_uri".into(), "notify".into()],
            version: "0.1.0".into(),
            clients: vec!["laptop".into(), "desktop".into()],
            endpoint_id: "abc123".into(),
        };
        let vr = resp.to_wire();
        let params = vr.parameters.unwrap();
        assert!(vr.error.is_none());
        assert_eq!(params["version"], "0.1.0");
        let caps = params["capabilities"].as_array().unwrap();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0], "open_uri");
        assert_eq!(caps[1], "notify");
        let clients = params["clients"].as_array().unwrap();
        assert_eq!(clients.len(), 2);
        assert_eq!(clients[0], "laptop");
        assert_eq!(clients[1], "desktop");
        assert_eq!(params["endpoint_id"], "abc123");
    }

    #[test]
    fn response_ok_to_wire() {
        let resp = Response::Ok;
        let vr = resp.to_wire();
        assert!(vr.error.is_none());
        let params = vr.parameters.unwrap();
        assert!(params.as_object().unwrap().is_empty());
    }

    #[test]
    fn response_notify_delivered_to_wire() {
        let resp = Response::NotifyDelivered {
            id: "notif-456".into(),
        };
        let vr = resp.to_wire();
        assert!(vr.error.is_none());
        let params = vr.parameters.unwrap();
        assert_eq!(params["id"], "notif-456");
    }

    #[test]
    fn response_ticket_to_wire() {
        let resp = Response::Ticket {
            ticket_json: r#"{"endpoint_id":"abc"}"#.into(),
        };
        let vr = resp.to_wire();
        assert!(vr.error.is_none());
        let params = vr.parameters.unwrap();
        assert_eq!(params["ticket_json"], r#"{"endpoint_id":"abc"}"#);
    }

    #[test]
    fn error_wire_round_trip() {
        let errors = [
            SubportalError::UserDenied,
            SubportalError::NotSupported {
                capability: "OpenFile".into(),
            },
            SubportalError::FileTooLarge {
                max_bytes: 5_242_880,
            },
            SubportalError::NoClient,
            SubportalError::NotFound {
                what: "test".into(),
            },
        ];
        for err in &errors {
            let vr = err.to_wire();
            let params = vr.parameters.as_ref().unwrap();
            let back = SubportalError::from_wire(vr.error.as_deref().unwrap(), params).unwrap();
            assert_eq!(&back, err);
        }
    }

    #[test]
    fn error_unknown_id_returns_none() {
        let empty = serde_json::Value::Object(Default::default());
        assert!(SubportalError::from_wire("io.subportal.DoesNotExist", &empty).is_none());
        assert!(SubportalError::from_wire("", &empty).is_none());
    }

    #[test]
    fn error_to_wire_response_user_denied() {
        let err = SubportalError::UserDenied;
        let vr = err.to_wire();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.UserDenied"));
        assert!(vr.parameters.unwrap().as_object().unwrap().is_empty());
    }

    #[test]
    fn error_to_wire_response_not_supported() {
        let err = SubportalError::NotSupported {
            capability: "OpenFile".into(),
        };
        let vr = err.to_wire();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.NotSupported"));
        let params = vr.parameters.unwrap();
        assert_eq!(params["capability"], "OpenFile");
    }

    #[test]
    fn error_to_wire_response_file_too_large() {
        let err = SubportalError::FileTooLarge {
            max_bytes: 5_242_880,
        };
        let vr = err.to_wire();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.FileTooLarge"));
        let params = vr.parameters.unwrap();
        assert_eq!(params["max_bytes"], 5_242_880);
    }

    #[test]
    fn error_not_found_round_trip() {
        let err = SubportalError::NotFound {
            what: "no client matching 'laptop'".into(),
        };
        let vr = err.to_wire();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.NotFound"));
        let params = vr.parameters.as_ref().unwrap();
        let back = SubportalError::from_wire("io.subportal.NotFound", params).unwrap();
        assert_eq!(back, err);
    }

    #[test]
    fn wire_response_skips_none() {
        let vr = WireResponse {
            parameters: None,
            error: None,
        };
        let json = serde_json::to_string(&vr).unwrap();
        assert_eq!(json, "{}");
    }

    // -----------------------------------------------------------------------
    // Tier 2 -- Wire format tests (in-memory duplex, async)
    // -----------------------------------------------------------------------

    fn duplex_pair() -> (DuplexStream, DuplexStream) {
        tokio::io::duplex(MAX_MESSAGE_SIZE + 1024)
    }

    #[tokio::test]
    async fn wire_round_trip_request() {
        let (mut client, server) = duplex_pair();
        let req = Request::Ping {};
        write_message(&mut client, &req).await.unwrap();
        let back: Request = read_message(&mut BufReader::new(server)).await.unwrap();
        assert_eq!(back, Request::Ping {});
    }

    #[tokio::test]
    async fn wire_round_trip_response() {
        let (client, mut server) = duplex_pair();
        let resp = WireResponse {
            parameters: Some(serde_json::json!({"version": "0.1.0"})),
            error: None,
        };
        write_message(&mut server, &resp).await.unwrap();
        let back: WireResponse = read_message(&mut BufReader::new(client)).await.unwrap();
        assert_eq!(back.parameters.unwrap()["version"], "0.1.0");
        assert!(back.error.is_none());
    }

    #[tokio::test]
    async fn wire_multiple_messages() {
        let (mut client, server) = duplex_pair();
        let requests = [
            Request::Ping {},
            Request::OpenURI {
                uri: "https://a.com".into(),
            },
            Request::NotifyDismiss { id: "n1".into() },
        ];
        for req in &requests {
            write_message(&mut client, req).await.unwrap();
        }
        let mut reader = BufReader::new(server);
        for expected in &requests {
            let back: Request = read_message(&mut reader).await.unwrap();
            assert_eq!(&back, expected);
        }
    }

    #[tokio::test]
    async fn wire_message_too_large() {
        let (mut client, server) = duplex_pair();
        // Spawn the writer so it runs concurrently with the reader.
        tokio::spawn(async move {
            let big = vec![b'x'; MAX_MESSAGE_SIZE + 1];
            client.write_all(&big).await.unwrap();
            client.write_u8(NUL).await.unwrap();
            client.flush().await.unwrap();
        });
        let result: anyhow::Result<serde_json::Value> =
            read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("maximum size"),
            "unexpected error: {err_msg}"
        );
    }

    #[tokio::test]
    async fn wire_empty_message() {
        let (mut client, server) = duplex_pair();
        // Send just a NUL byte -- empty payload
        client.write_u8(NUL).await.unwrap();
        client.flush().await.unwrap();
        let result: anyhow::Result<serde_json::Value> =
            read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wire_invalid_json() {
        let (mut client, server) = duplex_pair();
        client.write_all(b"not json at all").await.unwrap();
        client.write_u8(NUL).await.unwrap();
        client.flush().await.unwrap();
        let result: anyhow::Result<serde_json::Value> =
            read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wire_connection_closed() {
        let (client, server) = duplex_pair();
        // Close the client side without sending NUL
        drop(client);
        let result: anyhow::Result<serde_json::Value> =
            read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
    }
}

//! Varlink protocol types and wire I/O.
//!
//! This module defines the typed request/response enums ([`Request`],
//! [`Response`]), the corresponding Varlink wire types ([`VarlinkRequest`],
//! [`VarlinkResponse`]), domain errors ([`SubportalError`]), and async
//! functions for reading/writing NUL-delimited JSON messages.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use crate::consts::MAX_MESSAGE_SIZE;

/// NUL byte delimiter for Varlink wire format.
const NUL: u8 = 0;

// ---------------------------------------------------------------------------
// Wire-level types (Varlink JSON framing)
// ---------------------------------------------------------------------------

/// A raw Varlink request as it appears on the wire.
#[derive(Debug, Serialize, Deserialize)]
pub struct VarlinkRequest {
    pub method: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// A raw Varlink response as it appears on the wire.
#[derive(Debug, Serialize, Deserialize)]
pub struct VarlinkResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Typed request / response enums
// ---------------------------------------------------------------------------

/// A typed subportal request.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Check connectivity and discover the daemon's capabilities.
    Ping,
    /// Open a URI in the client's default application.
    OpenURI { uri: String },
    /// Transfer a file (base64-encoded) and open it on the client.
    OpenFile { name: String, mime: String, content: String },
    /// Show a desktop notification on the client.
    Notify { title: String, body: Option<String>, urgency: Option<String>, icon: Option<String> },
}

/// A typed subportal response.
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    /// Reply to a [`Request::Ping`] with capabilities and version.
    Ping { capabilities: Vec<String>, version: String },
    /// Generic success for non-Ping requests.
    Ok,
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
}

impl SubportalError {
    pub fn varlink_id(&self) -> &'static str {
        match self {
            Self::UserDenied => "io.subportal.UserDenied",
            Self::NotSupported { .. } => "io.subportal.NotSupported",
            Self::FileTooLarge { .. } => "io.subportal.FileTooLarge",
            Self::NoClient => "io.subportal.NoClient",
        }
    }

    pub fn from_varlink(id: &str, parameters: &serde_json::Value) -> Option<Self> {
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
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion: typed ↔ wire
// ---------------------------------------------------------------------------

impl Request {
    pub fn to_varlink(&self) -> VarlinkRequest {
        match self {
            Request::Ping => VarlinkRequest {
                method: "io.subportal.Ping".into(),
                parameters: serde_json::Value::Object(Default::default()),
            },
            Request::OpenURI { uri } => VarlinkRequest {
                method: "io.subportal.OpenURI".into(),
                parameters: serde_json::json!({ "uri": uri }),
            },
            Request::OpenFile { name, mime, content } => VarlinkRequest {
                method: "io.subportal.OpenFile".into(),
                parameters: serde_json::json!({
                    "name": name,
                    "mime": mime,
                    "content": content,
                }),
            },
            Request::Notify { title, body, urgency, icon } => VarlinkRequest {
                method: "io.subportal.Notify".into(),
                parameters: serde_json::json!({
                    "title": title,
                    "body": body,
                    "urgency": urgency,
                    "icon": icon,
                }),
            },
        }
    }

    pub fn from_varlink(vr: &VarlinkRequest) -> anyhow::Result<Self> {
        match vr.method.as_str() {
            "io.subportal.Ping" => Ok(Request::Ping),
            "io.subportal.OpenURI" => {
                let uri = vr.parameters.get("uri")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing 'uri' parameter"))?
                    .to_string();
                Ok(Request::OpenURI { uri })
            }
            "io.subportal.OpenFile" => {
                let params = &vr.parameters;
                let name = params.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing 'name' parameter"))?
                    .to_string();
                let mime = params.get("mime")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing 'mime' parameter"))?
                    .to_string();
                let content = params.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing 'content' parameter"))?
                    .to_string();
                Ok(Request::OpenFile { name, mime, content })
            }
            "io.subportal.Notify" => {
                let params = &vr.parameters;
                let title = params.get("title")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing 'title' parameter"))?
                    .to_string();
                let body = params.get("body")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let urgency = params.get("urgency")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let icon = params.get("icon")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Ok(Request::Notify { title, body, urgency, icon })
            }
            other => anyhow::bail!("unknown method: {other}"),
        }
    }
}

impl Response {
    pub fn to_varlink(&self) -> VarlinkResponse {
        match self {
            Response::Ping { capabilities, version } => VarlinkResponse {
                parameters: Some(serde_json::json!({
                    "capabilities": capabilities,
                    "version": version,
                })),
                error: None,
            },
            Response::Ok => VarlinkResponse {
                parameters: Some(serde_json::Value::Object(Default::default())),
                error: None,
            },
        }
    }
}

impl SubportalError {
    pub fn to_varlink(&self) -> VarlinkResponse {
        let parameters = match self {
            Self::NotSupported { capability } => {
                serde_json::json!({ "capability": capability })
            }
            Self::FileTooLarge { max_bytes } => {
                serde_json::json!({ "max_bytes": max_bytes })
            }
            _ => serde_json::Value::Object(Default::default()),
        };
        VarlinkResponse {
            parameters: Some(parameters),
            error: Some(self.varlink_id().to_string()),
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
    reader.take(MAX_MESSAGE_SIZE as u64 + 1).read_until(NUL, &mut buf).await?;

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
    fn request_ping_round_trip() {
        let req = Request::Ping;
        let vr = req.to_varlink();
        assert_eq!(vr.method, "io.subportal.Ping");
        let back = Request::from_varlink(&vr).unwrap();
        assert_eq!(back, Request::Ping);
    }

    #[test]
    fn request_open_uri_round_trip() {
        let req = Request::OpenURI {
            uri: "https://example.com".into(),
        };
        let vr = req.to_varlink();
        assert_eq!(vr.method, "io.subportal.OpenURI");
        let back = Request::from_varlink(&vr).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_open_file_round_trip() {
        let req = Request::OpenFile {
            name: "test.txt".into(),
            mime: "text/plain".into(),
            content: "aGVsbG8=".into(),
        };
        let vr = req.to_varlink();
        assert_eq!(vr.method, "io.subportal.OpenFile");
        let back = Request::from_varlink(&vr).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_notify_round_trip_full() {
        let req = Request::Notify {
            title: "Alert".into(),
            body: Some("Something happened".into()),
            urgency: Some("critical".into()),
            icon: Some("dialog-warning".into()),
        };
        let vr = req.to_varlink();
        assert_eq!(vr.method, "io.subportal.Notify");
        let back = Request::from_varlink(&vr).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn request_notify_round_trip_minimal() {
        let req = Request::Notify {
            title: "Hello".into(),
            body: None,
            urgency: None,
            icon: None,
        };
        let vr = req.to_varlink();
        let back = Request::from_varlink(&vr).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn response_ping_to_varlink() {
        let resp = Response::Ping {
            capabilities: vec!["open_uri".into(), "notify".into()],
            version: "0.1.0".into(),
        };
        let vr = resp.to_varlink();
        let params = vr.parameters.unwrap();
        assert!(vr.error.is_none());
        assert_eq!(params["version"], "0.1.0");
        let caps = params["capabilities"].as_array().unwrap();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0], "open_uri");
        assert_eq!(caps[1], "notify");
    }

    #[test]
    fn response_ok_to_varlink() {
        let resp = Response::Ok;
        let vr = resp.to_varlink();
        assert!(vr.error.is_none());
        let params = vr.parameters.unwrap();
        assert!(params.as_object().unwrap().is_empty());
    }

    #[test]
    fn error_varlink_round_trip() {
        let errors = [
            SubportalError::UserDenied,
            SubportalError::NotSupported { capability: "OpenFile".into() },
            SubportalError::FileTooLarge { max_bytes: 5_242_880 },
            SubportalError::NoClient,
        ];
        for err in &errors {
            let vr = err.to_varlink();
            let params = vr.parameters.as_ref().unwrap();
            let back = SubportalError::from_varlink(vr.error.as_deref().unwrap(), params).unwrap();
            assert_eq!(&back, err);
        }
    }

    #[test]
    fn error_unknown_id_returns_none() {
        let empty = serde_json::Value::Object(Default::default());
        assert!(SubportalError::from_varlink("io.subportal.DoesNotExist", &empty).is_none());
        assert!(SubportalError::from_varlink("", &empty).is_none());
    }

    #[test]
    fn error_to_varlink_response_user_denied() {
        let err = SubportalError::UserDenied;
        let vr = err.to_varlink();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.UserDenied"));
        assert!(vr.parameters.unwrap().as_object().unwrap().is_empty());
    }

    #[test]
    fn error_to_varlink_response_not_supported() {
        let err = SubportalError::NotSupported { capability: "OpenFile".into() };
        let vr = err.to_varlink();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.NotSupported"));
        let params = vr.parameters.unwrap();
        assert_eq!(params["capability"], "OpenFile");
    }

    #[test]
    fn error_to_varlink_response_file_too_large() {
        let err = SubportalError::FileTooLarge { max_bytes: 5_242_880 };
        let vr = err.to_varlink();
        assert_eq!(vr.error.as_deref(), Some("io.subportal.FileTooLarge"));
        let params = vr.parameters.unwrap();
        assert_eq!(params["max_bytes"], 5_242_880);
    }

    #[test]
    fn from_varlink_unknown_method() {
        let vr = VarlinkRequest {
            method: "io.subportal.Unknown".into(),
            parameters: serde_json::Value::Object(Default::default()),
        };
        assert!(Request::from_varlink(&vr).is_err());
    }

    #[test]
    fn from_varlink_missing_params() {
        // OpenURI missing uri
        let vr = VarlinkRequest {
            method: "io.subportal.OpenURI".into(),
            parameters: serde_json::json!({}),
        };
        assert!(Request::from_varlink(&vr).is_err());

        // OpenFile missing name
        let vr = VarlinkRequest {
            method: "io.subportal.OpenFile".into(),
            parameters: serde_json::json!({"mime": "text/plain", "content": "abc"}),
        };
        assert!(Request::from_varlink(&vr).is_err());

        // Notify missing title
        let vr = VarlinkRequest {
            method: "io.subportal.Notify".into(),
            parameters: serde_json::json!({"body": "test"}),
        };
        assert!(Request::from_varlink(&vr).is_err());
    }

    #[test]
    fn varlink_request_json_round_trip() {
        let vr = VarlinkRequest {
            method: "io.subportal.OpenURI".into(),
            parameters: serde_json::json!({"uri": "https://example.com"}),
        };
        let json = serde_json::to_string(&vr).unwrap();
        let back: VarlinkRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.method, vr.method);
        assert_eq!(back.parameters, vr.parameters);
    }

    #[test]
    fn varlink_request_default_params() {
        let json = r#"{"method":"io.subportal.Ping"}"#;
        let vr: VarlinkRequest = serde_json::from_str(json).unwrap();
        assert_eq!(vr.method, "io.subportal.Ping");
        assert!(vr.parameters.is_null());
    }

    #[test]
    fn varlink_response_skips_none() {
        let vr = VarlinkResponse {
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
        let req = VarlinkRequest {
            method: "io.subportal.Ping".into(),
            parameters: serde_json::Value::Object(Default::default()),
        };
        write_message(&mut client, &req).await.unwrap();
        let back: VarlinkRequest = read_message(&mut BufReader::new(server)).await.unwrap();
        assert_eq!(back.method, "io.subportal.Ping");
    }

    #[tokio::test]
    async fn wire_round_trip_response() {
        let (client, mut server) = duplex_pair();
        let resp = VarlinkResponse {
            parameters: Some(serde_json::json!({"version": "0.1.0"})),
            error: None,
        };
        write_message(&mut server, &resp).await.unwrap();
        let back: VarlinkResponse = read_message(&mut BufReader::new(client)).await.unwrap();
        assert_eq!(back.parameters.unwrap()["version"], "0.1.0");
        assert!(back.error.is_none());
    }

    #[tokio::test]
    async fn wire_multiple_messages() {
        let (mut client, server) = duplex_pair();
        for i in 0..3 {
            let req = VarlinkRequest {
                method: format!("io.subportal.Test{i}"),
                parameters: serde_json::Value::Object(Default::default()),
            };
            write_message(&mut client, &req).await.unwrap();
        }
        let mut reader = BufReader::new(server);
        for i in 0..3 {
            let back: VarlinkRequest = read_message(&mut reader).await.unwrap();
            assert_eq!(back.method, format!("io.subportal.Test{i}"));
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
        let result: anyhow::Result<VarlinkRequest> = read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("maximum size"), "unexpected error: {err_msg}");
    }

    #[tokio::test]
    async fn wire_empty_message() {
        let (mut client, server) = duplex_pair();
        // Send just a NUL byte -- empty payload
        client.write_u8(NUL).await.unwrap();
        client.flush().await.unwrap();
        let result: anyhow::Result<VarlinkRequest> = read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wire_invalid_json() {
        let (mut client, server) = duplex_pair();
        client.write_all(b"not json at all").await.unwrap();
        client.write_u8(NUL).await.unwrap();
        client.flush().await.unwrap();
        let result: anyhow::Result<VarlinkRequest> = read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn wire_connection_closed() {
        let (client, server) = duplex_pair();
        // Close the client side without sending NUL
        drop(client);
        let result: anyhow::Result<VarlinkRequest> = read_message(&mut BufReader::new(server)).await;
        assert!(result.is_err());
    }
}

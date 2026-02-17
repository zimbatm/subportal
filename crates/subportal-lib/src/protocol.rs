use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::consts::MAX_MESSAGE_SIZE;

/// NUL byte delimiter for Varlink wire format.
const NUL: u8 = 0;

// ---------------------------------------------------------------------------
// Wire-level types (Varlink JSON framing)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct VarlinkRequest {
    pub method: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

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

#[derive(Debug, Clone)]
pub enum Request {
    Ping,
    OpenURI { uri: String },
    OpenFile { name: String, mime: String, content: String },
    Notify { title: String, body: Option<String>, urgency: Option<String>, icon: Option<String> },
}

#[derive(Debug, Clone)]
pub enum Response {
    Ping { capabilities: Vec<String>, version: String },
    Ok,
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum SubportalError {
    #[error("user denied the request")]
    UserDenied,
    #[error("operation not supported")]
    NotSupported,
    #[error("file too large")]
    FileTooLarge,
    #[error("no client daemon reachable")]
    NoClient,
}

impl SubportalError {
    pub fn varlink_id(&self) -> &'static str {
        match self {
            Self::UserDenied => "io.subportal.UserDenied",
            Self::NotSupported => "io.subportal.NotSupported",
            Self::FileTooLarge => "io.subportal.FileTooLarge",
            Self::NoClient => "io.subportal.NoClient",
        }
    }

    pub fn from_varlink_id(id: &str) -> Option<Self> {
        match id {
            "io.subportal.UserDenied" => Some(Self::UserDenied),
            "io.subportal.NotSupported" => Some(Self::NotSupported),
            "io.subportal.FileTooLarge" => Some(Self::FileTooLarge),
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
        VarlinkResponse {
            parameters: Some(serde_json::Value::Object(Default::default())),
            error: Some(self.varlink_id().to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire I/O: read/write NUL-delimited JSON over TCP
// ---------------------------------------------------------------------------

/// Read a NUL-delimited JSON message from a TCP stream.
pub async fn read_message<T: serde::de::DeserializeOwned>(stream: &mut TcpStream) -> anyhow::Result<T> {
    let mut buf = Vec::new();
    loop {
        let byte = stream.read_u8().await?;
        if byte == NUL {
            break;
        }
        buf.push(byte);
        if buf.len() > MAX_MESSAGE_SIZE {
            anyhow::bail!("message exceeds maximum size ({MAX_MESSAGE_SIZE} bytes)");
        }
    }
    let msg = serde_json::from_slice(&buf)?;
    Ok(msg)
}

/// Write a NUL-delimited JSON message to a TCP stream.
pub async fn write_message<T: Serialize>(stream: &mut TcpStream, msg: &T) -> anyhow::Result<()> {
    let data = serde_json::to_vec(msg)?;
    stream.write_all(&data).await?;
    stream.write_u8(NUL).await?;
    stream.flush().await?;
    Ok(())
}

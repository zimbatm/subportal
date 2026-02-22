//! Control channel messages exchanged over the first bi-directional QUIC stream.
//!
//! The control stream uses the same NUL-delimited JSON wire format as the
//! Varlink protocol, but carries typed control messages instead.

use serde::{Deserialize, Serialize};
use subportal_lib::protocol::{read_message, write_message};

/// Sent by the client when the control stream is opened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    /// Human-readable name for this client device.
    pub name: String,
    /// Capabilities supported by this client (e.g. "OpenURI", "Notify").
    pub capabilities: Vec<String>,
    /// Platform identifier (e.g. "linux", "macos").
    pub platform: String,
    /// Enrollment token (present = enrollment, absent = reconnection).
    pub token: Option<String>,
}

/// Sent by the server in response to ClientHello.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    /// Hostname of the server.
    pub hostname: String,
    /// Whether the client is now enrolled.
    pub enrolled: bool,
}

/// Focus state reported by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusState {
    /// The user is actively using this device.
    Active,
    /// The device is idle / screen locked.
    Idle,
}

/// A control message sent over the control bi-stream after the hello exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlMessage {
    /// Focus state update from client to server.
    FocusUpdate { state: FocusState },
    /// Dismiss a notification across all devices.
    DismissNotification { id: String },
}

/// Write a control message to the stream.
pub async fn write_control<T: Serialize>(
    stream: &mut (impl tokio::io::AsyncWrite + Unpin),
    msg: &T,
) -> anyhow::Result<()> {
    write_message(stream, msg).await
}

/// Read a control message from the stream.
pub async fn read_control<T: serde::de::DeserializeOwned>(
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
) -> anyhow::Result<T> {
    read_message(reader).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn client_hello_round_trip() {
        let (mut tx, rx) = tokio::io::duplex(4096);
        let hello = ClientHello {
            name: "laptop".into(),
            capabilities: vec!["OpenURI".into(), "Notify".into()],
            platform: "linux".into(),
            token: Some("abc123".into()),
        };
        write_control(&mut tx, &hello).await.unwrap();
        let mut reader = BufReader::new(rx);
        let back: ClientHello = read_control(&mut reader).await.unwrap();
        assert_eq!(back.name, "laptop");
        assert_eq!(back.token, Some("abc123".into()));
    }

    #[tokio::test]
    async fn control_message_round_trip() {
        let (mut tx, rx) = tokio::io::duplex(4096);
        let msg = ControlMessage::FocusUpdate {
            state: FocusState::Active,
        };
        write_control(&mut tx, &msg).await.unwrap();
        let mut reader = BufReader::new(rx);
        let back: ControlMessage = read_control(&mut reader).await.unwrap();
        match back {
            ControlMessage::FocusUpdate { state } => assert_eq!(state, FocusState::Active),
            _ => panic!("wrong message type"),
        }
    }

    #[tokio::test]
    async fn dismiss_notification_round_trip() {
        let (mut tx, rx) = tokio::io::duplex(4096);
        let msg = ControlMessage::DismissNotification {
            id: "notif-42".into(),
        };
        write_control(&mut tx, &msg).await.unwrap();
        let mut reader = BufReader::new(rx);
        let back: ControlMessage = read_control(&mut reader).await.unwrap();
        match back {
            ControlMessage::DismissNotification { id } => assert_eq!(id, "notif-42"),
            _ => panic!("wrong message type"),
        }
    }
}

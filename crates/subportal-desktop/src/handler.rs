//! Request dispatch for the subportal daemon.
//!
//! Maps each incoming [`Request`] variant to the corresponding
//! [`crate::portal`] function and returns the protocol response or error.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use subportal_lib::consts::{MAX_FILE_SIZE, VERSION};
use subportal_lib::protocol::{Request, Response, SubportalError};
use tracing::{info, warn};

use crate::{dismiss, portal};

/// Capabilities advertised by the daemon.
pub const CAPABILITIES: &[&str] = &["OpenURI", "OpenFile", "Notify", "Confirm"];

/// Handle a parsed request and return the response or error.
///
/// `host` is the originating server's hostname, if provided in the request.
/// `notification_id` is the agent-assigned ID for dismiss tracking (fan-out only).
pub async fn handle(
    request: Request,
    host: Option<&str>,
    notification_id: Option<String>,
) -> Result<Response, SubportalError> {
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
            portal::open_uri(uri).await.map_err(|e| {
                warn!("open_uri failed: {e:#}");
                SubportalError::UserDenied
            })?;
            Ok(Response::Ok)
        }
        Request::OpenFile {
            ref name,
            ref mime,
            ref content,
        } => {
            info!("open_file: {name} ({mime})");
            // Validate decoded file size before writing to disk.
            let decoded_len = BASE64.decode(content).map(|d| d.len()).unwrap_or(0);
            if decoded_len > MAX_FILE_SIZE {
                return Err(SubportalError::FileTooLarge {
                    max_bytes: MAX_FILE_SIZE as u64,
                });
            }
            portal::open_file(name, mime, content).await.map_err(|e| {
                warn!("open_file failed: {e:#}");
                SubportalError::UserDenied
            })?;
            Ok(Response::Ok)
        }
        Request::Notify {
            ref title,
            ref body,
            ref urgency,
            ref icon,
        } => {
            info!("notify: {title}");
            let dbus_id = portal::notify(
                title,
                body.as_deref(),
                urgency.as_deref(),
                icon.as_deref(),
                host,
            )
            .await
            .map_err(|e| {
                warn!("notify failed: {e:#}");
                SubportalError::NotSupported {
                    capability: "Notify".into(),
                }
            })?;
            if let Some(nid) = notification_id {
                dismiss::register(nid, dbus_id);
            }
            Ok(Response::Ok)
        }
        Request::Confirm {
            ref message,
            ref title,
        } => {
            info!("confirm: {message}");
            // Can't show the prompt -> abstain; another device may still ask.
            let decision = portal::confirm(message, title.as_deref(), host)
                .await
                .map_err(|e| {
                    warn!("confirm failed: {e:#}");
                    SubportalError::NoDecision
                })?;
            match decision {
                portal::ConfirmDecision::Approved => Ok(Response::Ok),
                portal::ConfirmDecision::Denied => Err(SubportalError::UserDenied),
                portal::ConfirmDecision::NoDecision => Err(SubportalError::NoDecision),
            }
        }
        Request::NotifyDismiss { ref id } => {
            info!("notify_dismiss: {id}");
            Ok(Response::Ok)
        }
        Request::GenerateTicket { .. } => {
            // GenerateTicket is agent-only; clients should never receive it.
            warn!("received GenerateTicket on client daemon -- ignoring");
            Err(SubportalError::NotSupported {
                capability: "GenerateTicket".into(),
            })
        }
        Request::RevokeClient { .. } => {
            // RevokeClient is agent-only; clients should never receive it.
            warn!("received RevokeClient on client daemon -- ignoring");
            Err(SubportalError::NotSupported {
                capability: "RevokeClient".into(),
            })
        }
    }
}

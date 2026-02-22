//! Request dispatch for the subportal daemon.
//!
//! Maps each incoming [`Request`] variant to the corresponding
//! [`crate::portal`] function and returns the protocol response or error.

use subportal_lib::consts::VERSION;
use subportal_lib::protocol::{Request, Response, SubportalError};
use tracing::{info, warn};

use crate::{dismiss, portal};

/// Capabilities advertised by the daemon.
pub const CAPABILITIES: &[&str] = &["OpenURI", "OpenFile", "Notify"];

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

//! Request dispatch for the subportal daemon.
//!
//! Maps each incoming [`Request`] variant to the corresponding
//! [`crate::portal`] function and returns the protocol response or error.

use subportal_lib::consts::VERSION;
use subportal_lib::protocol::{Request, Response, SubportalError};
use tracing::{info, warn};

use crate::portal;

/// V1 capabilities advertised by the daemon.
const CAPABILITIES: &[&str] = &["OpenURI", "OpenFile", "Notify"];

/// Handle a parsed request and return the response or error.
///
/// `host` is the originating server's hostname, if provided in the request.
pub async fn handle(
    request: Request,
    host: Option<&str>,
) -> Result<Response, SubportalError> {
    match request {
        Request::Ping => {
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
            portal::notify(
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
            Ok(Response::Ok)
        }
    }
}

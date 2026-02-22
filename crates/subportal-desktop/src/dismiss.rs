use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tracing::{info, warn};

/// Maps agent notification IDs (e.g. "n1") to D-Bus notification u32 IDs.
static NOTIFY_MAP: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a mapping from an agent notification ID to a D-Bus notification ID.
pub fn register(agent_id: String, dbus_id: u32) {
    let mut map = NOTIFY_MAP.lock().unwrap();
    map.insert(agent_id, dbus_id);
}

/// Dismiss a notification by its agent tracking ID.
///
/// Looks up the D-Bus notification ID and calls `CloseNotification(u32)`.
/// Errors from D-Bus are logged and ignored (the notification may already be gone).
pub async fn dismiss_notification(agent_id: &str) {
    let dbus_id = {
        let mut map = NOTIFY_MAP.lock().unwrap();
        map.remove(agent_id)
    };

    let Some(dbus_id) = dbus_id else {
        info!("no D-Bus ID for notification {agent_id}, ignoring dismiss");
        return;
    };

    info!("dismissing notification {agent_id} (D-Bus ID {dbus_id})");

    if let Err(e) = close_notification(dbus_id).await {
        warn!("failed to close notification {dbus_id}: {e:#}");
    }
}

/// Call `org.freedesktop.Notifications.CloseNotification(u32)` via D-Bus.
async fn close_notification(id: u32) -> anyhow::Result<()> {
    use anyhow::Context;
    use zbus::Connection;

    let connection = Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;

    connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "CloseNotification",
            &(id,),
        )
        .await
        .context("CloseNotification call failed")?;

    Ok(())
}

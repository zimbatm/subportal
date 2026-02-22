use tracing::info;

/// Dismiss a local notification by its tracking ID.
///
/// In a future version this would use D-Bus `CloseNotification(id)` to close
/// the notification on the local desktop. For now it just logs the event.
pub async fn dismiss_notification(id: &str) {
    info!("dismissing notification {id} (not yet implemented)");
}

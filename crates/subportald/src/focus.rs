use subportal_iroh::control::{write_control, ControlMessage, FocusState};
use tracing::warn;

/// Periodically send focus state updates over the control stream.
///
/// Currently reports Active always. A future version could use D-Bus
/// `org.freedesktop.ScreenSaver.GetActive()` to detect idle state.
pub async fn send_focus_updates(mut send: iroh::endpoint::SendStream) {
    // Send initial Active state
    let msg = ControlMessage::FocusUpdate {
        state: FocusState::Active,
    };
    if let Err(e) = write_control(&mut send, &msg).await {
        warn!("failed to send initial focus update: {e:#}");
        return;
    }

    // Poll for changes periodically
    let mut current = FocusState::Active;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let new_state = detect_focus_state().await;
        if new_state != current {
            let msg = ControlMessage::FocusUpdate { state: new_state };
            if let Err(e) = write_control(&mut send, &msg).await {
                warn!("failed to send focus update: {e:#}");
                break;
            }
            current = new_state;
        }
    }
}

/// Detect the current focus state.
///
/// Falls back to Active if detection fails.
async fn detect_focus_state() -> FocusState {
    // Try D-Bus org.freedesktop.ScreenSaver.GetActive
    // For now, always report Active as a safe default.
    FocusState::Active
}

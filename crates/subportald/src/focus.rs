use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context;
use subportal_iroh::control::{write_control, ControlMessage, FocusState};
use tracing::warn;
use zbus::Connection;

/// Periodically send focus state updates over the control stream.
///
/// Uses D-Bus `org.freedesktop.ScreenSaver.GetActive()` to detect idle state.
/// Falls back to `Active` if D-Bus is unavailable.
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

/// Whether we have already logged that ScreenSaver D-Bus is unsupported.
static SCREENSAVER_WARNED: AtomicBool = AtomicBool::new(false);

/// Detect the current focus state via D-Bus ScreenSaver.
///
/// Falls back to Active if detection fails.
async fn detect_focus_state() -> FocusState {
    match query_screensaver_active().await {
        Ok(true) => FocusState::Idle,
        Ok(false) => FocusState::Active,
        Err(e) => {
            if !SCREENSAVER_WARNED.swap(true, Ordering::Relaxed) {
                warn!("ScreenSaver D-Bus unsupported, assuming Active: {e:#}");
            }
            FocusState::Active
        }
    }
}

/// Query `org.freedesktop.ScreenSaver.GetActive()` via D-Bus.
///
/// Returns `true` if the screen saver is active (i.e. screen is locked/idle).
async fn query_screensaver_active() -> anyhow::Result<bool> {
    let connection = Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;

    let active: bool = connection
        .call_method(
            Some("org.freedesktop.ScreenSaver"),
            "/org/freedesktop/ScreenSaver",
            Some("org.freedesktop.ScreenSaver"),
            "GetActive",
            &(),
        )
        .await
        .context("ScreenSaver.GetActive call failed")?
        .body()
        .deserialize()
        .context("failed to parse ScreenSaver.GetActive reply")?;

    Ok(active)
}
